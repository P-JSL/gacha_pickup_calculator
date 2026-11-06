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

from pinned_icon_sources import (
    approved_pinned_url, validate_pinned_identity, PINNED_SOURCES, PINNED_PNG_SHA256,
)

ROOT = Path("Image")
CATALOG = Path("src/catalog_data.tsv")
PAIRS = Path("src/signature_pairs.tsv")
UNPAIRED = Path("src/unpaired_five_star_targets.tsv")
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
    if approved_pinned_url(url):
        return True
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
            if (not approved_url(source)
                    or not validate_pinned_identity(game, role, ident, source)
                    or (game, role, source) in urls):
                raise ValueError(f"{CATALOG}:{lineno}: disallowed, mismatched or shared source URL")
            urls.add((game, role, source))
    for key, source in PINNED_SOURCES.items():
        if key not in catalog or catalog[key][5] != source:
            raise ValueError(f"Missing or mismatched reviewed icon source for {key}")
    if len(catalog) != 679:
        raise ValueError(f"Expected 679 catalog entries; got {len(catalog)}")
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
    if count != 141:
        raise ValueError(f"Signature catalog count changed unexpectedly: {count}")
    # Every five-star identity must have a real signature or an explicit, justified
    # exception. No newly added unreleased character may silently inherit a
    # random 5-star weapon when its signature has not been announced.
    allowed_reasons = {
        "story_protagonist", "standard_nonlimited",
        "no_exclusive_arc_in_game_snapshot",
        "banner_or_signature_unconfirmed",
    }
    exceptions = {}
    for lineno, line in enumerate(UNPAIRED.read_text(encoding="utf-8").splitlines(), 1):
        if not line or line.startswith("#"):
            continue
        fields = line.split("\t")
        if len(fields) != 4:
            raise ValueError(f"{UNPAIRED}:{lineno}: invalid unpaired entry")
        game, char_id, reason, display_name = fields
        key = (game, char_id)
        if (reason not in allowed_reasons or key in exceptions
                or (game, "c", char_id) not in catalog
                or catalog[game, "c", char_id][3] != display_name
                or catalog[game, "c", char_id][4] != "5"):
            raise ValueError(f"{UNPAIRED}:{lineno}: unexpected unpaired exception")
        exceptions[key] = reason
    unpaired = {(g, char_id) for g, role, char_id in catalog
                if role == "c" and catalog[g, role, char_id][4] == "5"
                and (g, char_id) not in seen}
    if set(exceptions) != unpaired or len(unpaired) != 30:
        raise ValueError(f"Signature coverage mismatch: unexpected={unpaired - set(exceptions)} "
                         f"obsolete={set(exceptions) - unpaired}")
    return count



def audit_pinned_game_sources(catalog):
    """Cross-check catalog identities against independently captured game data.

    Sora: pinned localized Character/Disc tables (IDs and real names).
    NTE: pinned internal ForkItemData icon/quality/exclusivity metadata.
    This checks data existence, not whether an item is a currently available banner.
    """
    hsr_rows = {}
    for line in Path("src/source_snapshot_hsr.tsv").read_text(
            encoding="utf-8").splitlines():
        if not line or line.startswith("#"):
            continue
        role, ident, localized, rarity, path = line.split("\t")
        key = ("hsr", role, ident)
        if key not in catalog or key in hsr_rows:
            raise ValueError(f"HSR source ID missing or duplicated: {ident}")
        current = catalog[key]
        if current[4] != rarity or not path:
            raise ValueError(f"HSR rarity/path mismatch: {ident}")
        # Two March 7th variants and each named/elemental protagonist are
        # deliberately more descriptive than the base game's placeholder.
        if role == "w" and current[3] != localized:
            raise ValueError(f"HSR Light Cone name mismatch: {ident}")
        if role == "c" and not (
                current[3] == localized or
                (ident in {"1001", "1224"} and current[3].startswith(localized + " (")) or
                (ident in {str(n) for n in range(8001, 8011)}
                 and localized == "{NICKNAME}" and current[3].startswith("개척자 ("))):
            raise ValueError(f"HSR character name mismatch: {ident}")
        hsr_rows[key] = True
    extra_hsr = {key for key in catalog if key[0] == "hsr"} - set(hsr_rows)
    # Explicitly allow only the announced characters with provisional UI IDs.
    if extra_hsr != {("hsr", "c", "upcoming_ellen_joe"),
                     ("hsr", "c", "upcoming_astra_yao")}:
        raise ValueError(f"HSR unverified game data IDs changed: {sorted(extra_hsr)}")
    if len([k for k in hsr_rows if k[1] == "w"]) != 172:
        raise ValueError("HSR Light Cone coverage unexpectedly changed")

    wuwa_checked = {}
    for line in Path("src/source_snapshot_wuwa_weapons.tsv").read_text(
            encoding="utf-8").splitlines():
        if not line or line.startswith("#"):
            continue
        ident, name = line.split("\t")
        key = ("wuwa", "w", ident)
        if key not in catalog or key in wuwa_checked or catalog[key][3] != name:
            raise ValueError(f"WuWa secondary weapon ID/name mismatch: {ident} {name}")
        wuwa_checked[key] = True
    if len(wuwa_checked) != 90:
        raise ValueError(f"WuWa independently cross-checked names changed: {len(wuwa_checked)}")

    sora_rows = {}
    for line in Path("src/source_snapshot_sora.tsv").read_text(
            encoding="utf-8").splitlines():
        if not line or line.startswith("#"):
            continue
        role, ident, name, available, visible = line.split("\t")
        if role not in ("c", "w") or available != "Y" or visible != "Y":
            raise ValueError(f"Unreleased/hidden Sora source entry: {ident}")
        key = ("sora", role, ident)
        if key in sora_rows or key not in catalog or catalog[key][3] != name:
            raise ValueError(f"Sora game source ID/name mismatch: {ident} {name}")
        sora_rows[key] = True
    if set(sora_rows) != {k for k in catalog if k[0] == "sora"}:
        raise ValueError("Sora catalog has uncrosschecked or missing rows")

    nte_rows = {}
    nte_exclusive = set()
    for line in Path("src/source_snapshot_nte_arcs.tsv").read_text(
            encoding="utf-8").splitlines():
        if not line or line.startswith("#"):
            continue
        ident, internal_key, stem, rarity, exclusive = line.split("\t")
        key = ("nte", "w", ident)
        if key in nte_rows or key not in catalog:
            raise ValueError(f"NTE internal arc ID not in catalog: {ident}")
        actual = catalog[key]
        if not internal_key.startswith("fork_") or actual[4] != rarity:
            raise ValueError(f"NTE fork rarity or internal ID mismatch: {ident}")
        if not actual[5].endswith("/" + stem + ".png"):
            raise ValueError(f"NTE arc icon does not match game data: {ident}")
        nte_rows[key] = True
        if exclusive != "-":
            for char_id in exclusive.split(","):
                if ("nte", "c", char_id) in catalog:
                    nte_exclusive.add((char_id, ident))
    if set(nte_rows) != {k for k in catalog if k[0] == "nte" and k[1] == "w"}:
        raise ValueError("NTE fork catalog has unverified internal equipment")
    configured_nte = {
        (fields[1], fields[2])
        for line in PAIRS.read_text(encoding="utf-8").splitlines()
        if line and not line.startswith("#")
        for fields in [line.split("\t")] if fields[0] == "nte"
    }
    if nte_exclusive != configured_nte:
        raise ValueError(f"NTE exclusive-character matching differs from game table: "
                         f"missing={nte_exclusive - configured_nte}; "
                         f"unsupported={configured_nte - nte_exclusive}")
    return len(hsr_rows), len(wuwa_checked), len(sora_rows), len(nte_rows), len(nte_exclusive)


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
    for (game, role, ident), checked_hash in PINNED_PNG_SHA256.items():
        rel = f"{GAME_DIR[game]}/{role}-{ident}.png"
        if manifest.get(rel) != checked_hash:
            raise ValueError(f"Reviewed character/light-cone image changed: {rel}")
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
    hsr_checked, wuwa_checked, sora_checked, nte_checked, nte_mapped = audit_pinned_game_sources(catalog)
    images = audit_pngs(catalog)
    print(f"Offline catalog integrity OK: {images} PNGs and {paired} signatures; "
          f"{hsr_checked} HSR IDs, {wuwa_checked} independent WuWa weapons, "
          f"{sora_checked} Sora game IDs, {nte_checked} NTE forks, "
          f"{nte_mapped} exclusive NTE mappings; no image network requests")


if __name__ == "__main__":
    main()
