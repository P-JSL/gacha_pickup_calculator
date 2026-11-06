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
PINNED_URLS = frozenset(PINNED_SOURCES.values())


def approved_pinned_url(url):
    return url in PINNED_URLS


def validate_pinned_identity(game, role, ident, url):
    return url not in PINNED_URLS or PINNED_SOURCES.get((game, role, ident)) == url
