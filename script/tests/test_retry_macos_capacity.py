import copy
import importlib.util
from pathlib import Path
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location(
    "retry_capacity", Path(__file__).resolve().parents[1] / "retry-macos-capacity.py"
)
capacity = importlib.util.module_from_spec(spec)
spec.loader.exec_module(capacity)


class CapacityRetryTests(unittest.TestCase):
    def setUp(self):
        self.run = {
            "status": "completed", "conclusion": "failure", "run_attempt": 1,
            "path": capacity.WORKFLOW_PATH, "head_branch": "main", "head_sha": "current",
            "head_repository": {"full_name": "owner/zed-lite"},
        }
        self.jobs = [{"name": "build", "steps": [], "conclusion": "cancelled", "labels": ["macos-26"]}]
        self.annotations = [
            {"message": "The job was not acquired by Runner of type hosted even after multiple attempts"},
            {"message": "Due to capacity constraints, jobs targeting macOS arm64 runners may experience longer queue times."},
        ]

    def reason(self, sha="current"):
        return capacity.retry_reason(self.run, self.jobs, self.annotations, "owner/zed-lite", sha)

    def test_actual_capacity_failure_allows_two_retries(self):
        for attempt in (1, 2):
            self.run["run_attempt"] = attempt
            self.assertIsNone(self.reason())
        self.run["run_attempt"] = 3
        self.assertIsNotNone(self.reason())

    def test_executed_steps_and_missing_capacity_are_not_retried(self):
        self.jobs[0]["steps"] = [{"name": "Compile", "conclusion": "failure"}]
        self.assertIsNotNone(self.reason())
        self.jobs[0]["steps"] = []
        self.annotations.pop()
        self.assertIsNotNone(self.reason())

    def test_stale_fork_cancelled_and_other_workflow_are_not_retried(self):
        self.assertIsNotNone(self.reason("newer"))
        for key, value in (("conclusion", "cancelled"), ("head_branch", "feature"),
                           ("path", "other.yml"), ("head_repository", {"full_name": "fork/repo"})):
            original = copy.deepcopy(self.run)
            self.run[key] = value
            self.assertIsNotNone(self.reason())
            self.run = original
        self.jobs[0]["conclusion"] = "skipped"
        self.assertIsNotNone(self.reason())

    def test_dry_run_and_new_revision_during_backoff_do_not_write(self):
        arguments = ["retry", "--repository", "owner/zed-lite", "--run-id", "123"]
        with patch("sys.argv", arguments + ["--dry-run"]), patch.object(capacity, "inspect_run", return_value=(self.run, None)), patch.object(capacity, "api") as api, patch.object(capacity.time, "sleep") as sleep:
            capacity.main()
            api.assert_not_called()
            sleep.assert_not_called()
        with patch("sys.argv", arguments), patch.object(capacity, "inspect_run", side_effect=[(self.run, None), (self.run, "A newer main revision exists")]), patch.object(capacity, "api") as api, patch.object(capacity.time, "sleep"):
            capacity.main()
            api.assert_not_called()

    def test_only_failed_jobs_are_requested_after_recheck(self):
        with patch("sys.argv", ["retry", "--repository", "owner/zed-lite", "--run-id", "123"]), patch.object(capacity, "inspect_run", return_value=(self.run, None)), patch.object(capacity, "api") as api, patch.object(capacity.time, "sleep"):
            capacity.main()
            api.assert_called_once_with("repos/owner/zed-lite/actions/runs/123/rerun-failed-jobs", "POST")


if __name__ == "__main__":
    unittest.main()
