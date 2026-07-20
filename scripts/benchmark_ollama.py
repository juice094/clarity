#!/usr/bin/env python3
"""Quantitative benchmark of Ollama models for Claw chat use case.

Measures for each model:
- time_to_first_token (ms)
- total_duration (ms)
- tokens_per_second
- output_char_count

Outputs a Markdown table and a JSON report under .clarity/ollama-benchmark.json.
"""

import json
import statistics
import sys
import time
import urllib.request
from pathlib import Path

OLLAMA_HOST = "http://localhost:11434"
REPEAT = 3
PROMPT = "Explain the difference between a stack and a queue in one paragraph."
# NOTE: This benchmark measures raw text-generation speed only. For agentic
# OpenClaw/Gateway use (system prompt + tool calling), run
# scripts/benchmark_ollama_agentic.py instead. Small models can be fast here
# but fail or loop when tools are present.
MODELS = [
    "llama3.2:1b",
    "llama3.2:3b",
    "phi3:3.8b",
    "gemma2:2b",
    "qwen2.5:1.5b",
    "qwen2.5:7b",
]


def chat_once(model: str) -> dict:
    body = json.dumps(
        {
            "model": model,
            "messages": [{"role": "user", "content": PROMPT}],
            "stream": False,
        }
    ).encode()
    req = urllib.request.Request(
        f"{OLLAMA_HOST}/api/chat",
        data=body,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    start = time.perf_counter()
    with urllib.request.urlopen(req, timeout=120) as resp:
        data = json.loads(resp.read().decode())
    end = time.perf_counter()
    total_ms = (end - start) * 1000
    content = data.get("message", {}).get("content", "")
    eval_count = data.get("eval_count", 0)
    eval_duration_ns = data.get("eval_duration", 1)
    load_duration_ns = data.get("load_duration", 0)
    prompt_eval_duration_ns = data.get("prompt_eval_duration", 0)
    tps = (eval_count / eval_duration_ns) * 1e9 if eval_duration_ns else 0
    # Approximate time to first token = load + prompt eval
    ttft_ms = ((load_duration_ns + prompt_eval_duration_ns) / 1e6) if prompt_eval_duration_ns else total_ms * 0.2
    return {
        "total_ms": round(total_ms, 1),
        "ttft_ms": round(ttft_ms, 1),
        "tps": round(tps, 1),
        "chars": len(content),
    }


def benchmark_model(model: str) -> dict:
    print(f"Benchmarking {model}...", flush=True)
    runs = []
    for i in range(REPEAT):
        try:
            result = chat_once(model)
            runs.append(result)
            print(f"  run {i+1}: total={result['total_ms']}ms ttft={result['ttft_ms']}ms tps={result['tps']}", flush=True)
        except Exception as e:
            print(f"  run {i+1}: FAILED {e}", flush=True)
            runs.append({"total_ms": None, "ttft_ms": None, "tps": None, "chars": None})
        time.sleep(1)
    valid = [r for r in runs if r["total_ms"] is not None]
    if not valid:
        return {"model": model, "error": "all runs failed", "runs": runs}
    return {
        "model": model,
        "runs": runs,
        "total_ms": {
            "mean": round(statistics.mean(r["total_ms"] for r in valid), 1),
            "min": round(min(r["total_ms"] for r in valid), 1),
            "max": round(max(r["total_ms"] for r in valid), 1),
        },
        "ttft_ms": {
            "mean": round(statistics.mean(r["ttft_ms"] for r in valid), 1),
            "min": round(min(r["ttft_ms"] for r in valid), 1),
            "max": round(max(r["ttft_ms"] for r in valid), 1),
        },
        "tps": {
            "mean": round(statistics.mean(r["tps"] for r in valid), 1),
            "min": round(min(r["tps"] for r in valid), 1),
            "max": round(max(r["tps"] for r in valid), 1),
        },
        "chars": round(statistics.mean(r["chars"] for r in valid), 0),
    }


def print_table(results: list[dict]) -> None:
    header = f"{'Model':<18} {'Total(ms)':<15} {'TTFT(ms)':<15} {'TPS':<12} {'Chars':<8}"
    print("\n" + header)
    print("-" * len(header))
    for r in results:
        if "error" in r:
            print(f"{r['model']:<18} {r['error']}")
            continue
        total = f"{r['total_ms']['mean']:.0f} ({r['total_ms']['min']:.0f}-{r['total_ms']['max']:.0f})"
        ttft = f"{r['ttft_ms']['mean']:.0f} ({r['ttft_ms']['min']:.0f}-{r['ttft_ms']['max']:.0f})"
        tps = f"{r['tps']['mean']:.1f} ({r['tps']['min']:.1f}-{r['tps']['max']:.1f})"
        print(f"{r['model']:<18} {total:<15} {ttft:<15} {tps:<12} {int(r['chars']):<8}")


def main() -> int:
    results = []
    for model in MODELS:
        results.append(benchmark_model(model))

    print_table(results)

    out = Path(".clarity/ollama-benchmark.json")
    out.write_text(json.dumps({"prompt": PROMPT, "repeat": REPEAT, "results": results}, indent=2))
    print(f"\nReport saved to {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
