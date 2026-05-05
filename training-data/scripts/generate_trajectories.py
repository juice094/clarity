#!/usr/bin/env python3
"""
J10 Phase 2: High-quality synthetic-real hybrid trajectories for Jumpy World Model.

These trajectories are hand-crafted based on real tool behaviours in the
clarity / devbase codebase.  Each observation captures a plausible
(skill, params, before_state, after_state) transition that would occur
when the agent executes the corresponding tool.

Quality criteria:
  - Skill IDs match real ToolRegistry names.
  - Params are valid JSON shapes for those tools.
  - State transitions are logically consistent (files appear in active_files
    after read/write, tags reflect success/failure, progress advances).
  - Scenarios cover the actual workspace (clarity + devbase).
"""

import json
import os
from datetime import datetime, timezone

CLARITY_ROOT = r"C:\Users\22414\dev\third_party\clarity"
OUTPUT_DIR = os.path.join(CLARITY_ROOT, "training-data", "trajectories")


def ensure_dir(path: str) -> None:
    os.makedirs(path, exist_ok=True)


def observation(skill_id: str, params: dict, before: dict, after: dict) -> dict:
    return {
        "skill_id": skill_id,
        "params": json.dumps(params, ensure_ascii=False),
        "before": before,
        "after": after,
    }


def state(tags, memory, active_files, context_summary, progress) -> dict:
    return {
        "tags": tags,
        "memory": memory,
        "active_files": active_files,
        "context_summary": context_summary,
        "progress": round(progress, 2),
    }


def generate_trajectories() -> list[dict]:
    traj = []

    # ------------------------------------------------------------------
    # 1. File operations (clarity / devbase codebase)
    # ------------------------------------------------------------------
    traj.append(observation(
        "file_read",
        {"path": "crates/clarity-core/src/agent/jumpy/predictor.rs"},
        state(
            tags=["jumpy-review"],
            memory={},
            active_files=[],
            context_summary="Need to understand predictor implementation",
            progress=0.0,
        ),
        state(
            tags=["jumpy-review", "file-loaded"],
            memory={"last_read_file": "crates/clarity-core/src/agent/jumpy/predictor.rs"},
            active_files=["crates/clarity-core/src/agent/jumpy/predictor.rs"],
            context_summary="Loaded predictor.rs, found LlmAdapter and OutcomePredictor trait",
            progress=0.15,
        ),
    ))

    traj.append(observation(
        "file_read",
        {"path": "crates/clarity-headless/src/main.rs"},
        state(
            tags=["cli-refactor", "file-loaded"],
            memory={"last_read_file": "crates/clarity-core/src/agent/jumpy/predictor.rs"},
            active_files=["crates/clarity-core/src/agent/jumpy/predictor.rs"],
            context_summary="Need to check headless CLI structure before adding subcommands",
            progress=0.15,
        ),
        state(
            tags=["cli-refactor", "file-loaded"],
            memory={"last_read_file": "crates/clarity-headless/src/main.rs"},
            active_files=["crates/clarity-headless/src/main.rs"],
            context_summary="Loaded main.rs, confirmed flat CLI uses clap Parser derive",
            progress=0.25,
        ),
    ))

    traj.append(observation(
        "glob",
        {"pattern": "crates/clarity-core/src/agent/jumpy/*.rs"},
        state(
            tags=["jumpy-audit"],
            memory={},
            active_files=[],
            context_summary="Need to list all Jumpy module files",
            progress=0.0,
        ),
        state(
            tags=["jumpy-audit", "files-listed"],
            memory={"glob_match_count": "5"},
            active_files=[
                "crates/clarity-core/src/agent/jumpy/mod.rs",
                "crates/clarity-core/src/agent/jumpy/predictor.rs",
                "crates/clarity-core/src/agent/jumpy/state.rs",
                "crates/clarity-core/src/agent/jumpy/planner.rs",
                "crates/clarity-core/src/agent/jumpy/composer.rs",
            ],
            context_summary="Found 5 Rust files in jumpy module",
            progress=0.1,
        ),
    ))

    traj.append(observation(
        "file_write",
        {"path": "training-data/scripts/generate_trajectories.py", "content": "# script content..."},
        state(
            tags=["data-pipeline"],
            memory={"task": "J10 Phase 2"},
            active_files=[],
            context_summary="Need to create trajectory generator script",
            progress=0.0,
        ),
        state(
            tags=["data-pipeline", "file-created"],
            memory={"task": "J10 Phase 2", "last_written": "training-data/scripts/generate_trajectories.py"},
            active_files=["training-data/scripts/generate_trajectories.py"],
            context_summary="Created generate_trajectories.py with 25 observations",
            progress=0.3,
        ),
    ))

    traj.append(observation(
        "file_edit",
        {"path": "crates/clarity-headless/src/main.rs", "replacements": [{"old": "struct Args {", "new": "struct Args {\n    #[command(subcommand)]\n    command: Command,"}]},
        state(
            tags=["cli-refactor", "file-loaded"],
            memory={"last_read_file": "crates/clarity-headless/src/main.rs"},
            active_files=["crates/clarity-headless/src/main.rs"],
            context_summary="About to refactor Args into subcommand enum",
            progress=0.25,
        ),
        state(
            tags=["cli-refactor", "file-modified"],
            memory={"last_read_file": "crates/clarity-headless/src/main.rs", "edit_count": "1"},
            active_files=["crates/clarity-headless/src/main.rs"],
            context_summary="Injected subcommand field into Args struct",
            progress=0.35,
        ),
    ))

    # ------------------------------------------------------------------
    # 2. Code search (grep)
    # ------------------------------------------------------------------
    traj.append(observation(
        "grep",
        {"pattern": "OutcomePredictor", "path": "crates/clarity-core/src", "output_mode": "files_with_matches"},
        state(
            tags=["api-audit"],
            memory={},
            active_files=[],
            context_summary="Find all usages of OutcomePredictor trait",
            progress=0.0,
        ),
        state(
            tags=["api-audit", "search-complete"],
            memory={"match_count": "7"},
            active_files=[],
            context_summary="Found 7 files referencing OutcomePredictor across jumpy, flow, subagents",
            progress=0.1,
        ),
    ))

    traj.append(observation(
        "grep",
        {"pattern": "LlmAdapter", "path": "crates/clarity-core/src", "output_mode": "content"},
        state(
            tags=["jumpy-review", "file-loaded"],
            memory={"last_read_file": "crates/clarity-core/src/agent/jumpy/predictor.rs"},
            active_files=["crates/clarity-core/src/agent/jumpy/predictor.rs"],
            context_summary="Verify LlmAdapter exists in predictor.rs",
            progress=0.15,
        ),
        state(
            tags=["jumpy-review", "verified"],
            memory={"llm_adapter_exists": "true"},
            active_files=["crates/clarity-core/src/agent/jumpy/predictor.rs"],
            context_summary="Confirmed LlmAdapter struct and impl are present at line 34",
            progress=0.2,
        ),
    ))

    # ------------------------------------------------------------------
    # 3. Shell / build operations
    # ------------------------------------------------------------------
    traj.append(observation(
        "bash",
        {"command": "cargo check -p clarity-headless", "description": "Check headless compiles"},
        state(
            tags=["build-check"],
            memory={},
            active_files=["crates/clarity-headless/src/main.rs"],
            context_summary="Need to verify subcommand refactor compiles",
            progress=0.35,
        ),
        state(
            tags=["build-check", "compile-ok"],
            memory={"last_build_status": "success", "crate": "clarity-headless"},
            active_files=["crates/clarity-headless/src/main.rs"],
            context_summary="cargo check passed for clarity-headless",
            progress=0.5,
        ),
    ))

    traj.append(observation(
        "bash",
        {"command": "cargo test -p clarity-headless", "description": "Run headless tests"},
        state(
            tags=["build-check", "compile-ok"],
            memory={"last_build_status": "success"},
            active_files=["crates/clarity-headless/src/main.rs"],
            context_summary="About to run headless unit tests",
            progress=0.5,
        ),
        state(
            tags=["build-check", "tests-passed"],
            memory={"last_build_status": "success", "test_count": "8", "failed": "0"},
            active_files=["crates/clarity-headless/src/main.rs"],
            context_summary="All 8 headless tests passed (0 failed)",
            progress=0.65,
        ),
    ))

    traj.append(observation(
        "bash",
        {"command": "cargo check -p clarity-core --lib", "description": "Check core library"},
        state(
            tags=["build-check"],
            memory={},
            active_files=["crates/clarity-core/src/agent/jumpy/predictor.rs"],
            context_summary="Verify LlmAdapter addition doesn't break core",
            progress=0.2,
        ),
        state(
            tags=["build-check", "compile-ok"],
            memory={"last_build_status": "success", "crate": "clarity-core"},
            active_files=["crates/clarity-core/src/agent/jumpy/predictor.rs"],
            context_summary="cargo check passed for clarity-core lib",
            progress=0.3,
        ),
    ))

    traj.append(observation(
        "bash",
        {"command": "cargo test -p clarity-core --lib", "description": "Run core lib tests"},
        state(
            tags=["build-check", "compile-ok"],
            memory={"last_build_status": "success"},
            active_files=["crates/clarity-core/src/agent/jumpy/predictor.rs"],
            context_summary="About to run full clarity-core test suite",
            progress=0.3,
        ),
        state(
            tags=["build-check", "tests-passed"],
            memory={"last_build_status": "success", "test_count": "524", "failed": "0", "ignored": "6"},
            active_files=["crates/clarity-core/src/agent/jumpy/predictor.rs"],
            context_summary="524 core tests passed, 0 failed, 6 ignored",
            progress=0.45,
        ),
    ))

    # ------------------------------------------------------------------
    # 4. Git operations
    # ------------------------------------------------------------------
    traj.append(observation(
        "bash",
        {"command": "git status --short", "description": "Check working tree"},
        state(
            tags=["pre-commit"],
            memory={},
            active_files=[
                "crates/clarity-core/src/agent/jumpy/predictor.rs",
                "crates/clarity-headless/src/main.rs",
            ],
            context_summary="Need to verify changes before commit",
            progress=0.65,
        ),
        state(
            tags=["pre-commit", "dirty"],
            memory={"dirty_files": "2", "untracked": "0"},
            active_files=[
                "crates/clarity-core/src/agent/jumpy/predictor.rs",
                "crates/clarity-headless/src/main.rs",
            ],
            context_summary="Working tree has 2 modified files: predictor.rs and main.rs",
            progress=0.7,
        ),
    ))

    traj.append(observation(
        "bash",
        {"command": "git diff --stat", "description": "Review diff stats"},
        state(
            tags=["pre-commit", "dirty"],
            memory={"dirty_files": "2"},
            active_files=[
                "crates/clarity-core/src/agent/jumpy/predictor.rs",
                "crates/clarity-headless/src/main.rs",
            ],
            context_summary="Reviewing diff size before commit",
            progress=0.7,
        ),
        state(
            tags=["pre-commit", "diff-reviewed"],
            memory={"predictor.rs": "+34", "main.rs": "+347/-59"},
            active_files=[
                "crates/clarity-core/src/agent/jumpy/predictor.rs",
                "crates/clarity-headless/src/main.rs",
            ],
            context_summary="Diff reviewed: +322/-59 total, no unexpected changes",
            progress=0.75,
        ),
    ))

    traj.append(observation(
        "bash",
        {"command": "git commit -m 'feat(j9): ...'", "description": "Commit J9 changes"},
        state(
            tags=["pre-commit", "diff-reviewed"],
            memory={"predictor.rs": "+34", "main.rs": "+347/-59"},
            active_files=[
                "crates/clarity-core/src/agent/jumpy/predictor.rs",
                "crates/clarity-headless/src/main.rs",
            ],
            context_summary="Ready to commit J9 feature",
            progress=0.75,
        ),
        state(
            tags=["committed"],
            memory={"commit_hash": "4c191aac", "commit_msg": "feat(j9): clarity-headless Jumpy subcommand + LlmAdapter"},
            active_files=[],
            context_summary="Committed 2 files to main (4c191aac)",
            progress=0.85,
        ),
    ))

    # ------------------------------------------------------------------
    # 5. Devbase operations
    # ------------------------------------------------------------------
    traj.append(observation(
        "file_read",
        {"path": "src/knowledge_engine/llm.rs"},
        state(
            tags=["security-audit"],
            memory={},
            active_files=[],
            context_summary="Review HV-1 LLM leakage risk in devbase",
            progress=0.0,
        ),
        state(
            tags=["security-audit", "file-loaded"],
            memory={"last_read_file": "src/knowledge_engine/llm.rs", "risk": "HV-1"},
            active_files=["src/knowledge_engine/llm.rs"],
            context_summary="Loaded llm.rs, confirmed default_llm_enabled() returns false",
            progress=0.15,
        ),
    ))

    traj.append(observation(
        "grep",
        {"pattern": "try_llm_summary", "path": "src", "output_mode": "content"},
        state(
            tags=["security-audit", "file-loaded"],
            memory={"last_read_file": "src/knowledge_engine/llm.rs"},
            active_files=["src/knowledge_engine/llm.rs"],
            context_summary="Find all call sites of try_llm_summary",
            progress=0.15,
        ),
        state(
            tags=["security-audit", "verified"],
            memory={"call_sites": "2", "gated": "true"},
            active_files=["src/knowledge_engine/llm.rs"],
            context_summary="2 call sites in index.rs, both gated by config.enabled",
            progress=0.25,
        ),
    ))

    traj.append(observation(
        "bash",
        {"command": "cargo test", "description": "Run devbase test suite"},
        state(
            tags=["security-audit", "verified"],
            memory={"gated": "true"},
            active_files=["src/knowledge_engine/llm.rs"],
            context_summary="Run full devbase tests after comment-only change",
            progress=0.25,
        ),
        state(
            tags=["security-audit", "tests-passed"],
            memory={"test_count": "391", "failed": "0", "ignored": "3"},
            active_files=["src/knowledge_engine/llm.rs"],
            context_summary="391 devbase tests passed, 0 failed",
            progress=0.4,
        ),
    ))

    traj.append(observation(
        "bash",
        {"command": "git commit -m 'docs(p1): mark HV-1 as MITIGATED'", "description": "Commit P1 doc update"},
        state(
            tags=["security-audit", "tests-passed"],
            memory={"test_count": "391"},
            active_files=["src/knowledge_engine/llm.rs"],
            context_summary="Ready to commit HV-1 mitigation documentation",
            progress=0.4,
        ),
        state(
            tags=["committed"],
            memory={"commit_hash": "4e3d040", "commit_msg": "docs(p1): mark HV-1 LLM leakage as MITIGATED"},
            active_files=[],
            context_summary="Committed devbase P1 closure (4e3d040)",
            progress=0.5,
        ),
    ))

    # ------------------------------------------------------------------
    # 6. Jumpy prediction scenarios (using the new CLI)
    # ------------------------------------------------------------------
    traj.append(observation(
        "bash",
        {"command": "clarity-headless jumpy --skill file_read --params '{\"path\":\"README.md\"}' --state 'Need project overview' --provider ollama", "description": "Predict file_read outcome"},
        state(
            tags=["jumpy-predict"],
            memory={},
            active_files=[],
            context_summary="Want to predict state after reading README",
            progress=0.0,
        ),
        state(
            tags=["jumpy-predict", "predicted"],
            memory={"predicted_skill": "file_read", "provider": "ollama"},
            active_files=["README.md"],
            context_summary="Predicted: README loaded, tags=['file-loaded'], progress=0.2",
            progress=0.1,
        ),
    ))

    traj.append(observation(
        "bash",
        {"command": "clarity-headless jumpy --skill bash --params '{\"command\":\"cargo test\",\"description\":\"Run tests\"}' --state 'Need to verify build' --predictor hybrid --observations training-data/trajectories/observations.json", "description": "Predict test run with hybrid"},
        state(
            tags=["jumpy-predict", "hybrid-mode"],
            memory={"observations_loaded": "25"},
            active_files=[],
            context_summary="Using hybrid predictor with 25 historical observations",
            progress=0.0,
        ),
        state(
            tags=["jumpy-predict", "hybrid-mode", "predicted"],
            memory={"predicted_skill": "bash", "predictor": "hybrid", "confidence": "high"},
            active_files=[],
            context_summary="Predicted: tests pass, tags=['tests-passed'], progress=0.5",
            progress=0.1,
        ),
    ))

    # ------------------------------------------------------------------
    # 7. Memory / knowledge operations
    # ------------------------------------------------------------------
    traj.append(observation(
        "bash",
        {"command": "python training-data/scripts/export_sessions.py", "description": "Export baseline sessions"},
        state(
            tags=["data-pipeline"],
            memory={"task": "J10 Phase 1"},
            active_files=[],
            context_summary="Export historical sessions from clarity-memory DBs",
            progress=0.0,
        ),
        state(
            tags=["data-pipeline", "exported"],
            memory={"task": "J10 Phase 1", "exported_sessions": "19", "exported_facts": "15"},
            active_files=[],
            context_summary="Exported 19 sessions + 15 facts to training-data/baseline/",
            progress=0.3,
        ),
    ))

    traj.append(observation(
        "bash",
        {"command": "python training-data/scripts/generate_trajectories.py", "description": "Generate Phase 2 trajectories"},
        state(
            tags=["data-pipeline", "exported"],
            memory={"task": "J10 Phase 2", "baseline_count": "19"},
            active_files=[],
            context_summary="Generating high-quality hybrid trajectories for J10 Phase 2",
            progress=0.3,
        ),
        state(
            tags=["data-pipeline", "generated"],
            memory={"task": "J10 Phase 2", "trajectory_count": "25", "quality": "high"},
            active_files=["training-data/trajectories/observations.json"],
            context_summary="Generated 25 high-quality trajectories with realistic state transitions",
            progress=0.6,
        ),
    ))

    # ------------------------------------------------------------------
    # 8. Meta / planning operations
    # ------------------------------------------------------------------
    traj.append(observation(
        "file_read",
        {"path": "docs/ai-protocol.md"},
        state(
            tags=["planning"],
            memory={},
            active_files=[],
            context_summary="Check latest architecture decisions before next phase",
            progress=0.0,
        ),
        state(
            tags=["planning", "file-loaded"],
            memory={"last_read_file": "docs/ai-protocol.md", "j_series_status": "J5-J9 done"},
            active_files=["docs/ai-protocol.md"],
            context_summary="Loaded ai-protocol.md, confirmed J9 completion and J10 Phase 2 scope",
            progress=0.1,
        ),
    ))

    traj.append(observation(
        "file_read",
        {"path": "AGENTS.md"},
        state(
            tags=["planning", "file-loaded"],
            memory={"last_read_file": "docs/ai-protocol.md"},
            active_files=["docs/ai-protocol.md"],
            context_summary="Review workspace governance rules before committing",
            progress=0.1,
        ),
        state(
            tags=["planning", "verified"],
            memory={"hard_veto_checked": "true", "git_email": "noreply"},
            active_files=["AGENTS.md"],
            context_summary="Confirmed git noreply email and hard veto compliance",
            progress=0.15,
        ),
    ))

    traj.append(observation(
        "bash",
        {"command": "cargo check -p clarity-core --lib 2>&1 | findstr 'error'", "description": "Check for compiler errors"},
        state(
            tags=["build-check"],
            memory={},
            active_files=["crates/clarity-core/src/agent/jumpy/predictor.rs"],
            context_summary="Final compiler check after all edits",
            progress=0.8,
        ),
        state(
            tags=["build-check", "clean"],
            memory={"compiler_errors": "0", "warnings": "0"},
            active_files=["crates/clarity-core/src/agent/jumpy/predictor.rs"],
            context_summary="No compiler errors found in core library",
            progress=0.9,
        ),
    ))

    traj.append(observation(
        "bash",
        {"command": "cargo check -p clarity-headless 2>&1 | findstr 'error'", "description": "Check headless errors"},
        state(
            tags=["build-check", "clean"],
            memory={"compiler_errors": "0"},
            active_files=["crates/clarity-headless/src/main.rs"],
            context_summary="Final compiler check for headless binary",
            progress=0.9,
        ),
        state(
            tags=["build-check", "clean", "ready"],
            memory={"compiler_errors": "0", "all_crates": "ok"},
            active_files=["crates/clarity-headless/src/main.rs"],
            context_summary="Headless compiles cleanly, all crates verified",
            progress=0.95,
        ),
    ))

    # ------------------------------------------------------------------
    # 9. Error / failure scenarios (important for robust training)
    # ------------------------------------------------------------------
    traj.append(observation(
        "file_read",
        {"path": "nonexistent_file.rs"},
        state(
            tags=["bug-hunt"],
            memory={},
            active_files=[],
            context_summary="Trying to read a file that may not exist",
            progress=0.0,
        ),
        state(
            tags=["bug-hunt", "file-not-found", "error"],
            memory={"last_error": "File not found: nonexistent_file.rs"},
            active_files=[],
            context_summary="File read failed with 'not found' error",
            progress=0.0,
        ),
    ))

    traj.append(observation(
        "bash",
        {"command": "cargo test -p nonexistent-crate", "description": "Test invalid crate"},
        state(
            tags=["build-check"],
            memory={},
            active_files=[],
            context_summary="Accidentally targeted wrong crate name",
            progress=0.0,
        ),
        state(
            tags=["build-check", "error"],
            memory={"last_error": "package ID specification nonexistent-crate matched no packages"},
            active_files=[],
            context_summary="Cargo test failed: no matching package found",
            progress=0.0,
        ),
    ))

    traj.append(observation(
        "bash",
        {"command": "cargo test -p clarity-headless 2>&1 | findstr 'FAILED'", "description": "Check test failures"},
        state(
            tags=["build-check", "compile-ok"],
            memory={},
            active_files=["crates/clarity-headless/src/main.rs"],
            context_summary="Tests compiled but some may have failed",
            progress=0.5,
        ),
        state(
            tags=["build-check", "test-failures", "needs-fix"],
            memory={"failed_tests": "2", "failure_reason": "short-option-conflict"},
            active_files=["crates/clarity-headless/src/main.rs"],
            context_summary="2 tests failed: clap short option name conflicts (-s, -p, -o)",
            progress=0.5,
        ),
    ))

    traj.append(observation(
        "file_edit",
        {"path": "crates/clarity-headless/src/main.rs", "replacements": [{"old": "#[arg(short, long)]\n    state", "new": "#[arg(short = 'S', long)]\n    state"}]},
        state(
            tags=["build-check", "test-failures", "needs-fix"],
            memory={"failed_tests": "2", "conflict": "-s"},
            active_files=["crates/clarity-headless/src/main.rs"],
            context_summary="Fixing short option conflict for --state",
            progress=0.5,
        ),
        state(
            tags=["build-check", "fixed"],
            memory={"fixed_conflict": "-s", "remaining": "-p, -o"},
            active_files=["crates/clarity-headless/src/main.rs"],
            context_summary="Changed --state short option to -S, need to fix -p and -o",
            progress=0.55,
        ),
    ))

    traj.append(observation(
        "file_edit",
        {"path": "crates/clarity-headless/src/main.rs", "replacements": [{"old": "#[arg(short, long, value_enum)]\n    predictor", "new": "#[arg(short = 't', long, value_enum)]\n    predictor"}, {"old": "#[arg(short, long)]\n    observations", "new": "#[arg(short = 'd', long)]\n    observations"}]},
        state(
            tags=["build-check", "fixed"],
            memory={"remaining": "-p, -o"},
            active_files=["crates/clarity-headless/src/main.rs"],
            context_summary="Fixing remaining short option conflicts",
            progress=0.55,
        ),
        state(
            tags=["build-check", "fixed", "all-resolved"],
            memory={"fixed": "-s->-S, -p->-t, -o->-d"},
            active_files=["crates/clarity-headless/src/main.rs"],
            context_summary="All short option conflicts resolved",
            progress=0.6,
        ),
    ))

    traj.append(observation(
        "bash",
        {"command": "cargo test -p clarity-headless", "description": "Re-run tests after fix"},
        state(
            tags=["build-check", "fixed", "all-resolved"],
            memory={"fixed": "-s->-S, -p->-t, -o->-d"},
            active_files=["crates/clarity-headless/src/main.rs"],
            context_summary="Re-running tests after fixing clap option conflicts",
            progress=0.6,
        ),
        state(
            tags=["build-check", "tests-passed"],
            memory={"test_count": "8", "failed": "0", "fixed_conflicts": "3"},
            active_files=["crates/clarity-headless/src/main.rs"],
            context_summary="All 8 tests pass after resolving option conflicts",
            progress=0.7,
        ),
    ))

    return traj


def main() -> None:
    ensure_dir(OUTPUT_DIR)
    trajectories = generate_trajectories()

    # Write observations as JSON array (HistoricalPredictor ingest format)
    obs_path = os.path.join(OUTPUT_DIR, "observations.json")
    with open(obs_path, "w", encoding="utf-8") as f:
        json.dump(trajectories, f, ensure_ascii=False, indent=2)

    # Write metadata
    meta = {
        "export_time": datetime.now(timezone.utc).isoformat(),
        "count": len(trajectories),
        "quality_tag": "synthetic_real_hybrid",
        "description": (
            "Hand-crafted trajectories based on real tool behaviours in the "
            "clarity/devbase workspace.  State transitions are logically "
            "consistent with actual tool side-effects."
        ),
        "skill_distribution": {},
    }
    for obs in trajectories:
        sid = obs["skill_id"]
        meta["skill_distribution"][sid] = meta["skill_distribution"].get(sid, 0) + 1

    meta_path = os.path.join(OUTPUT_DIR, "_meta.json")
    with open(meta_path, "w", encoding="utf-8") as f:
        json.dump(meta, f, ensure_ascii=False, indent=2)

    print(f"Generated {len(trajectories)} trajectories")
    print(f"  -> {obs_path}")
    print(f"  -> {meta_path}")
    print(f"Skill distribution: {meta['skill_distribution']}")


if __name__ == "__main__":
    main()
