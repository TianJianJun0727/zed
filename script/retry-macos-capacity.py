#!/usr/bin/env python3
"""Retry an unstarted macOS job only when GitHub reports hosted runner capacity errors."""

import argparse
import json
import subprocess
import time

WORKFLOW_PATH = ".github/workflows/local-editor-macos.yml"
MAX_ATTEMPTS = 3


def retry_reason(run, jobs, annotations, repository, main_sha):
    if run.get("status") != "completed" or run.get("conclusion") != "failure":
        return "Run did not finish with failure"
    if (run.get("path") != WORKFLOW_PATH or run.get("head_branch") != "main"
            or run.get("head_repository", {}).get("full_name") != repository):
        return "Run is not this repository's main build"
    if run.get("head_sha") != main_sha:
        return "A newer main revision exists"
    if not 1 <= run.get("run_attempt", 0) < MAX_ATTEMPTS:
        return "Automatic retry limit reached"
    build_jobs = [job for job in jobs if job.get("name") == "build"]
    if len(build_jobs) != 1:
        return "Expected one macOS build job"
    job = build_jobs[0]
    if (job.get("steps") or job.get("conclusion") not in ("failure", "cancelled")
            or "macos-26" not in job.get("labels", [])):
        return "macOS job executed steps or was not a failed runner allocation"
    messages = "\n".join(item.get("message", "") for item in annotations).lower()
    if "not acquired" not in messages or "capacity constraints" not in messages:
        return "GitHub did not report runner capacity exhaustion"
    return None


def api(path, method="GET"):
    return json.loads(subprocess.check_output(
        ["gh", "api", "--method", method, path], text=True
    ) or "null")


def inspect_run(repository, run_id):
    run = api(f"repos/{repository}/actions/runs/{run_id}")
    main_sha = api(f"repos/{repository}/git/ref/heads/main")["object"]["sha"]
    attempt = run["run_attempt"]
    jobs = api(f"repos/{repository}/actions/runs/{run_id}/attempts/{attempt}/jobs?per_page=100")["jobs"]
    annotations = []
    for job in jobs:
        if job.get("name") == "build" and not job.get("steps"):
            # Construct the endpoint locally instead of requesting an event-provided URL.
            check_id = int(job["check_run_url"].rsplit("/", 1)[1])
            annotations = api(f"repos/{repository}/check-runs/{check_id}/annotations?per_page=100")
    return run, retry_reason(run, jobs, annotations, repository, main_sha)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--run-id", required=True, type=int)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    run, reason = inspect_run(args.repository, args.run_id)
    if reason:
        print(f"No retry: {reason}")
        return
    attempt = run["run_attempt"]
    if args.dry_run:
        print(f"Eligible: retry attempt {attempt + 1} of {MAX_ATTEMPTS}")
        return
    time.sleep(60 * attempt)
    current, reason = inspect_run(args.repository, args.run_id)
    if reason or current["run_attempt"] != attempt:
        print(f"No retry after backoff: {reason or 'Another retry has already started'}")
        return
    api(f"repos/{args.repository}/actions/runs/{args.run_id}/rerun-failed-jobs", "POST")
    print(f"Requested attempt {attempt + 1} of {MAX_ATTEMPTS} for run {args.run_id}")


if __name__ == "__main__":
    main()
