#!/usr/bin/env python3
"""Fail-closed JSON adapter for llm-verifier."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", required=True)
    parser.add_argument("--output", required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    input_path = Path(args.input)
    output_path = Path(args.output)

    try:
        payload = json.loads(input_path.read_text(encoding="utf-8"))
        candidates = payload["candidates"]
        if len(candidates) < 2:
            raise ValueError("at least two eligible candidates are required")

        import llm_verifier

        llm_verifier.USAGE.reset()
        config = payload.get("verifier", {})
        kwargs = {
            "criteria": payload["criteria_path"],
            "n_evaluations": config.get("n_evaluations", 4),
            "pivots": config.get("pivots", 2),
            "seed": config.get("seed", 0),
            "cache": payload.get("cache_path"),
            "progress": False,
            "on_error": "raise",
        }
        if config.get("model"):
            kwargs["model"] = config["model"]

        result = llm_verifier.select(
            problem=payload["problem"],
            candidates=candidates,
            **kwargs,
        )
        output = {
            "status": "SELECTED",
            "backend": "llm-verifier",
            "selected_index": result.index,
            "ranking": result.ranking,
            "scores": result.scores,
            "n_comparisons": result.n_comparisons,
            "criteria": result.criteria,
            "token_usage": llm_verifier.token_usage(),
            "on_error": "raise",
        }
        output_path.write_text(json.dumps(output, indent=2), encoding="utf-8")
        return 0
    except Exception as exc:  # fail closed at the process boundary
        print(f"llm-verifier failed: {exc}", file=sys.stderr)
        if output_path.exists():
            output_path.unlink()
        return 2


if __name__ == "__main__":
    sys.exit(main())
