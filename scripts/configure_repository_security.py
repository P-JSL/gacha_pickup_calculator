#!/usr/bin/env python3
"""Preview or apply GitHub main-branch rules with an admin-scoped gh login.

The ChatGPT GitHub integration cannot mutate repository settings. Run this
locally with an authenticated maintainer account after CI is green. No tokens
are accepted on the command line or stored in the repository.
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys

DEFAULT_REPO = "P-JSL/gacha_pickup_calculator"
RULESET_NAME = "Protect main (verified PR and CI)"
CHECKS = [
    "Verify (windows-latest)",
    "Verify (ubuntu-24.04)",
    "Audit and scan checked-in offline images",
    "Capture latest-character GUI for README",
    "CodeQL (rust)",
    "CodeQL (python)",
    "CodeQL (actions)",
]


def gh_api(endpoint: str, payload: dict | None = None, method: str = "GET"):
    command = ["gh", "api", endpoint]
    if payload is not None:
        command += ["--method", method, "--input", "-"]
    call = subprocess.run(
        command, input=json.dumps(payload) if payload is not None else None,
        text=True, capture_output=True, check=False,
    )
    if call.returncode != 0:
        raise RuntimeError(f"gh api {method} {endpoint} failed: {call.stderr.strip()}")
    return json.loads(call.stdout)


def ruleset(approvals: int) -> dict:
    return {
        "name": RULESET_NAME,
        "target": "branch",
        "enforcement": "active",
        "bypass_actors": [],
        "conditions": {"ref_name": {"include": ["refs/heads/main"], "exclude": []}},
        "rules": [
            {"type": "deletion"},
            {"type": "non_fast_forward"},
            {"type": "required_signatures"},
            {"type": "pull_request", "parameters": {
                "allowed_merge_methods": ["squash"],
                "required_approving_review_count": approvals,
                "dismiss_stale_reviews_on_push": approvals > 0,
                "require_code_owner_review": False,
                "require_last_push_approval": approvals > 0,
                "required_review_thread_resolution": True,
            }},
            {"type": "required_status_checks", "parameters": {
                "strict_required_status_checks_policy": True,
                "do_not_enforce_on_create": False,
                "required_status_checks": [{"context": name} for name in CHECKS],
            }},
        ],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", default=DEFAULT_REPO)
    parser.add_argument("--approvals", type=int, choices=[0, 1], default=0,
                        help="Use 1 only after a distinct authorized reviewer exists")
    parser.add_argument("--apply", action="store_true",
                        help="Write GitHub ruleset (default: print preview only)")
    args = parser.parse_args()
    if args.repo != DEFAULT_REPO:
        parser.error("Explicitly reviewed repository only; adjust DEFAULT_REPO in the source.")
    payload = ruleset(args.approvals)
    print(json.dumps(payload, ensure_ascii=False, indent=2))
    if not args.apply:
        print("\nPreview only. Apply with --apply using a GitHub account that has Administration:write.")
        return 0
    try:
        existing = gh_api(f"repos/{args.repo}/rulesets")
        matched = [item for item in existing
                   if item.get("name") == RULESET_NAME and item.get("target") == "branch"]
        if len(matched) > 1:
            raise RuntimeError("Multiple matching rulesets; refusing ambiguous update")
        if matched:
            endpoint = f"repos/{args.repo}/rulesets/{matched[0]['id']}"
            result = gh_api(endpoint, payload, "PUT")
        else:
            result = gh_api(f"repos/{args.repo}/rulesets", payload, "POST")
        print(f"Applied rule id={result['id']} enforcement={result['enforcement']}")
        return 0
    except (OSError, ValueError, KeyError, RuntimeError) as exc:
        print(f"Security rule application failed: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
