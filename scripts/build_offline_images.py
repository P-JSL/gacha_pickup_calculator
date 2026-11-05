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

CATALOG = Path("src/catalog_data.tsv")
ROOT = Path("dist/Image")
GAME_DIR = {"hsr": "HSR", "wuwa": "WutheringWaves",
            "sora": "StellaSora", "nte": "Neverness"}
MAX_INPUT_BYTES = 2 * 1024 * 1024
MAX_PIXELS = 8_000_000
MAX_EDGE = 4096
EXPECTED_FORMATS = {"PNG", "JPEG"}
HOSTS = {"raw.githubusercontent.com"}
Image.MAX_IMAGE_PIXELS = MAX_PIXELS
warnings.simplefilter("error", Image.DecompressionBombWarning)


class NoUntrustedRedirects(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        parsed = urllib.parse.urlsplit(newurl)
        if parsed.scheme != "https" or parsed.hostname not in HOSTS:
            raise ValueError("Image redirect blocked")
        return super().redirect_request(req, fp, code, msg, headers, newurl)


OPENER = urllib.request.build_opener(
    urllib.request.HTTPSHandler(context=ssl.create_default_context()),
    NoUntrustedRedirects(),
)


def entries():
    seen = set()
    for row in CATALOG.read_text(encoding="utf-8").splitlines():
        if not row or row.startswith("#"):
            continue
        columns = row.split("\t")
        if len(columns) != 6:
            raise ValueError("Invalid catalog row")
        game, role, ident, _, _, url = columns
        if game not in GAME_DIR or role not in ("c", "w"):
            raise ValueError("Unexpected catalog type")
        if not ident or any(not (c.isascii() and (c.isalnum() or c in "_-"))
                            for c in ident):
            raise ValueError("Unsafe image identifier")
        if url:
            parsed = urllib.parse.urlsplit(url)
            if parsed.scheme != "https" or parsed.hostname not in HOSTS:
                raise ValueError("Unapproved image origin")
            if parsed.username or parsed.password or parsed.port not in (None, 443):
                raise ValueError("Credentials or nonstandard port in image URL")
        output = ROOT / GAME_DIR[game] / f"{role}-{ident}.png"
        if output in seen:
            raise ValueError(f"Duplicate image destination: {output}")
        seen.add(output)
        yield url, output


def fetch_and_normalize(item):
    url, destination = item
    if not url:
        # Latest officially announced items can lack redistribution-ready portraits.
        # Make this visibly a placeholder, not a misleading image of another item.
        im = Image.new("RGBA", (112, 112), (22, 49, 67, 255))
        draw = ImageDraw.Draw(im)
        draw.rectangle((2, 2, 109, 109), outline=(75, 173, 198, 255), width=2)
        draw.text((40, 42), "NEW", fill=(239, 246, 250, 255))
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
        if kind not in ("image/png", "image/jpeg", "application/octet-stream"):
            raise ValueError(f"Unsupported content type {kind}")
        raw = response.read(MAX_INPUT_BYTES + 1)
    if not raw or len(raw) > MAX_INPUT_BYTES:
        raise ValueError("Image response empty or too large")
    # Ignore misleading file names and MIME labels: decode only explicit formats.
    with Image.open(io.BytesIO(raw)) as im:
        if im.format not in EXPECTED_FORMATS:
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
