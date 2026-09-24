#!/usr/bin/env python3
"""Check the resolved kernel config, not just requested fragment text."""
import argparse
from pathlib import Path
import re


def config(path):
    result = {}
    for line in Path(path).read_text().splitlines():
        if match := re.fullmatch(r"(CONFIG_\w+)=(.*)", line):
            result[match[1]] = match[2]
        elif match := re.fullmatch(r"# (CONFIG_\w+) is not set", line):
            result[match[1]] = "n"
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("config", type=Path)
    parser.add_argument("--fragment", type=Path,
                        default=Path(__file__).resolve().parents[1] / "profiles/linux-standard.config")
    args = parser.parse_args()
    actual, requested = config(args.config), config(args.fragment)
    failures = [f"{key}: wanted {value}, resolved {actual.get(key, 'ABSENT')}"
                for key, value in requested.items() if actual.get(key) != value]
    if failures:
        raise SystemExit("\n".join(failures))
    print(f"BUILD config PASS: {len(requested)} symbols; no RP1 transport/glue claim")


if __name__ == "__main__":
    main()
