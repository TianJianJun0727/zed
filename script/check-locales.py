#!/usr/bin/env python3
"""Check embedded locale keys, interpolation parameters, and literal call sites."""

from pathlib import Path
import argparse
import json
import re
import tomllib

root = Path(__file__).resolve().parent.parent
locales = root / "crates/zed_i18n/locales"


def flatten(value, prefix=""):
    result = {}
    for key, item in value.items():
        if key.startswith("_"):
            continue
        name = f"{prefix}.{key}" if prefix else key
        if isinstance(item, dict):
            result.update(flatten(item, name))
        elif isinstance(item, str):
            result[name] = item
    return result


def load(locale):
    result = {}
    paths = [locales / f"{locale}.toml", *sorted((locales / locale).glob("*.toml"))]
    for path in paths:
        with path.open("rb") as source:
            for key, text in flatten(tomllib.load(source)).items():
                if key in result:
                    raise ValueError(f"Duplicate key {key} in {path.relative_to(root)}")
                if not text.strip():
                    raise ValueError(f"Empty translation {key} in {path.relative_to(root)}")
                result[key] = text
    return result


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--actions", type=Path, help="JSON emitted by zed-lite --dump-all-actions")
args = parser.parse_args()

english, chinese = load("en"), load("zh-CN")
errors = []
canonical_labels = {
    "Step In": "单步进入",
    "Step Into": "单步进入",
    "Show Incoming Calls": "显示调用方",
    "Show Outgoing Calls": "显示被调用方",
    "Copy and Trim": "复制并去除缩进",
    "Suppress": "不再显示",
    "Gutter": "行号栏",
    "Breadcrumbs": "面包屑导航",
}
for key in sorted(english.keys() ^ chinese.keys()):
    errors.append(f"Locale key mismatch: {key}")
for key in english.keys() & chinese.keys():
    placeholders = lambda text: set(re.findall(r"%\{([^}]+)\}", text))
    if placeholders(english[key]) != placeholders(chinese[key]):
        errors.append(f"Interpolation mismatch: {key}")
    expected_label = canonical_labels.get(english[key])
    if expected_label and chinese[key] != expected_label:
        errors.append(f"Inconsistent terminology: {key}")
    if any(term in chinese[key] for term in ("内联提示", "装订线", "痕迹导航")):
        errors.append(f"Inconsistent terminology: {key}")
    if key.startswith(("command_descriptions.", "schema_descriptions.")):
        action_references = lambda text: set(re.findall(r"\b[a-z_]+::[A-Z]\w*", text))
        if not action_references(english[key]) <= action_references(chinese[key]):
            errors.append(f"Command reference changed in translation: {key}")
        code_examples = lambda text: re.findall(r"```[^\n]*\n(.*?)```", text, re.DOTALL)
        if code_examples(english[key]) != code_examples(chinese[key]):
            errors.append(f"Code example changed in translation: {key}")
for path in (root / "crates").rglob("*.rs"):
    if "zed_i18n" in path.parts:
        continue
    source = "\n".join(line for line in path.read_text().splitlines() if not line.lstrip().startswith("//"))
    for key in re.findall(r'\bt!\s*\(\s*"([^"]+)"', source):
        if key not in english:
            errors.append(f"Missing key {key}: {path.relative_to(root)}")
if args.actions:
    dump = json.loads(args.actions.read_text())
    for action in dump["actions"]:
        name = action["name"]
        if f"commands.{name}" not in chinese:
            errors.append(f"Missing command name: {name}")
        documentation = action.get("documentation")
        if documentation:
            key = f"command_descriptions.{name}"
            if key not in chinese:
                errors.append(f"Missing command description: {name}")
            elif english.get(key) != documentation:
                errors.append(f"Outdated English command description: {name}")

    registered = {action["name"] for action in dump["actions"]}
    translated = {key.removeprefix("commands.") for key in chinese if key.startswith("commands.")}
    # This action is registered only in debug builds; CI exports a release binary.
    debug_actions = {"dev::ShowWorkspaceError"}
    for name in sorted(translated - registered - debug_actions):
        errors.append(f"Unused command translation: {name}")

    schema_translations = {}
    for key, text in english.items():
        if key.startswith(("schema_descriptions.", "command_descriptions.")):
            schema_translations.setdefault(text, set()).add(chinese[key])

    def compare_schema(source, translated, pseudo, path=""):
        if isinstance(source, dict):
            if source.keys() != translated.keys() or source.keys() != pseudo.keys():
                errors.append(f"Schema keys changed: {path}")
                return
            for key, value in source.items():
                location = f"{path}/{key}"
                if key == "description" and isinstance(value, str):
                    action_key = f"command_descriptions.{source.get('const', '')}"
                    expected = {chinese[action_key]} if action_key in chinese else schema_translations.get(value)
                    if expected is None:
                        errors.append(f"Missing schema description {location}: {value!r}")
                    elif translated[key] not in expected:
                        errors.append(f"Incorrect schema translation: {location}")
                    if pseudo[key] != f"⟦{value}⟧":
                        errors.append(f"Schema pseudo translation missing: {location}")
                else:
                    compare_schema(value, translated[key], pseudo[key], location)
        elif isinstance(source, list):
            if len(source) != len(translated) or len(source) != len(pseudo):
                errors.append(f"Schema alternatives changed: {path}")
                return
            for index, (value, target, marked) in enumerate(zip(source, translated, pseudo)):
                compare_schema(value, target, marked, f"{path}/{index}")
        elif source != translated or source != pseudo:
            errors.append(f"Schema contract changed: {path}")

    schemas = dump["localized_schemas"]
    compare_schema(schemas["en"], schemas["zh-CN"], schemas["pseudo"])
    print(f"Checked {len(dump['actions'])} registered commands and localized JSON schemas.")

if errors:
    raise SystemExit("\n".join(sorted(set(errors))))
print(f"Validated {len(english)} keys in English and Simplified Chinese.")
