"""Pinned, identity-reviewed Gachabase image sources (not a general host allowlist).

The three asset URLs correspond to the beta item pages below.
Only explicitly reviewed (game, role, in-game ID) pairs may use these URLs.
Assets are vendored to Image/ by CI, not fetched by the running application.
"""
PINNED_SOURCES = {
    ("hsr", "c", "1511"): "https://i.gachabase.net/-LuzQkoz_xxThpfhIoggZyPwP5qGH2PNb7JagsfT7JE/q%3A50/f%3Awebp/plain/hsr/assets/2be49dbbe7cfe8f77187dcfea51d9863.png",
    ("hsr", "w", "22009"): "https://i.gachabase.net/uBAzqOHCvKf2Lw_wZbXzjj80cTLw831sAmvOKy7ROS8/q%3A50/f%3Awebp/plain/hsr/assets/e4d45ac64a652c8e44f9a7c7da2d4e07.png",
    ("hsr", "w", "23065"): "https://i.gachabase.net/i6VciWWsCxBhjhKd0bBK-E_qf9P-VDTdz-I-8-zUPeY/q%3A50/f%3Awebp/plain/hsr/assets/ff88fd079e093ae22ccf24269eb85a17.png",
}
REFERENCE_PAGES = {
    ("hsr", "c", "1511"): "https://hsr.gachabase.net/characters/1511/aeon-aha/beta",
    ("hsr", "w", "22009"): "https://hsr.gachabase.net/lightcones/22009/see-you-at-the-worlds-end/beta?lang=en",
    ("hsr", "w", "23065"): "https://hsr.gachabase.net/lightcones/23065/upon-the-first-echo-of-aha/beta?lang=ko",
}
# SHA-256 of each reviewed and normalized PNG committed by the 4.7 vendor workflow.
# Intentionally independent of the general Image/SHA256SUMS.txt manifest.
PINNED_PNG_SHA256 = {
    ("hsr", "c", "1511"): "e5cae622d77a3466d2791e9ef0ac291d5072587efc01320e1a4eb94a86791c60",
    ("hsr", "w", "22009"): "565c4bb6e032553b3df5af5c83cd6423549c695d8d8b79d21377eb10b41ad6ea",
    ("hsr", "w", "23065"): "f9c6e6ca3a40478b210b33b3f760bcffb9cb6fdd26f6dedbbfcede7a70b87ed2",
}
PINNED_URLS = frozenset(PINNED_SOURCES.values())


def approved_pinned_url(url):
    return url in PINNED_URLS


def validate_pinned_identity(game, role, ident, url):
    return url not in PINNED_URLS or PINNED_SOURCES.get((game, role, ident)) == url
