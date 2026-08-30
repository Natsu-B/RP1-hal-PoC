#!/usr/bin/env sh
set -eu

set -- cargo run -p cargo-rp1 -- build --example minimal

if [ "${NO_DEFAULT_FEATURES:-0}" = 1 ]; then
  set -- "$@" --no-default-features
fi

if [ -n "${FEATURES:-}" ]; then
  set -- "$@" --features "$FEATURES"
fi

if [ -n "${RP1_CONFIG:-}" ]; then
  set -- "$@" --config "$RP1_CONFIG"
fi

"$@"

printf '%s\n' "${CARGO_TARGET_DIR:-target}/rp1/release/RP1.elf"
