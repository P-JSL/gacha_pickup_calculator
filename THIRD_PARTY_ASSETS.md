# Third-party artwork and asset provenance

The repository's original calculator source code is distributed under its root
[MIT license](LICENSE). **The MIT license does not extend to artwork supplied by
game publishers or third parties.** A GitHub repo hosting an image does not itself
establish permission to redistribute that copyrighted image. Users and downstream
distributors are responsible for obtaining any rights required by applicable law.

The checked-in `Image/` directory contains 679 **pre-processed offline PNGs**,
copied from this repository's internally built and malware-scanned GitHub Actions
artifact (run `42149571938`). Every image is enumerated in
[`Image/SHA256SUMS.txt`](Image/SHA256SUMS.txt). The package is a frozen source
snapshot: the standard release CI does not download art from third-party
origin providers. At runtime the app **may fetch updated, repository-approved
PNG files over HTTPS from a verified GitHub commit**, which is separate from
re-collecting external source art. The previously packaged PNGs remain local.

## Provenance of originally collected assets

| Game | Upstream community data repository | Upstream repository code/license | Artwork redistribution rights |
|---|---|---|---|
| Honkai: Star Rail | https://github.com/Mar-7th/StarRailRes | Repository declares GNU AGPL-3.0 | Separate underlying game artwork rights **not established** |
| Wuthering Waves | https://github.com/ryanbenson/wuthering-waves-assets | No recognized license found in repository metadata | **Not established**; upstream collects art through its own sources |
| Stella Sora | https://github.com/JforPlay/sstoy | No recognized license found in repository metadata | **Not established** |
| Neverness to Everness | https://github.com/Waifus-Grace/NTE_Assets | No recognized license found in repository metadata | **Not established** |

Source-asset URLs are retained only as provenance metadata inside
[`src/catalog_data.tsv`](src/catalog_data.tsv). They are **not directly fetched** at runtime or by the ordinary release build;
runtime updates are restricted to the project's verified GitHub snapshot. If re-collecting art is required, a maintainer must
review copyright permissions, verify the upstream ownership and pin the
expected content digests before authorizing an update.

**Do not interpret this file as a license grant on anyone else's artwork.**
The game-related names, icons and images remain associated with their respective
rightsholders. Anyone distributing copies of this public repository or bundled
releases should independently assess whether permission is required.

## Dependency/license checks

- Rust dependencies are specified in `Cargo.toml` and the resolved dependency
  graph is audited by the CI `cargo audit` step.
- CI uses Pillow only for an explicit **manual asset-refresh** procedure; the
  normal release path reads locally vendored PNGs and requires no Pillow.
- This document records license statements found in upstream repositories,
  not a legal determination that downstream redistribution is authorized.
