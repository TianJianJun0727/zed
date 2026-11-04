#!/usr/bin/env python3
"""Allow repackaging only a fully validated build with unchanged application sources."""

import argparse
import json
import os
import re
import subprocess
import sys

WORKFLOW_PATH = ".github/workflows/local-editor-macos.yml"
NATIVE_STEPS = {
    "Verify upstream stable version",
    "Check application types",
    "Dependency licenses",
    "Build application and CLI",
    "Localization coverage and schema contracts",
    "Check test targets",
    "Local editor UI contracts",
    "Local project regression tests",
    "Settings migration tests",
    "Updater contract tests",
}
PACKAGING_FILES = {
    "README.md",
    WORKFLOW_PATH,
    "script/install-bundled-git-macos",
    "script/check-macos-package-source.py",
    "script/tests/test_macos_package_source.py",
}


def validated_build(run, jobs, repository):
    if run.get("status") != "completed" or run.get("conclusion") != "success":
        raise ValueError("The source build must have completed successfully")
    if (run.get("path") != WORKFLOW_PATH or run.get("head_branch") != "main"
            or (run.get("head_repository") or {}).get("full_name") != repository
            or run.get("event") not in ("push", "workflow_dispatch")):
        raise ValueError("The source must be this repository's main macOS workflow")
    source_sha = run.get("head_sha")
    if not isinstance(source_sha, str) or not re.fullmatch(r"[0-9a-f]{40}", source_sha):
        raise ValueError("The source build has an invalid commit SHA")
    builds = [job for job in jobs if job.get("name") == "build"]
    if len(builds) != 1 or builds[0].get("conclusion") != "success":
        raise ValueError("Expected one successful native build job")
    passed = {step["name"] for step in builds[0].get("steps", [])
              if step.get("status") == "completed" and step.get("conclusion") == "success"}
    missing = NATIVE_STEPS - passed
    if missing:
        raise ValueError("Source lacks successful native validation: " + ", ".join(sorted(missing)))
    return source_sha


def check_package_artifact(artifacts):
    packages = [artifact for artifact in artifacts
                if artifact.get("name") == "Zed-Lite-macOS-arm64" and artifact.get("expired") is False]
    if len(packages) != 1:
        raise ValueError("Expected one unexpired macOS package artifact")


def check_app_changes(paths):
    changed = set(paths) - PACKAGING_FILES
    if changed:
        raise ValueError("Application sources changed; run a full build: " + ", ".join(sorted(changed)))


def api(path):
    return json.loads(subprocess.check_output(["gh", "api", path], text=True))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--run-id", required=True, type=int)
    args = parser.parse_args()
    if args.run_id <= 0:
        parser.error("--run-id must be positive")
    run = api(f"repos/{args.repository}/actions/runs/{args.run_id}")
    jobs = api(f"repos/{args.repository}/actions/runs/{args.run_id}/attempts/{run['run_attempt']}/jobs?per_page=100")["jobs"]
    source_sha = validated_build(run, jobs, args.repository)
    artifacts = api(f"repos/{args.repository}/actions/runs/{args.run_id}/artifacts?per_page=100")["artifacts"]
    check_package_artifact(artifacts)
    subprocess.run(["git", "fetch", "--no-tags", "--depth=1", "origin", source_sha],
                   check=True, stdout=sys.stderr, stderr=sys.stderr)
    changes = subprocess.check_output(["git", "diff", "--name-only", "-z", source_sha, "HEAD"])
    check_app_changes(os.fsdecode(path) for path in changes.split(b"\0") if path)
    print(f"run_id={args.run_id}")
    print(f"source_sha={source_sha}")


if __name__ == "__main__":
    try:
        main()
    except ValueError as error:
        raise SystemExit(str(error)) from error
