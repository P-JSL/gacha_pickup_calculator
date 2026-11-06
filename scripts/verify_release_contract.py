#!/usr/bin/env python3
"""Static 1.0 release-contract verification without external dependencies."""
from __future__ import annotations

import re
import subprocess
import tomllib
from pathlib import Path, PurePosixPath

ROOT = Path(__file__).resolve().parents[1]
REQUIRED = {
    "src/probability/rates.rs", "src/probability/banners.rs",
    "src/probability/distributions.rs", "src/probability/fft.rs", "src/domain/games.rs",
    "src/domain/feedback.rs", "src/analysis/observations.rs",
    "src/analysis/tracker.rs", "src/data/catalog.rs",
    "src/data/runtime_updates.rs", "src/data/security.rs",
    "src/app/funding.rs", "src/app/compute.rs", "src/app/state.rs",
    "src/app/controller_session.rs", "src/app/controller_editor.rs",
    "src/app/controller_refunds.rs", "src/app/controller_analytics.rs",
    "src/app/controller_evidence.rs", "src/app/lifecycle.rs",
    "src/ui/navigation.rs", "src/ui/charts.rs", "src/ui/funding_view.rs", "src/ui/dashboard.rs",
    "src/ui/i18n.rs", "tests/v1_contracts.rs", "tests/performance_regression.rs",
}
OLD_FLAT = {
    "src/games.rs", "src/feedback.rs", "src/observations.rs",
    "src/tracker.rs", "src/catalog.rs", "src/security.rs",
    "src/runtime_updates.rs", "src/dashboard_ui.rs", "src/i18n.rs",
}
MACROS = re.compile(r'include(?:_bytes|_str)?!\s*\(\s*"([^"]+)"\s*\)')
MODULE = re.compile(r'#\[path\s*=\s*"([^"]+)"\]\s*(?:pub\s+)?mod\s+[A-Za-z_][A-Za-z_0-9]*')
SOURCE_INCLUDE = re.compile(r'(?<![A-Za-z])include!\s*\(')
# Binary-only modules are implementation details, not an external Rust API.
INTERNAL_MODULES = [ROOT / "src" / folder for folder in ("app", "data", "ui")]
UNRESTRICTED_PUBLIC = re.compile(r"^\s*pub\s+", re.MULTILINE)
SENSITIVE_FILENAMES = {"id_rsa", "id_ed25519", "secrets.json", "credentials.json", ".env"}
SENSITIVE_EXTENSIONS = {".pem", ".p12", ".pfx", ".key", ".kdbx"}
DOC_LINK = re.compile(r'\]\(([^)]+)\)')


def check() -> int:
    errors: list[str] = []
    version = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["package"]["version"]
    lock = tomllib.loads((ROOT / "Cargo.lock").read_text(encoding="utf-8"))
    packages = [p for p in lock["package"] if p["name"] == "gacha_pickup_calculator"]
    if len(packages) != 1 or packages[0]["version"] != version:
        errors.append("Cargo.toml and Cargo.lock must have the same package version")
    if not (ROOT / f"RELEASE_NOTES_v{version}.md").is_file():
        errors.append(f"Missing release notes for v{version}")
    for path in REQUIRED:
        if not (ROOT / path).is_file():
            errors.append(f"Missing required module: {path}")
    for path in OLD_FLAT:
        if (ROOT / path).exists():
            errors.append(f"Obsolete flat module still exists: {path}")
    inspected = 0
    for path in (ROOT / "src").rglob("*.rs"):
        text = path.read_text(encoding="utf-8")
        if any(folder in path.parents for folder in INTERNAL_MODULES):
            for match in UNRESTRICTED_PUBLIC.finditer(text):
                line = text.count("\n", 0, match.start()) + 1
                errors.append(f"Binary-private module exports unrestricted pub: {path.relative_to(ROOT)}:{line}")
        if SOURCE_INCLUDE.search(text):
            errors.append(f"Legacy source include! in {path.relative_to(ROOT)}")
        for pattern in (MACROS, MODULE):
            for found in pattern.finditer(text):
                reference = (path.parent / found.group(1)).resolve()
                inspected += 1
                if not reference.is_file() or not reference.is_relative_to(ROOT):
                    errors.append(f"Invalid module or embedded file: {path.relative_to(ROOT)} -> {found.group(1)}")
    checked_links = 0
    for path in [ROOT / "README.md", *sorted((ROOT / "docs").rglob("*.md")),
                 *sorted(ROOT.glob("RELEASE_NOTES_*.md"))]:
        content = path.read_text(encoding="utf-8")
        for match in DOC_LINK.finditer(content):
            link = match.group(1).strip().split("#", 1)[0].split("?", 1)[0]
            if not link or link.startswith(("https://", "http://", "mailto:", "data:")):
                continue
            checked_links += 1
            resolved = (path.parent / link).resolve()
            if not resolved.is_file() or not resolved.is_relative_to(ROOT):
                errors.append(f"Broken local document link: {path.relative_to(ROOT)} -> {link}")
    # A Git release archive includes every tracked file. Refuse obvious private
    # material before the source ZIP is created, without scanning its history.
    if (ROOT / ".git").exists():
        tracked = subprocess.run(["git", "ls-files", "-z"], cwd=ROOT,
            check=True, capture_output=True).stdout.decode("utf-8").split("\0")
        for raw in filter(None, tracked):
            path = PurePosixPath(raw)
            if (path.name.lower() in SENSITIVE_FILENAMES
                or path.suffix.lower() in SENSITIVE_EXTENSIONS
                or any(part == ".ssh" or part.startswith(".env.") for part in path.parts)):
                errors.append(f"Potential private material tracked for source release: {raw}")
    for failure in errors:
        print(f"ERROR: {failure}")
    print(f"Verified v{version}: {len(REQUIRED)} modules; {inspected} Rust paths; "
          f"{checked_links} relative documentation links; errors={len(errors)}")
    return int(bool(errors))


if __name__ == "__main__":
    raise SystemExit(check())
