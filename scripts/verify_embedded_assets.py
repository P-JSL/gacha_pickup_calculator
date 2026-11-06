#!/usr/bin/env python3
"""Offline-only source asset integrity, catalog and signature mapping audit.

No network calls or image decoding. Checks every tracked PNG against the SHA-256
manifest embedded into the Rust executable. More expensive perceptual and
semantic review remains a separately documented maintenance operation.
"""
from __future__ import annotations

import hashlib
import re
import struct
import urllib.parse
from collections import defaultdict
from pathlib import Path

ROOT = Path("Image")
CATALOG = Path("src/catalog_data.tsv")
PAIRS = Path("src/signature_pairs.tsv")
GAME_DIR = {"hsr": "HSR", "wuwa": "WutheringWaves",
            "nte": "Neverness", "sora": "StellaSora"}
PREFIXES = (
    "/Mar-7th/StarRailRes/",
    "/ryanbenson/wuthering-waves-assets/",
    "/JforPlay/sstoy/",
    "/Waifus-Grace/NTE_Assets/",
)
ALLOWED_SHARED_ART = {
    frozenset({"WutheringWaves/c-1502.png", "WutheringWaves/c-1604.png"}),
}
ID_RE = re.compile(r"^[A-Za-z0-9_-]+$")
MAX_PNG_BYTES = 512 * 1024
PNG_MAGIC = b"\x89PNG\r\n\x1a\n"


def approved_url(url: str) -> bool:
    if len(url) > 512 or any(ord(c) < 32 or ord(c) == 127 for c in url):
        return False
    parsed = urllib.parse.urlsplit(url)
    return (
        parsed.scheme == "https"
        and parsed.hostname == "raw.githubusercontent.com"
        and parsed.username is None and parsed.password is None
        and parsed.port in (None, 443)
        and not parsed.query and not parsed.fragment
        and any(parsed.path.startswith(prefix) for prefix in PREFIXES)
        and parsed.path.lower().endswith((".png", ".jpg", ".jpeg"))
        and all(part not in ("", ".", "..") and all(
            ch.isascii() and (ch.isalnum() or ch in "_-().")
            for ch in part) for part in parsed.path[1:].split("/"))
    )


def catalog_entries():
    catalog = {}
    names = set()
    urls = set()
    for lineno, row in enumerate(CATALOG.read_text(encoding="utf-8").splitlines(), 1):
        if not row or row.startswith("#"):
            continue
        parts = row.split("\t")
        if len(parts) != 6:
            raise ValueError(f"{CATALOG}:{lineno}: six columns expected")
        game, role, ident, name, rarity, source = parts
        if game not in GAME_DIR or role not in ("c", "w") or not ID_RE.fullmatch(ident):
            raise ValueError(f"{CATALOG}:{lineno}: invalid identifier")
        if rarity not in ("1", "2", "3", "4", "5"):
            raise ValueError(f"{CATALOG}:{lineno}: invalid rarity")
        if not name.strip() or name.startswith("{") or name.isdecimal():
            raise ValueError(f"{CATALOG}:{lineno}: unresolved display name")
        id_key = (game, role, ident)
        visible_key = (game, role, name)
        if id_key in catalog or visible_key in names:
            raise ValueError(f"{CATALOG}:{lineno}: duplicate identity")
        catalog[id_key] = parts
        names.add(visible_key)
        if source:
            if not approved_url(source) or (game, role, source) in urls:
                raise ValueError(f"{CATALOG}:{lineno}: disallowed or shared remote source URL")
            urls.add((game, role, source))
    if len(catalog) != 677:
        raise ValueError(f"Expected 677 catalog entries; got {len(catalog)}")
    return catalog


def audit_signature_pairs(catalog):
    seen = set()
    count = 0
    for lineno, line in enumerate(PAIRS.read_text(encoding="utf-8").splitlines(), 1):
        if not line or line.startswith("#"):
            continue
        cols = line.split("\t")
        if len(cols) != 4:
            raise ValueError(f"{PAIRS}:{lineno}: expected four columns")
        game, char_id, weapon_id, source = cols
        if (game, "c", char_id) not in catalog or (game, "w", weapon_id) not in catalog:
            raise ValueError(f"{PAIRS}:{lineno}: referenced identity does not exist")
        if catalog[game, "c", char_id][4] != "5" or catalog[game, "w", weapon_id][4] != "5":
            raise ValueError(f"{PAIRS}:{lineno}: non-five-star pair")
        if not source.startswith("https://") or (game, char_id) in seen:
            raise ValueError(f"{PAIRS}:{lineno}: unsupported source or duplicate")
        seen.add((game, char_id))
        count += 1
    if count != 101:
        raise ValueError(f"Signature catalog count changed unexpectedly: {count}")
    return count


def audit_pngs(catalog):
    expected = {
        f"{GAME_DIR[g]}/{role}-{ident}.png"
        for g, role, ident in catalog
    }
    manifest_path = ROOT / "SHA256SUMS.txt"
    manifest = {}
    for line in manifest_path.read_text(encoding="utf-8").splitlines():
        if "  " not in line:
            raise ValueError("Malformed image checksum manifest")
        digest, key = line.split("  ", 1)
        if not re.fullmatch(r"[0-9a-f]{64}", digest) or key in manifest:
            raise ValueError(f"Invalid manifest entry: {key}")
        manifest[key] = digest
    actual = {p.relative_to(ROOT).as_posix() for p in ROOT.glob("*/*.png")}
    if set(manifest) != expected or actual != expected:
        raise ValueError(f"Manifest/directory/catalog mismatch: expected={len(expected)} "
                         f"manifest={len(manifest)} actual={len(actual)}")
    if {p.relative_to(ROOT).as_posix() for p in ROOT.rglob("*") if p.is_file()} - (expected | {"SHA256SUMS.txt"}):
        raise ValueError("Untracked or unexpected file found in checked-in Image/")
    hashes = defaultdict(list)
    for rel, expected_hash in manifest.items():
        path = ROOT / rel
        if path.is_symlink() or not path.is_file():
            raise ValueError(f"Non-regular image: {path}")
        raw = path.read_bytes()
        if not 24 <= len(raw) <= MAX_PNG_BYTES or not raw.startswith(PNG_MAGIC):
            raise ValueError(f"Invalid PNG header or size: {path}")
        if raw[12:16] != b"IHDR":
            raise ValueError(f"PNG IHDR missing: {path}")
        width, height = struct.unpack(">II", raw[16:24])
        if not 0 < width <= 256 or not 0 < height <= 256:
            raise ValueError(f"Unexpected image size: {path}: {width}x{height}")
        actual_hash = hashlib.sha256(raw).hexdigest()
        if actual_hash != expected_hash:
            raise ValueError(f"Changed artwork bytes: {path}")
        hashes[(rel.split("/", 1)[0], rel.split("/", 1)[1][0], actual_hash)].append(rel)
    for key, files in hashes.items():
        if len(files) > 1 and frozenset(files) not in ALLOWED_SHARED_ART:
            raise ValueError(f"Same artwork bytes for multiple identities: {files}")
    return len(expected)


def main():
    catalog = catalog_entries()
    paired = audit_signature_pairs(catalog)
    images = audit_pngs(catalog)
    print(f"Offline catalog integrity OK: {images} PNGs and {paired} signature mappings, "
          "no image network requests")


if __name__ == "__main__":
    main()
