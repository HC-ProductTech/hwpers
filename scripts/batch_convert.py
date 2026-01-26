#!/usr/bin/env python3
"""
Batch converter that adapts the josn-result JSON files into the jsontohwpx CLI format.

Usage:
    python scripts/batch_convert.py josn-result/*.json

The script streams each converted JSON into jsontohwpx via stdin so that large
inputs don't require temporary files on disk.
"""

import argparse
import json
import re
import subprocess
import sys
from datetime import datetime
from pathlib import Path
from typing import Dict, List, Optional, Sequence, Set


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Convert article dumps to HWPX files.")
    parser.add_argument(
        "inputs",
        nargs="+",
        help="Input JSON files (each must contain a list of article records).",
    )
    parser.add_argument(
        "--converter",
        default="target/release/jsontohwpx",
        help="Path to the jsontohwpx binary (default: %(default)s).",
    )
    parser.add_argument(
        "--output-dir",
        default="output",
        help="Directory where the generated *.hwpx files will be written.",
    )
    parser.add_argument(
        "--include-header",
        action="store_true",
        help="Force the converter to include header metadata (regEmpName/regDeptName/regDt).",
    )
    parser.add_argument(
        "--max-per-file",
        type=int,
        default=None,
        help="Limit the number of articles converted per input file.",
    )
    parser.add_argument(
        "--title-date-name",
        action="store_true",
        help="Name output files as '<title>_<YYYY-MM-DD>.hwpx' using metadata.created_at.",
    )
    return parser.parse_args()


def strip_control_chars(value: str) -> str:
    return "".join(ch for ch in value if (ord(ch) >= 0x20) or ch in "\n\r\t")


def normalise_text(value: Optional[str]) -> Optional[str]:
    if value is None:
        return None
    text = strip_control_chars(value).strip()
    return text or None


def build_contents(article: Dict) -> List[Dict]:
    text_block = normalise_text(article.get("content_text"))
    if text_block:
        return [{"type": "text", "value": text_block}]

    contents: List[Dict] = []
    for block in article.get("content", []):
        block_type = block.get("type")
        if block_type == "text":
            text = block.get("value")
        elif block_type == "link":
            text_part = block.get("text") or block.get("value") or ""
            url = block.get("url") or ""
            text = f"{text_part} ({url})".strip()
        else:
            continue

        text = normalise_text(text)
        if text:
            contents.append({"type": "text", "value": text})

    if contents:
        return contents

    fallback = normalise_text(article.get("title"))
    if fallback:
        contents.append({"type": "text", "value": fallback})

    return contents


def ensure_unique_id(candidates: List[Optional[str]], used_ids: Set[str]) -> str:
    for candidate in candidates:
        candidate = normalise_text(candidate)
        if candidate:
            break
    else:
        candidate = f"ARTICLE_{len(used_ids)+1:05d}"

    base = candidate
    counter = 1
    while candidate in used_ids:
        counter += 1
        candidate = f"{base}_{counter}"

    used_ids.add(candidate)
    return candidate


def try_parse_created_date(raw: Optional[str]) -> Optional[str]:
    if not raw:
        return None

    text = raw.strip()
    if not text:
        return None

    # Normalise locale-specific markers and delimiters
    replacements = {
        "오전": "AM",
        "오후": "PM",
        ".": "-",
    }
    for src, dst in replacements.items():
        text = text.replace(src, dst)

    patterns: Sequence[str] = (
        "%Y-%m-%d %p %I:%M:%S",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d",
    )
    for pattern in patterns:
        try:
            dt = datetime.strptime(text, pattern)
            return dt.strftime("%Y-%m-%d")
        except ValueError:
            continue
    return None


_INVALID_FILENAME_CHARS = re.compile(r"[\\/:*?\"<>|]+")


def safe_filename_component(value: str) -> str:
    collapsed = " ".join(value.split())
    cleaned = _INVALID_FILENAME_CHARS.sub("_", collapsed)
    cleaned = cleaned.strip().strip(".")
    return cleaned


def ensure_unique_filename(base: str, used_names: Set[str]) -> str:
    if not base:
        base = "document"

    candidate = base
    counter = 1
    while candidate in used_names:
        counter += 1
        candidate = f"{base}_{counter}"

    used_names.add(candidate)
    return candidate


def build_output_basename(
    raw_article: Dict,
    metadata: Dict,
    fallback_id: str,
    used_filenames: Set[str],
    use_title_date_name: bool,
) -> str:
    if use_title_date_name:
        title = normalise_text(raw_article.get("title"))
        created = try_parse_created_date(metadata.get("created_at"))

        parts = []
        if title:
            parts.append(title)
        if created:
            parts.append(created)

        if parts:
            candidate = "_".join(parts)
        else:
            candidate = fallback_id
    else:
        candidate = fallback_id

    safe_candidate = safe_filename_component(candidate)
    if not safe_candidate:
        safe_candidate = safe_filename_component(fallback_id)
    return ensure_unique_filename(safe_candidate, used_filenames)


def convert_article(
    raw_article: Dict,
    include_header: bool,
    converter: Path,
    output_dir: Path,
    used_ids: Set[str],
    used_filenames: Set[str],
    use_title_date_name: bool,
) -> str:
    atcl_id = ensure_unique_id(
        [
            raw_article.get("article_id"),
            raw_article.get("id"),
        ],
        used_ids,
    )

    contents = build_contents(raw_article)
    metadata = raw_article.get("metadata") or {}
    if not isinstance(metadata, dict):
        metadata = {}

    response = {
        "responseCode": "0",
        "responseText": "SUCCESS",
        "options": {"includeHeader": include_header},
        "data": {
            "article": {
                "atclId": atcl_id,
                "subject": raw_article.get("title") or "",
                "contents": contents,
                "regDt": metadata.get("created_at"),
                "regEmpName": metadata.get("author"),
                "regDeptName": metadata.get("department"),
            }
        },
    }

    output_dir.mkdir(parents=True, exist_ok=True)
    base_name = build_output_basename(
        raw_article,
        metadata,
        atcl_id,
        used_filenames,
        use_title_date_name,
    )
    output_path = output_dir / f"{base_name}.hwpx"
    cmd = [str(converter), "-", "--output", str(output_path)]
    if include_header:
        cmd.append("--include-header")

    proc = subprocess.run(
        cmd,
        input=json.dumps(response, ensure_ascii=False).encode("utf-8"),
        stdout=sys.stdout,
        stderr=sys.stderr,
        check=False,
    )
    if proc.returncode != 0:
        raise subprocess.CalledProcessError(proc.returncode, cmd)

    return output_path.name


def main() -> None:
    args = parse_args()
    converter = Path(args.converter)
    if not converter.exists():
        raise SystemExit(f"converter not found: {converter}")

    base_output_dir = Path(args.output_dir)
    base_output_dir.mkdir(parents=True, exist_ok=True)

    for input_path_str in args.inputs:
        input_path = Path(input_path_str)
        if not input_path.exists():
            print(f"[SKIP] missing file: {input_path}", file=sys.stderr)
            continue

        with input_path.open(encoding="utf-8") as fh:
            articles = json.load(fh)

        if not isinstance(articles, list):
            print(f"[WARN] {input_path} does not contain a list; skipping", file=sys.stderr)
            continue

        target_dir = base_output_dir / input_path.stem
        target_dir.mkdir(parents=True, exist_ok=True)
        used_ids: Set[str] = set()

        limit = args.max_per_file
        if limit is not None and limit > 0:
            articles_to_process = articles[:limit]
        else:
            articles_to_process = articles

        print(
            f"[INFO] Converting {input_path} -> {target_dir} "
            f"({len(articles_to_process)}/{len(articles)} articles)"
        )
        used_filenames: Set[str] = set()
        for idx, article in enumerate(articles_to_process, start=1):
            try:
                output_name = convert_article(
                    article,
                    args.include_header,
                    converter,
                    target_dir,
                    used_ids,
                    used_filenames,
                    args.title_date_name,
                )
                print(f"  - [{idx}/{len(articles)}] -> {output_name}")
            except Exception as exc:  # pragma: no cover - manual script
                print(
                    f"  ! Failed to convert record #{idx} in {input_path}: {exc}",
                    file=sys.stderr,
                )


if __name__ == "__main__":
    main()
