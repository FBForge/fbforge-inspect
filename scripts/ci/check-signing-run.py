"""Keep the signing runner behind an exact successful build and owner approval."""

import json
import os
import re
import subprocess


REPOSITORY = "FBForge/fbforge-inspect"


def require(value, reason):
    if not value:
        raise RuntimeError(reason)


def validate(run, jobs, environment, commit, branch):
    require(bool(re.fullmatch(r"[a-f0-9]{40}", commit)), "signing_commit_invalid")
    require(branch == "main", "signing_branch_denied")
    require(run.get("head_sha") == commit, "signing_build_commit_mismatch")
    require(run.get("head_branch") == branch, "signing_build_branch_mismatch")
    require(run.get("head_repository", {}).get("full_name") == REPOSITORY, "signing_build_repository_mismatch")
    require(run.get("path") == ".github/workflows/verify.yml", "signing_build_workflow_mismatch")
    require(run.get("event") in ("push", "workflow_dispatch"), "signing_build_event_denied")
    require(run.get("status") == "completed" and run.get("conclusion") == "success", "signing_build_not_successful")
    for name in ("verify", "windows"):
        selected = [job for job in jobs if job.get("name") == name]
        require(len(selected) == 1 and selected[0].get("conclusion") == "success", "signing_" + name + "_validation_missing")
    approvals = [rule for rule in environment.get("protection_rules", []) if rule.get("type") == "required_reviewers"]
    require(len(approvals) == 1, "signing_environment_approval_missing")
    reviewers = approvals[0].get("reviewers", [])
    require(
        len(reviewers) == 1
        and reviewers[0].get("type") == "User"
        and reviewers[0].get("reviewer", {}).get("login", "").lower() == "fbforge",
        "signing_environment_owner_required",
    )
    require(not approvals[0].get("prevent_self_review", False), "signing_owner_cannot_approve_own_run")


def api(path):
    result = subprocess.run(["gh", "api", path], capture_output=True, text=True, check=False)
    require(result.returncode == 0, "signing_github_read_failed")
    return json.loads(result.stdout)


def main():
    require(os.environ.get("GITHUB_REPOSITORY") == REPOSITORY, "signing_repository_denied")
    require(os.environ.get("GITHUB_EVENT_NAME") == "workflow_dispatch", "signing_event_denied")
    branch = os.environ.get("GITHUB_REF", "").removeprefix("refs/heads/")
    run_id = os.environ.get("VALIDATION_RUN", "")
    require(bool(re.fullmatch(r"[1-9][0-9]{0,18}", run_id)), "signing_run_id_invalid")
    run = api(f"repos/{REPOSITORY}/actions/runs/{run_id}")
    jobs = api(f"repos/{REPOSITORY}/actions/runs/{run_id}/jobs?filter=latest&per_page=100")
    require(jobs.get("total_count", 0) <= 100, "signing_job_limit")
    environment = api(f"repos/{REPOSITORY}/environments/windows-signing")
    validate(run, jobs["jobs"], environment, os.environ.get("GITHUB_SHA", ""), branch)
    print("Exact Windows validation and signing approval policy verified.")


if __name__ == "__main__":
    main()
