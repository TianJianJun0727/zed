import copy
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location(
    "package_source", Path(__file__).resolve().parents[1] / "check-macos-package-source.py"
)
source = importlib.util.module_from_spec(spec)
spec.loader.exec_module(source)

# These are the validations required before an app can be reused, independently
# of the validator's own list.
REQUIRED_VALIDATION = (
    "Verify upstream stable version", "Check application types", "Dependency licenses",
    "Build application and CLI", "Localization coverage and schema contracts",
    "Check test targets", "Local editor UI contracts", "Local project regression tests",
    "Settings migration tests", "Updater contract tests",
)


class PackageSourceTests(unittest.TestCase):
    def setUp(self):
        self.run = {
            "status": "completed", "conclusion": "success", "event": "push",
            "path": ".github/workflows/local-editor-macos.yml", "head_branch": "main",
            "head_repository": {"full_name": "owner/zed-lite"}, "head_sha": "a" * 40,
        }
        self.jobs = [{"name": "build", "conclusion": "success", "steps": [
            {"name": name, "status": "completed", "conclusion": "success"}
            for name in REQUIRED_VALIDATION
        ]}]

    def validate(self):
        return source.validated_build(self.run, self.jobs, "owner/zed-lite")

    def test_full_native_validation_is_accepted(self):
        self.assertEqual(self.validate(), "a" * 40)
        source.check_app_changes(["README.md", "script/install-bundled-git-macos"])

    def test_each_missing_or_skipped_native_validation_is_rejected(self):
        for name in REQUIRED_VALIDATION:
            with self.subTest(name=name):
                original = copy.deepcopy(self.jobs)
                step = next(step for step in self.jobs[0]["steps"] if step["name"] == name)
                step["conclusion"] = "skipped"
                with self.assertRaises(ValueError):
                    self.validate()
                self.jobs = original
        self.jobs[0]["steps"] = []
        with self.assertRaises(ValueError):
            self.validate()

    def test_unsuccessful_or_running_build_is_rejected(self):
        for key, value in (("status", "in_progress"), ("conclusion", "failure"), ("conclusion", "cancelled")):
            original = self.run.copy()
            self.run[key] = value
            with self.assertRaises(ValueError):
                self.validate()
            self.run = original

    def test_foreign_repository_branch_workflow_or_event_is_rejected(self):
        for key, value in (("head_repository", {"full_name": "fork/zed-lite"}),
                           ("head_branch", "feature"), ("path", "unrelated.yml"),
                           ("event", "pull_request"), ("head_sha", "--unsafe")):
            original = self.run.copy()
            self.run[key] = value
            with self.assertRaises(ValueError):
                self.validate()
            self.run = original

    def test_application_catalog_assets_and_build_inputs_cannot_change(self):
        for path in ("crates/editor/src/editor.rs", "crates/zed_i18n/locales/zh-CN.toml",
                     "assets/settings/default.json", "Cargo.lock", "rust-toolchain.toml"):
            with self.subTest(path=path), self.assertRaises(ValueError):
                source.check_app_changes([path])

    def test_expired_missing_or_ambiguous_package_artifact_is_rejected(self):
        valid = {"name": "Zed-Lite-macOS-arm64", "expired": False}
        source.check_package_artifact([valid])
        for artifacts in ([], [{**valid, "expired": True}], [{"name": valid["name"]}], [valid, valid],
                          [{"name": "compiler-diagnostics", "expired": False}]):
            with self.subTest(artifacts=artifacts), self.assertRaises(ValueError):
                source.check_package_artifact(artifacts)

    def test_no_native_job_or_ambiguous_native_job_is_rejected(self):
        self.jobs.append(copy.deepcopy(self.jobs[0]))
        with self.assertRaises(ValueError):
            self.validate()
        self.jobs = []
        with self.assertRaises(ValueError):
            self.validate()


if __name__ == "__main__":
    unittest.main()
