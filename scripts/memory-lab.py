#!/usr/bin/env python3
"""Prepare and inspect a disposable roleplay memory profile."""

import argparse
from collections import Counter
import json
import os
from pathlib import Path
import sqlite3
import subprocess


REPOSITORY = Path(__file__).resolve().parents[1]


def prepare(root):
    if root.exists():
        raise SystemExit(f"Choose a fresh directory; {root} already exists.")
    environment = dict(os.environ, LETTUCE_MEMORY_LAB_DIR=str(root))
    subprocess.run(
        ["cargo", "test", "-p", "lettuce-app", "--lib", "--offline",
         "prepare_roleplay_memory_lab", "--", "--ignored", "--nocapture"],
        cwd=REPOSITORY, env=environment, check=True,
    )
    print(f"\nInspect: python3 scripts/memory-lab.py inspect {root}")
    print("Launch from the repository root:")
    print(f"bun run tauri dev --config {str(root / 'tauri.memory-lab.conf.json')!r}")


def inspect(root):
    report = json.loads((root / "report.json").read_text())
    database = Path(report["database"]).resolve()
    if not database.is_relative_to(root):
        raise SystemExit("The report database must belong to the lab directory.")
    conversation = report["conversation_id"]
    with sqlite3.connect(database.as_uri() + "?mode=ro", uri=True) as connection:
        connection.row_factory = sqlite3.Row
        connection.execute("BEGIN")
        counts = dict(connection.execute("""
            SELECT COUNT(*) AS saved_memories,
                   COALESCE(SUM(is_cold = 0), 0) AS hot,
                   COALESCE(SUM(is_cold = 1), 0) AS cold
            FROM memory_items WHERE space_id IN (
                SELECT space_id FROM conversation_memory_spaces
                WHERE conversation_id = ?
            )
        """, (conversation,)).fetchone())
        attempts = []
        for attempt in connection.execute("""
            SELECT a.id, a.run_id, a.job_id, a.status, a.failure,
                   r.summary_window_start, r.summary_window_end
            FROM dynamic_memory_run_attempts a
            JOIN dynamic_memory_runs r ON r.id = a.run_id
            WHERE r.conversation_id = ?
            ORDER BY a.created_at DESC, a.id DESC LIMIT 5
        """, (conversation,)):
            entry = dict(attempt)
            outcomes = [json.loads(row[0])["value"] for row in connection.execute("""
                SELECT outcome_json FROM dynamic_memory_background_tool_results
                WHERE attempt_id = ? ORDER BY ordinal
            """, (attempt["id"],))]
            entry["operations"] = dict(Counter(outcome["status"] for outcome in outcomes))
            entry["details"] = [outcome for outcome in outcomes
                                if outcome["status"] in ("duplicate_skipped", "rejected", "skipped")]
            attempts.append(entry)
    print(json.dumps({
        "conversation_id": conversation,
        "baseline_saved_memories": 16,
        "current": counts,
        "next_fact": report["next_fact"],
        "latest_attempts": attempts,
    }, indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("prepare", "inspect"))
    parser.add_argument("directory", type=Path, help="a disposable profile directory")
    arguments = parser.parse_args()
    root = arguments.directory.expanduser().resolve()
    {"prepare": prepare, "inspect": inspect}[arguments.action](root)


if __name__ == "__main__":
    main()
