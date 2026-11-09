#!/usr/bin/env python3
"""Build small, quality-checked palette PNGs for the distribution ZIP.

The immutable, SHA-256-verified PNGs in Image/ are never modified.
The optimized hashes are embedded into release builds through build.rs.
"""
from __future__ import annotations

import hashlib
import io
from collections import Counter
from pathlib import Path

from PIL import Image, ImageChops, ImageStat

SOURCE = Path("Image")
OUTPUT = Path("dist/Image")
MANIFEST = "SHA256SUMS.txt"
MAX_BYTES = 512 * 1024
# The first passing small image wins. High-complexity pictures stay larger.
PROFILES = ((80, 192), (96, 192), (96, 256), (112, 256))
Image.MAX_IMAGE_PIXELS = 256 * 256


def catalog_files() -> dict[str, str]:
    manifest = {}
    for line in (SOURCE / MANIFEST).read_text(encoding="utf-8").splitlines():
        digest, key = line.split("  ", 1)
        path = Path(key)
        if (
            len(digest) != 64
            or any(c not in "0123456789abcdef" for c in digest)
            or len(path.parts) != 2
            or path.suffix != ".png"
            or path.parts[0] not in {"HSR", "Neverness", "StellaSora", "WutheringWaves"}
            or key in manifest
        ):
            raise ValueError(f"Invalid manifest entry: {key}")
        manifest[key] = digest
    if len(manifest) != 679:
        raise ValueError(f"Expected 679 checked PNGs, got {len(manifest)}")
    if {p.relative_to(SOURCE).as_posix() for p in SOURCE.glob("*/*.png")} != set(manifest):
        raise ValueError("Catalog images and source manifest differ")
    return manifest


def preview_error(source: Image.Image, candidate: Image.Image) -> float:
    """Worst per-channel RMSE when rendered against light/dark UI backgrounds."""
    worst = 0.0
    resized = candidate.resize(source.size, Image.Resampling.LANCZOS)
    for background in ((15, 42, 62), (245, 248, 250)):
        for icon_side in (40, 64):
            def rendered(layer):
                matte = Image.new("RGBA", layer.size, (*background, 255))
                matte.alpha_composite(layer)
                return matte.convert("RGB").resize(
                    (icon_side, icon_side), Image.Resampling.LANCZOS
                )
            baseline = rendered(source)
            preview = rendered(resized)
            worst = max(worst, max(ImageStat.Stat(
                ImageChops.difference(baseline, preview)
            ).rms))
    return worst


def compact(raw: bytes) -> tuple[bytes, str, float]:
    with Image.open(io.BytesIO(raw)) as image:
        if image.format != "PNG" or not 0 < image.width <= 256 or not 0 < image.height <= 256:
            raise ValueError("Source must be a bounded PNG")
        source = image.convert("RGBA")
    selected, strategy, error = raw, "original", 0.0
    for side, colors in PROFILES:
        layer = source.copy()
        layer.thumbnail((side, side), Image.Resampling.LANCZOS)
        indexed = layer.quantize(
            colors=colors,
            method=Image.Quantize.FASTOCTREE,
            dither=Image.Dither.FLOYDSTEINBERG,
        )
        stream = io.BytesIO()
        indexed.save(stream, format="PNG", optimize=True, compress_level=9)
        candidate = stream.getvalue()
        if len(candidate) >= len(selected) * 0.97:
            continue
        with Image.open(io.BytesIO(candidate)) as reloaded:
            if reloaded.format != "PNG":
                raise ValueError("Invalid optimized PNG")
            preview = reloaded.convert("RGBA")
        distortion = preview_error(source, preview)
        if distortion <= 8.5:
            selected, strategy, error = candidate, f"{side}px/{colors} colors", distortion
            break
    return selected, strategy, error


def main() -> None:
    originals = catalog_files()
    if OUTPUT.resolve() == SOURCE.resolve():
        raise ValueError("Output cannot overwrite immutable originals")
    total_before = total_after = 0
    strategies = Counter()
    outputs = {}
    for key, digest in sorted(originals.items()):
        source_path = SOURCE / key
        if source_path.is_symlink() or source_path.stat().st_size > MAX_BYTES:
            raise ValueError(f"Invalid source image: {key}")
        raw = source_path.read_bytes()
        if hashlib.sha256(raw).hexdigest() != digest:
            raise ValueError(f"Original image digest mismatch: {key}")
        optimized, strategy, _error = compact(raw)
        if len(optimized) > MAX_BYTES:
            raise ValueError(f"Oversized optimized image: {key}")
        target = OUTPUT / key
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(optimized)
        outputs[key] = hashlib.sha256(optimized).hexdigest()
        total_before += len(raw)
        total_after += len(optimized)
        strategies[strategy] += 1
    (OUTPUT / MANIFEST).write_text(
        "".join(f"{digest}  {key}\n" for key, digest in sorted(outputs.items())),
        encoding="utf-8",
    )
    if total_after >= total_before * 0.9:
        raise SystemExit(
            f"Optimization insufficient: {total_before} -> {total_after} bytes"
        )
    for key, digest in outputs.items():
        if hashlib.sha256((OUTPUT / key).read_bytes()).hexdigest() != digest:
            raise SystemExit(f"Optimized package digest mismatch: {key}")
    print(
        f"Optimized {len(outputs)} images: {total_before:,} -> {total_after:,} bytes "
        f"({100 * (1 - total_after / total_before):.1f}% physical reduction)"
    )
    print("Strategies:", dict(strategies))


if __name__ == "__main__":
    main()
