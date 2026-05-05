#!/usr/bin/env python3
"""
J10 Phase 1: Baseline Trajectory Export Script for Clarity

Exports session data from clarity-memory databases into structured JSON
trajectories for training-data/baseline/.

Data sources:
  - sessions.db: conversation turns (sessions + session_messages)
  - memory.db: long-term facts / memories
"""

import sqlite3
import json
import os
from datetime import datetime, timezone

CLARITY_ROOT = r"C:\Users\22414\dev\third_party\clarity"
OUTPUT_DIR = os.path.join(CLARITY_ROOT, "training-data", "baseline")

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def ensure_dir(path: str) -> None:
    os.makedirs(path, exist_ok=True)


def load_sessions_db(db_path: str) -> list[dict]:
    """Load grouped session trajectories from a sessions.db file."""
    if not os.path.exists(db_path):
        return []

    conn = sqlite3.connect(db_path)
    conn.row_factory = sqlite3.Row
    cur = conn.cursor()

    # Verify tables exist
    cur.execute("SELECT name FROM sqlite_master WHERE type='table'")
    tables = {row[0] for row in cur.fetchall()}
    if "sessions" not in tables or "session_messages" not in tables:
        conn.close()
        return []

    # Fetch all messages grouped by session
    cur.execute("""
        SELECT m.id, m.session_id, m.role, m.content,
               m.tool_calls, m.tool_call_id, m.created_at,
               s.created_at as session_created_at,
               s.updated_at as session_updated_at
        FROM session_messages m
        JOIN sessions s ON m.session_id = s.session_id
        ORDER BY m.session_id, m.created_at
    """)

    sessions_map: dict[str, dict] = {}
    for row in cur.fetchall():
        sid = row["session_id"]
        if sid not in sessions_map:
            sessions_map[sid] = {
                "session_id": sid,
                "created_at": row["session_created_at"],
                "updated_at": row["session_updated_at"],
                "messages": [],
            }
        msg = {
            "id": row["id"],
            "role": row["role"],
            "content": row["content"],
            "tool_calls": row["tool_calls"],
            "tool_call_id": row["tool_call_id"],
            "created_at": row["created_at"],
        }
        sessions_map[sid]["messages"].append(msg)

    conn.close()
    return list(sessions_map.values())


def load_memory_db(db_path: str) -> list[dict]:
    """Load facts / memories from a memory.db file."""
    if not os.path.exists(db_path):
        return []

    conn = sqlite3.connect(db_path)
    conn.row_factory = sqlite3.Row
    cur = conn.cursor()

    cur.execute("SELECT name FROM sqlite_master WHERE type='table'")
    tables = {row[0] for row in cur.fetchall()}
    if "facts" not in tables:
        conn.close()
        return []

    cur.execute("SELECT id, fact, tags, time, session_id, created_at FROM facts ORDER BY created_at")
    facts = [dict(row) for row in cur.fetchall()]
    conn.close()
    return facts


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main() -> None:
    ensure_dir(OUTPUT_DIR)

    # Candidate database locations (project-local first, then user profile)
    candidates = [
        # gateway crate (has real session data)
        os.path.join(CLARITY_ROOT, "crates", "clarity-gateway", ".clarity", "sessions.db"),
        os.path.join(CLARITY_ROOT, "crates", "clarity-gateway", ".clarity", "memory.db"),
        # project root
        os.path.join(CLARITY_ROOT, ".clarity", "sessions.db"),
        os.path.join(CLARITY_ROOT, ".clarity", "memory.db"),
        # legacy user profile paths
        os.path.join(os.path.expanduser("~"), ".clarity", "sessions.db"),
        os.path.join(os.path.expanduser("~"), ".clarity", "memory.db"),
    ]

    sessions_db = None
    memory_dbs = []
    for p in candidates:
        if os.path.exists(p):
            if "sessions" in os.path.basename(p) and sessions_db is None:
                sessions_db = p
            if "memory" in os.path.basename(p):
                memory_dbs.append(p)

    total_exported = 0
    meta = {
        "export_time": datetime.now(timezone.utc).isoformat(),
        "sources": {},
        "counts": {"sessions": 0, "facts": 0},
    }

    # ---- Export session trajectories ----
    if sessions_db:
        print(f"[sessions] Using {sessions_db}")
        trajectories = load_sessions_db(sessions_db)
        meta["sources"]["sessions_db"] = sessions_db
        meta["counts"]["sessions"] = len(trajectories)

        for i, traj in enumerate(trajectories):
            # Filter: skip empty / mock-only sessions
            msgs = traj.get("messages", [])
            real_msgs = [m for m in msgs if m.get("content") and m["content"].strip()]
            if not real_msgs:
                continue

            # Detect mock data
            has_mock = any("mock" in (m.get("content") or "").lower() for m in real_msgs)
            if has_mock and len(real_msgs) <= 2:
                # Very short mock session — still keep for baseline, but mark it
                traj["_quality_tag"] = "short_mock"

            fname = f"session_{i:04d}_{traj['session_id'][:8]}.json"
            fpath = os.path.join(OUTPUT_DIR, fname)
            with open(fpath, "w", encoding="utf-8") as f:
                json.dump(traj, f, ensure_ascii=False, indent=2)
            total_exported += 1
            print(f"  + {fname} ({len(msgs)} messages)")
    else:
        print("[sessions] No sessions.db found")

    # ---- Export memory facts (aggregate from all memory.db sources) ----
    all_facts = []
    seen_facts = set()
    for mdb in memory_dbs:
        facts = load_memory_db(mdb)
        for f in facts:
            key = (f.get("fact", "") or "") + str(f.get("created_at", ""))
            if key not in seen_facts:
                seen_facts.add(key)
                f["_source_db"] = mdb
                all_facts.append(f)

    if all_facts:
        meta["sources"]["memory_dbs"] = memory_dbs
        meta["counts"]["facts"] = len(all_facts)
        fpath = os.path.join(OUTPUT_DIR, "memory_facts.json")
        with open(fpath, "w", encoding="utf-8") as f:
            json.dump(all_facts, f, ensure_ascii=False, indent=2)
        total_exported += 1
        print(f"[memory]   Aggregated {len(all_facts)} facts from {len(memory_dbs)} source(s)")
        print(f"  + memory_facts.json")
    else:
        print("[memory]   No memory.db with facts found")

    # ---- Write metadata ----
    meta_path = os.path.join(OUTPUT_DIR, "_export_meta.json")
    with open(meta_path, "w", encoding="utf-8") as f:
        json.dump(meta, f, ensure_ascii=False, indent=2)
    print(f"  + _export_meta.json")

    print(f"\nTotal files exported: {total_exported}")
    print(f"Output directory: {OUTPUT_DIR}")


if __name__ == "__main__":
    main()
