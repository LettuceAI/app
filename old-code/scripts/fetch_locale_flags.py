#!/usr/bin/env python3

from __future__ import annotations

import argparse
import sys
import urllib.error
import urllib.request
from pathlib import Path


FLAG_BASE_URL = "https://cdn.jsdelivr.net/gh/lipis/flag-icons@7.3.2/flags/4x3"
DEFAULT_OUTPUT_DIR = Path("src/core/i18n/localesIcons")

# Locale-to-country mapping is an approximation for language display only.
# Some language-to-flag pairings are inherently imperfect or politically loaded.
LOCALE_TO_FLAG = {
    "en": "gb",
    "es": "es",
    "fr": "fr",
    "de": "de",
    "ja": "jp",
    "pl": "pl",
    "pt": "pt",
    "no": "no",
    "id": "id",
    "fil": "ph",
    "nl": "nl",
    "el": "gr",
    "hi": "in",
    "it": "it",
    "vi": "vn",
    "ru": "ru",
    "ko": "kr",
    "zh-Hant": "tw",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Download locale flag SVGs into src/core/i18n/localesIcons.",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=DEFAULT_OUTPUT_DIR,
        help=f"Directory to write SVG files into. Defaults to {DEFAULT_OUTPUT_DIR}.",
    )
    parser.add_argument(
        "--base-url",
        default=FLAG_BASE_URL,
        help=f"Base URL for flag SVGs. Defaults to {FLAG_BASE_URL}.",
    )
    parser.add_argument(
        "--overwrite",
        action="store_true",
        help="Replace existing files instead of skipping them.",
    )
    parser.add_argument(
        "--timeout",
        type=float,
        default=20.0,
        help="HTTP timeout in seconds. Defaults to 20.",
    )
    parser.add_argument(
        "locales",
        nargs="*",
        help="Optional locale codes to fetch. Defaults to all known locales.",
    )
    return parser.parse_args()


def fetch_svg(url: str, timeout: float) -> bytes:
    request = urllib.request.Request(
        url,
        headers={
            "User-Agent": "mobile-app-locale-flag-fetcher/1.0",
            "Accept": "image/svg+xml,*/*;q=0.8",
        },
    )
    with urllib.request.urlopen(request, timeout=timeout) as response:
        content_type = response.headers.get("Content-Type", "")
        if "svg" not in content_type and "xml" not in content_type:
            raise ValueError(f"Unexpected content type for {url}: {content_type or 'unknown'}")
        return response.read()


def resolve_locales(selected: list[str]) -> list[str]:
    if not selected:
        return list(LOCALE_TO_FLAG.keys())

    unknown = [locale for locale in selected if locale not in LOCALE_TO_FLAG]
    if unknown:
        raise SystemExit(f"Unsupported locale codes: {', '.join(sorted(unknown))}")

    return selected


def main() -> int:
    args = parse_args()
    output_dir = args.output_dir
    output_dir.mkdir(parents=True, exist_ok=True)

    locales = resolve_locales(args.locales)
    failures: list[str] = []

    for locale in locales:
        country_code = LOCALE_TO_FLAG[locale]
        target_path = output_dir / f"{locale}.svg"

        if target_path.exists() and not args.overwrite:
            print(f"skip  {locale:<7} {target_path}")
            continue

        source_url = f"{args.base_url}/{country_code}.svg"
        try:
            svg_bytes = fetch_svg(source_url, timeout=args.timeout)
        except (urllib.error.URLError, ValueError) as error:
            print(f"error {locale:<7} {source_url} -> {error}", file=sys.stderr)
            failures.append(locale)
            continue

        target_path.write_bytes(svg_bytes)
        print(f"saved {locale:<7} {target_path} <- {source_url}")

    if failures:
        print(
            "failed locales: " + ", ".join(failures),
            file=sys.stderr,
        )
        return 1

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
