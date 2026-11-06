#!/usr/bin/env python3
"""Build a bounded, sanitized, offline-only icon package from catalog sources.

All external image bytes are downloaded exclusively in CI, never at app runtime.
Only HTTPS raw.githubusercontent.com is permitted. Files are re-encoded as PNG
and verified before publication. No downloaded executable/archive is processed.
"""
from __future__ import annotations

import concurrent.futures
import hashlib
import io
from pathlib import Path
import ssl
import urllib.error
import urllib.parse
import urllib.request
import warnings

from PIL import Image, ImageDraw, UnidentifiedImageError
from pinned_icon_sources import approved_pinned_url, validate_pinned_identity

CATALOG = Path("src/catalog_data.tsv")
ROOT = Path("dist/Image")
GAME_DIR = {"hsr": "HSR", "wuwa": "WutheringWaves",
            "sora": "StellaSora", "nte": "Neverness"}
MAX_INPUT_BYTES = 8 * 1024 * 1024
MAX_PIXELS = 8_000_000
MAX_EDGE = 4096
EXPECTED_FORMATS = {"PNG", "JPEG"}
HOSTS = {"raw.githubusercontent.com"}
APPROVED_PATH_PREFIXES = (
    "/Mar-7th/StarRailRes/",
    "/ryanbenson/wuthering-waves-assets/",
    "/JforPlay/sstoy/",
    "/Waifus-Grace/NTE_Assets/",
)
Image.MAX_IMAGE_PIXELS = MAX_PIXELS
warnings.simplefilter("error", Image.DecompressionBombWarning)


def approved_source_url(url: str) -> bool:
    if not url or len(url) > 512 or any(ord(ch) < 32 or ord(ch) == 127 for ch in url):
        return False
    if approved_pinned_url(url):
        return True
    try:
        parsed = urllib.parse.urlsplit(url)
        return (
            parsed.scheme == "https"
            and parsed.hostname in HOSTS
            and parsed.port in (None, 443)
            and parsed.username is None and parsed.password is None
            and not parsed.query and not parsed.fragment
            and any(parsed.path.startswith(prefix) for prefix in APPROVED_PATH_PREFIXES)
            and parsed.path.lower().endswith((".png", ".jpg", ".jpeg"))
            and all(part not in ("", ".", "..") and
                    all(ch.isascii() and (ch.isalnum() or ch in "-._()") for ch in part)
                    for part in parsed.path[1:].split("/"))
        )
    except ValueError:
        return False


class NoUntrustedRedirects(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        if not approved_source_url(newurl):
            raise ValueError("Image redirect blocked")
        return super().redirect_request(req, fp, code, msg, headers, newurl)


OPENER = urllib.request.build_opener(
    urllib.request.HTTPSHandler(context=ssl.create_default_context()),
    NoUntrustedRedirects(),
)


def entries():
    seen = set()
    names = set()
    urls = set()
    for row in CATALOG.read_text(encoding="utf-8").splitlines():
        if not row or row.startswith("#"):
            continue
        columns = row.split("\t")
        if len(columns) != 6:
            raise ValueError("Invalid catalog row")
        game, role, ident, name, rarity, url = columns
        if game not in GAME_DIR or role not in ("c", "w"):
            raise ValueError("Unexpected catalog type")
        if not name.strip() or name.startswith("{") or name.isdecimal():
            raise ValueError(f"Unresolved character/weapon name: {game}/{ident}")
        if rarity not in ("1", "2", "3", "4", "5"):
            raise ValueError(f"Invalid rarity: {game}/{ident}")
        visible_name = (game, role, name)
        if visible_name in names:
            raise ValueError(f"Duplicate visible identity: {visible_name}")
        names.add(visible_name)
        if not ident or any(not (c.isascii() and (c.isalnum() or c in "_-"))
                            for c in ident):
            raise ValueError("Unsafe image identifier")
        if url:
            if not approved_source_url(url) or not validate_pinned_identity(game, role, ident, url):
                raise ValueError(f"Unapproved or wrong-identity source for {game}/{ident}")
            identity = (game, role, url)
            if identity in urls:
                raise ValueError(f"Reused image source for distinct identities: {identity}")
            urls.add(identity)
        output = ROOT / GAME_DIR[game] / f"{role}-{ident}.png"
        if output in seen:
            raise ValueError(f"Duplicate image destination: {output}")
        seen.add(output)
        yield url, output


def fetch_and_normalize(item):
    url, destination = item
    if not url:
        # Never substitute another character's photo. Distinguish all missing
        # assets with the character's own stable ID rather than a shared "NEW".
        im = Image.new("RGBA", (112, 112), (22, 49, 67, 255))
        draw = ImageDraw.Draw(im)
        draw.rectangle((2, 2, 109, 109), outline=(75, 173, 198, 255), width=2)
        draw.text((40, 33), "NO ART", fill=(239, 246, 250, 255))
        ident = destination.stem[2:].replace("upcoming_", "")
        label = ident[:13].upper()
        box = draw.textbbox((0, 0), label)
        draw.text(((112 - (box[2] - box[0])) // 2, 58), label,
                  fill=(136, 215, 225, 255))
        out = io.BytesIO()
        im.save(out, format="PNG")
        normalized = out.getvalue()
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(normalized)
        return destination, hashlib.sha256(normalized).hexdigest()
    request = urllib.request.Request(
        url, headers={"User-Agent": "gacha-pickup-calculator-build/0.1"}
    )
    with OPENER.open(request, timeout=18) as response:
        kind = response.headers.get_content_type()
        if kind not in ("image/png", "image/jpeg", "application/octet-stream") and not (
                approved_pinned_url(url) and kind == "image/webp"):
            raise ValueError(f"Unsupported content type {kind}")
        raw = response.read(MAX_INPUT_BYTES + 1)
    if not raw or len(raw) > MAX_INPUT_BYTES:
        raise ValueError("Image response empty or too large")
    # Ignore misleading file names and MIME labels: decode only explicit formats.
    with Image.open(io.BytesIO(raw)) as im:
        if im.format not in EXPECTED_FORMATS and not (
                approved_pinned_url(url) and im.format == "WEBP"):
            raise ValueError(f"Rejected image format {im.format}")
        w, h = im.size
        if min(w, h) < 1 or max(w, h) > MAX_EDGE or w * h > MAX_PIXELS:
            raise ValueError("Image dimensions exceed limit")
        im.load()
        rgba = im.convert("RGBA")
        rgba.thumbnail((112, 112), Image.Resampling.LANCZOS)
        output = io.BytesIO()
        rgba.save(output, format="PNG", optimize=True)
        normalized = output.getvalue()
    if len(normalized) > 512 * 1024:
        raise ValueError("Normalized icon unexpectedly large")
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_bytes(normalized)
    return destination, hashlib.sha256(normalized).hexdigest()


def main():
    ROOT.mkdir(parents=True, exist_ok=True)
    jobs = list(entries())
    completed = []
    failures = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
        futures = {pool.submit(fetch_and_normalize, job): job for job in jobs}
        for future in concurrent.futures.as_completed(futures):
            job = futures[future]
            try:
                completed.append(future.result())
            except (OSError, ValueError, UnidentifiedImageError, urllib.error.URLError,
                    Image.DecompressionBombWarning, Image.DecompressionBombError) as exc:
                failures.append((str(job[1]), str(exc)))
    for path, reason in sorted(failures):
        print(f"IMAGE FAILED: {path}: {reason}")
    # Fail closed: a release must not claim to ship all icons if any are missing.
    if failures or len(completed) != len(jobs):
        raise SystemExit(f"Offline package incomplete: {len(completed)}/{len(jobs)} images")
    # Source URLs may differ while serving identical incorrect artwork. Detect
    # byte-identical PNGs across distinct characters/objects in a game and role.
    # This is a strong but not exhaustive identity check: perceptual matches or
    # mislabeled unique portraits still require manual source review.
    hashes = {}
    # The different elemental forms of the same protagonist can legitimately
    # share a portrait. Explicitly permit only that exact observed Rover pair.
    same_subject_variants = {
        frozenset({"c-1502.png", "c-1604.png"}),
    }
    unexpected_duplicates = []
    for path, digest in completed:
        kind = path.name.split("-", 1)[0]
        key = (path.parent.name, kind, digest)
        if key in hashes:
            prior = hashes[key]
            if not (path.parent.name == "WutheringWaves" and
                    frozenset({prior.name, path.name}) in same_subject_variants):
                unexpected_duplicates.append((str(prior), str(path)))
        else:
            hashes[key] = path
    if unexpected_duplicates:
        for prior, current in sorted(unexpected_duplicates):
            print(f"UNEXPECTED DUPLICATE ART: {prior} and {current}")
        raise SystemExit(f"Duplicate packaged artwork: {len(unexpected_duplicates)} collisions")
    manifest = ROOT / "SHA256SUMS.txt"
    manifest.write_text(
        "".join(f"{digest}  {path.relative_to(ROOT).as_posix()}\n"
                for path, digest in sorted(completed)),
        encoding="utf-8",
    )
    # Re-read manifest and bytes to detect accidental packaging mixups.
    for path, digest in completed:
        if hashlib.sha256(path.read_bytes()).hexdigest() != digest:
            raise SystemExit(f"Image hash mismatch: {path}")
    print(f"Validated and re-encoded {len(completed)} images into {ROOT}")


if __name__ == "__main__":
    main()
