#!/usr/bin/env python3
"""One-time, fail-closed offline vendoring of the 4.7 images verified by ID.

Downloads precisely three immutable image URLs to the checked-in repository.
Source authenticity is bounded by PINNED_SOURCES and manual reference review;
this script does not assert semantic identity for unrelated gallery images.
"""
from __future__ import annotations
import hashlib
import io
import ssl
import urllib.request
from pathlib import Path
from PIL import Image, UnidentifiedImageError
from pinned_icon_sources import PINNED_SOURCES, REFERENCE_PAGES

ROOT = Path("Image")
CATALOG = Path("src/catalog_data.tsv")
MANIFEST = ROOT / "SHA256SUMS.txt"
ALLOWED_FORMATS = {"PNG", "JPEG", "WEBP"}
MAX_SOURCE_BYTES = 1_000_000


class ForbidRedirects(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, msg, headers, newurl):
        raise ValueError(f"Unexpected asset redirect: HTTP {code}")


OPENER = urllib.request.build_opener(
    urllib.request.HTTPSHandler(context=ssl.create_default_context()),
    ForbidRedirects(),
)


def main():
    catalog = {}
    for line in CATALOG.read_text(encoding="utf-8").splitlines():
        if line and not line.startswith("#"):
            game, role, ident, name, rarity, source = line.split("\t")
            catalog[(game, role, ident)] = source

    manifest = {}
    for line in MANIFEST.read_text(encoding="utf-8").splitlines():
        digest, rel = line.split("  ", 1)
        if rel in manifest:
            raise ValueError(f"Duplicate checksum path: {rel}")
        manifest[rel] = digest

    if len(manifest) != 679:
        raise ValueError(f"Unexpected image manifest count: {len(manifest)}")

    results = {}
    for key, source in PINNED_SOURCES.items():
        if catalog.get(key) != source:
            raise ValueError(f"Unreviewed catalog URL or missing item: {key}")
        game, role, ident = key
        if game != "hsr":
            raise ValueError(f"Unsupported game in 4.7 review: {key}")
        relative = f"HSR/{role}-{ident}.png"
        path = ROOT / relative
        if relative not in manifest or not path.is_file() or path.is_symlink():
            raise ValueError(f"Missing checked-in icon: {relative}")
        if hashlib.sha256(path.read_bytes()).hexdigest() != manifest[relative]:
            raise ValueError(f"Pre-existing icon checksum mismatch: {relative}")
        request = urllib.request.Request(
            source, headers={
                "User-Agent": "Mozilla/5.0 (compatible; GachaPickupIconAuditor/1.0)",
                "Accept": "image/webp,image/png,image/jpeg",
            },
        )
        with OPENER.open(request, timeout=25) as response:
            if response.geturl() != source:
                raise ValueError("Asset URL redirected unexpectedly")
            kind = response.headers.get_content_type()
            if kind not in ("image/webp", "image/png", "image/jpeg",
                            "application/octet-stream"):
                raise ValueError(f"Invalid image media type: {kind}")
            data = response.read(MAX_SOURCE_BYTES + 1)
        if not 40 <= len(data) <= MAX_SOURCE_BYTES:
            raise ValueError(f"Invalid source image size: {key}")
        with Image.open(io.BytesIO(data)) as image:
            if image.format not in ALLOWED_FORMATS:
                raise ValueError(f"Untrusted image format for {key}: {image.format}")
            if not 64 <= image.width <= 2048 or not 64 <= image.height <= 2048:
                raise ValueError(f"Unexpected image dimensions for {key}: {image.size}")
            image.load()
            rgba = image.convert("RGBA")
            rgba.thumbnail((112, 112), Image.Resampling.LANCZOS)
            if rgba.getbbox() is None:
                raise ValueError(f"Transparent/blank image: {key}")
            result = io.BytesIO()
            rgba.save(result, format="PNG", optimize=True)
            normalized = result.getvalue()
        if len(normalized) > 512 * 1024:
            raise ValueError(f"Normalized image too large: {key}")
        digest = hashlib.sha256(normalized).hexdigest()
        if digest == manifest[relative]:
            raise ValueError(f"New icon is identical to previous placeholder: {key}")
        results[relative] = (normalized, digest, REFERENCE_PAGES[key])

    # Prepare all bytes before writing: no partial update on a failed download.
    for relative, (binary, digest, reference) in results.items():
        (ROOT / relative).write_bytes(binary)
        manifest[relative] = digest
        print(f"VERIFIED {relative} SHA256={digest} SOURCE={reference}")

    MANIFEST.write_text(
        "".join(f"{digest}  {relative}\n"
                for relative, digest in sorted(manifest.items())),
        encoding="utf-8",
    )
    print(f"Updated {len(results)} authentic game item images; "
          f"preserved {len(manifest) - len(results)} other image files.")


if __name__ == "__main__":
    main()
