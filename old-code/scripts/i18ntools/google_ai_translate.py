
#!/usr/bin/env python3
import argparse
import concurrent.futures
import json
import os
import re
import sys
import threading
import time
from pathlib import Path

from google import genai
from google.genai import types


ROOT = Path(__file__).resolve().parent
API_KEY = os.environ.get("GOOGLE_API_KEY") or os.environ.get("GEMINI_API_KEY")

LOCALES = {
    "es": "Spanish",
    "fr": "French",
    "de": "German",
    "ja": "Japanese",
    "pl": "Polish",
    "pt": "Portuguese (Brazilian/neutral app UI)",
    "no": "Norwegian Bokmal-style app UI Norwegian",
    "id": "Indonesian",
    "fil": "Filipino/Tagalog",
    "nl": "Dutch",
    "el": "Greek",
    "hi": "Hindi",
    "it": "Italian",
    "vi": "Vietnamese",
    "ru": "Russian",
    "ko": "Korean",
    "tr": "Turkish",
    "zh-Hans": "Simplified Chinese",
    "zh-Hant": "Traditional Chinese",
}

DEFAULT_TARGETS = [
    "es-2",
    "de-2",
    "ja-2",
    "pl-2",
    "pt-2",
    "no-2",
    "id-2",
    "fil-2",
    "nl-2",
    "el-1",
    "el-2",
    "hi-1",
    "hi-2",
    "it-2",
    "vi-2",
    "ru-2",
    "ko-2",
    "tr-1",
    "tr-2",
    "zh-Hans-1",
    "zh-Hans-2",
    "zh-Hant-1",
    "zh-Hant-2",
]

DEFAULT_MODELS = [
    "gemma-4-31b-it",
    "gemma-4-26b-a4b-it",
]

PLACEHOLDER_RE = re.compile(r"\{\{[^}]+\}\}")
TOKEN_RE = re.compile(r"[A-Za-z][A-Za-z0-9']*")
ALLOWED_IDENTICAL = {
    "API",
    "BYOK",
    "B",
    "KB",
    "MB",
    "GB",
    "TB",
    "Mistral",
    "OpenAI",
    "Anthropic",
    "Cerebras",
    "LettuceAI",
    "Google",
    "AI",
    "Studio",
    "Discord",
    "JSON",
    "URL",
    "HTTP",
    "HTTPS",
}


def require_key() -> None:
    if not API_KEY:
        print("Set GOOGLE_API_KEY or GEMINI_API_KEY in the environment.", file=sys.stderr)
        raise SystemExit(1)


def parse_target(target: str) -> tuple[str, str]:
    match = re.fullmatch(r"(.+)-([12])", target)
    if not match:
        raise ValueError(f"invalid target {target!r}; expected locale-chunk, e.g. de-2")
    return match.group(1), match.group(2)


def read_json(path: Path) -> dict[str, str]:
    if not path.exists() or not path.read_text(encoding="utf-8").strip():
        return {}
    return json.loads(path.read_text(encoding="utf-8"))


def write_json_atomic(path: Path, data: dict[str, str]) -> None:
    tmp = ROOT / "tmp" / f"{path.name}.{os.getpid()}.tmp"
    tmp.parent.mkdir(parents=True, exist_ok=True)
    tmp.write_text(json.dumps(data, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    tmp.replace(path)


def placeholders(value: str) -> list[str]:
    return sorted(PLACEHOLDER_RE.findall(value))


def same_placeholders(src: str, out: str) -> bool:
    return placeholders(src) == placeholders(out)


def make_client(timeout: int) -> genai.Client:
    require_key()
    return genai.Client(
        api_key=API_KEY,
        http_options=types.HttpOptions(timeout=timeout * 1000),
    )


def list_models(client: genai.Client) -> None:
    for model in client.models.list():
        name = getattr(model, "name", "")
        methods = ", ".join(getattr(model, "supported_actions", None) or getattr(model, "supported_generation_methods", None) or [])
        display = getattr(model, "display_name", "") or getattr(model, "displayName", "")
        print(f"{name}\t{display}\t{methods}")


def function_declaration(keys: list[str]) -> types.FunctionDeclaration:
    return types.FunctionDeclaration(
        name="submit_translations",
        description="Submit translated app UI strings for the provided i18n keys.",
        parameters_json_schema={
            "type": "object",
            "properties": {
                "translations": {
                    "type": "array",
                    "description": (
                        "One item per provided i18n key. Each key must exactly match an input key."
                    ),
                    "items": {
                        "type": "object",
                        "properties": {
                            "key": {"type": "string", "enum": keys},
                            "value": {"type": "string"},
                        },
                        "required": ["key", "value"],
                    },
                }
            },
            "required": ["translations"],
        },
    )


def generate(
    client: genai.Client,
    model: str,
    locale: str,
    batch: dict[str, str],
) -> dict[str, str]:
    language = LOCALES.get(locale, locale)
    keys = list(batch)
    response = client.models.generate_content(
        model=model,
        contents=(
            f"Translate these English UI strings to {language}.\n"
            f"Call submit_translations with exactly {len(keys)} translation items. "
            f"Each item must have key and value fields. Keys: {', '.join(keys)}.\n"
            f"Source JSON:\n{json.dumps(batch, ensure_ascii=False, indent=1)}"
        ),
        config=types.GenerateContentConfig(
            system_instruction=(
                "You translate app UI localization strings. You must respond by calling "
                "the submit_translations function. Keep every input key exactly unchanged. "
                "Translate only string values. Preserve placeholders such as {{count}} exactly. "
                "Preserve product/provider names, file extensions, API acronyms, markdown/code "
                "punctuation, and variables. Keep strings concise and natural."
            ),
            temperature=0,
            tools=[types.Tool(function_declarations=[function_declaration(keys)])],
            tool_config=types.ToolConfig(
                function_calling_config=types.FunctionCallingConfig(
                    mode=types.FunctionCallingConfigMode.ANY,
                    allowed_function_names=["submit_translations"],
                )
            ),
        ),
    )
    candidates = response.candidates or []
    if not candidates:
        raise ValueError(f"no candidates: {response}")
    parts = candidates[0].content.parts or []
    for part in parts:
        call = getattr(part, "function_call", None)
        if not call or call.name != "submit_translations":
            continue
        args = call.args or {}
        translations = args.get("translations")
        if isinstance(translations, dict):
            return translations
        if isinstance(translations, list):
            out: dict[str, str] = {}
            for item in translations:
                if not isinstance(item, dict):
                    raise ValueError("translation item was not an object")
                key = item.get("key")
                value = item.get("value")
                if not isinstance(key, str) or not isinstance(value, str):
                    raise ValueError("translation item missing string key/value")
                out[key] = value
            return out
        raise ValueError("submit_translations did not include translations")
    raise ValueError(f"model did not call submit_translations: {json.dumps(parts)[:1000]}")


def validate_batch(source: dict[str, str], translated: dict[str, str]) -> tuple[bool, str]:
    source_keys = set(source)
    out_keys = set(translated)
    missing = source_keys - out_keys
    extra = out_keys - source_keys
    if missing or extra:
        return False, f"key mismatch: missing={len(missing)} extra={len(extra)}"
    non_strings = [key for key, value in translated.items() if not isinstance(value, str)]
    if non_strings:
        return False, f"non-string values: {non_strings[:3]}"
    bad_placeholders = [
        key for key, value in translated.items() if not same_placeholders(source[key], value)
    ]
    if bad_placeholders:
        return False, f"placeholder mismatch: {bad_placeholders[:3]}"
    return True, "ok"


def suspicious_unchanged(source: dict[str, str], translated: dict[str, str]) -> list[str]:
    suspicious = []
    for key, src in source.items():
        out = translated.get(key, "")
        if src == out:
            tokens = set(TOKEN_RE.findall(src))
            if not tokens or not tokens.issubset(ALLOWED_IDENTICAL):
                suspicious.append(key)
    return suspicious


def translate_batch(
    client: genai.Client,
    models: list[str],
    locale: str,
    batch: dict[str, str],
    request_counts: dict[str, int],
    request_counts_lock: threading.Lock,
    max_requests_per_model: int,
    retries_per_model: int,
) -> tuple[dict[str, str], str]:
    errors = []
    for model in models:
        for attempt in range(1, retries_per_model + 1):
            with request_counts_lock:
                if request_counts.get(model, 0) >= max_requests_per_model:
                    errors.append(f"{model}: local request cap reached")
                    break
                request_counts[model] = request_counts.get(model, 0) + 1
            try:
                out = generate(client, model, locale, batch)
                ok, reason = validate_batch(batch, out)
                if not ok:
                    raise ValueError(reason)
                return out, model
            except Exception as error:
                errors.append(f"{model} attempt {attempt}: {error}")
                if attempt < retries_per_model:
                    print(f"  {model} attempt {attempt} failed, retrying: {error}", flush=True)
                    time.sleep(min(2 * attempt, 10))
                else:
                    print(f"  {model} failed, trying fallback if available: {error}", flush=True)
    raise RuntimeError("; ".join(errors))


def verify_target(target: str) -> dict:
    missing = read_json(ROOT / f"missing-{target}.json")
    done = read_json(ROOT / f"done-{target}.json")
    missing_keys = [key for key in missing if key not in done]
    bad_placeholders = [
        key
        for key, value in missing.items()
        if key in done and (not isinstance(done[key], str) or not same_placeholders(value, done[key]))
    ]
    suspicious = suspicious_unchanged(missing, done)
    return {
        "target": target,
        "total": len(missing),
        "done": len(done),
        "missing": len(missing_keys),
        "bad_placeholders": len(bad_placeholders),
        "suspicious_unchanged": len(suspicious),
    }


def run_target(
    client: genai.Client,
    args: argparse.Namespace,
    target: str,
    request_counts: dict[str, int],
    request_counts_lock: threading.Lock,
) -> None:
    locale, _chunk = parse_target(target)
    missing_path = ROOT / f"missing-{target}.json"
    done_path = ROOT / f"done-{target}.json"
    missing = read_json(missing_path)
    done = read_json(done_path)
    remaining = [
        (key, value)
        for key, value in missing.items()
        if not isinstance(done.get(key), str) or not same_placeholders(value, done[key])
    ]
    if not remaining:
        print(f"{target}: complete ({len(done)}/{len(missing)})", flush=True)
        return

    print(
        f"{target}: translating {len(remaining)}/{len(missing)}; "
        f"batch={args.batch_size}; models={', '.join(args.models)}",
        flush=True,
    )
    planned = []
    batches_done = 0
    for offset in range(0, len(remaining), args.batch_size):
        if args.max_batches and batches_done >= args.max_batches:
            break
        planned.append((offset, dict(remaining[offset : offset + args.batch_size])))
        batches_done += 1

    suspects: list[str] = []
    if args.workers <= 1 or len(planned) <= 1:
        for offset, batch in planned:
            translated, used_model = translate_batch(
                client,
                args.models,
                locale,
                batch,
                request_counts,
                request_counts_lock,
                args.max_requests_per_model,
                args.retries_per_model,
            )
            suspects.extend(suspicious_unchanged(batch, translated))
            done.update(translated)
            write_json_atomic(done_path, done)
            print(
                f"  {target}: +{len(batch)} via {used_model}; file has {len(done)}/{len(missing)}; "
                f"requests={request_counts}",
                flush=True,
            )
    else:
        with concurrent.futures.ThreadPoolExecutor(max_workers=args.workers) as executor:
            futures = {
                executor.submit(
                    translate_batch,
                    client,
                    args.models,
                    locale,
                    batch,
                    request_counts,
                    request_counts_lock,
                    args.max_requests_per_model,
                    args.retries_per_model,
                ): (offset, batch)
                for offset, batch in planned
            }
            failed_batches = 0
            for future in concurrent.futures.as_completed(futures):
                offset, batch = futures[future]
                try:
                    translated, used_model = future.result()
                except Exception as error:
                    failed_batches += 1
                    print(
                        f"  {target}: batch at offset {offset} failed after fallbacks: {error}",
                        flush=True,
                    )
                    continue
                suspects.extend(suspicious_unchanged(batch, translated))
                done.update(translated)
                write_json_atomic(done_path, done)
                print(
                    f"  {target}: +{len(batch)} via {used_model}; file has {len(done)}/{len(missing)}; "
                    f"requests={request_counts}",
                    flush=True,
                )
            if failed_batches:
                print(
                    f"  {target}: {failed_batches} batch(es) failed; rerun the same command to resume.",
                    flush=True,
                )

    if suspects:
        suspect_path = ROOT / f"suspect-{target}.json"
        existing = read_json(suspect_path)
        for key in suspects:
            existing[key] = done.get(key, "")
        write_json_atomic(suspect_path, existing)
        print(f"  {target}: wrote suspect unchanged keys to {suspect_path.name}", flush=True)

    report = verify_target(target)
    print(f"  verify {target}: {report}", flush=True)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("targets", nargs="*", default=DEFAULT_TARGETS)
    parser.add_argument("--models", default=",".join(DEFAULT_MODELS))
    parser.add_argument("--batch-size", type=int, default=int(os.environ.get("BATCH_SIZE", "300")))
    parser.add_argument("--timeout", type=int, default=int(os.environ.get("GOOGLE_AI_TIMEOUT", "300")))
    parser.add_argument("--max-batches", type=int, default=0)
    parser.add_argument("--max-requests-per-model", type=int, default=15)
    parser.add_argument("--retries-per-model", type=int, default=int(os.environ.get("GOOGLE_AI_RETRIES", "1")))
    parser.add_argument("--workers", type=int, default=int(os.environ.get("GOOGLE_AI_WORKERS", "1")))
    parser.add_argument("--list-models", action="store_true")
    parser.add_argument("--verify-only", action="store_true")
    args = parser.parse_args()
    args.models = [model.strip() for model in args.models.split(",") if model.strip()]

    if args.list_models:
        client = make_client(args.timeout)
        list_models(client)
        return 0

    if args.verify_only:
        reports = [verify_target(target) for target in args.targets]
        for report in reports:
            print(report)
        failed = [r for r in reports if r["missing"] or r["bad_placeholders"]]
        return 1 if failed else 0

    client = make_client(args.timeout)
    request_counts: dict[str, int] = {}
    request_counts_lock = threading.Lock()
    for target in args.targets:
        run_target(client, args, target, request_counts, request_counts_lock)
        if all(request_counts.get(model, 0) >= args.max_requests_per_model for model in args.models):
            print("All model request caps reached; stopping for today.", flush=True)
            break
    return 0


if __name__ == "__main__":
    sys.exit(main())
