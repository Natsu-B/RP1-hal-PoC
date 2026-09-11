#!/usr/bin/env bash
set -euo pipefail
repo=$(cd -- "$(dirname -- "$0")/.." && pwd)
[[ $# == 1 && "$1" == /* && ! -e "$1" ]] || { echo 'usage: build-freertos-r1.sh /new/output/directory' >&2; exit 2; }
out=$1
feature=${RP1_RTOS_FEATURE:-freertos-r1}
case "$feature" in freertos-r1|freertos-r1-critical-timing|freertos-r1-fault|freertos-r1-panic|freertos-r1-assert|freertos-r1-timer-irq|freertos-r1-periodic-200us|freertos-r2-spi|freertos-r2-spi-lifecycle|freertos-r2-spi-cancel-window|freertos-r2-i2c-nack|freertos-r2-i2c-peer|freertos-r2-i2c-cancel-window|freertos-r2-uart|freertos-r2-uart-lifecycle|freertos-r2-uart-overflow|freertos-r2-mixed|freertos-r2-mixed-repeat) ;; *) exit 2 ;; esac
mkdir -p "$out"
exec > "$out/build.txt" 2>&1
cd "$repo"
date --iso-8601=seconds
printf 'selected_feature=%s\n' "$feature"
git branch --show-current
git rev-parse HEAD
git diff --binary > "$out/source.diff"
git diff --cached --binary > "$out/index.diff"
export CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1
export CARGO_PROFILE_RELEASE_DEBUG=0 CARGO_PROFILE_RELEASE_STRIP=debuginfo CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1
# The lifecycle workload adds a second task-side path. Fix its size optimization
# explicitly; never enlarge the application into the MSP/reserved SRAM budget.
if [[ "$feature" == freertos-r2-spi-lifecycle || "$feature" == freertos-r2-spi-cancel-window || "$feature" == freertos-r2-i2c-nack || "$feature" == freertos-r2-i2c-peer || "$feature" == freertos-r2-i2c-cancel-window || "$feature" == freertos-r2-uart || "$feature" == freertos-r2-uart-overflow || "$feature" == freertos-r2-uart-lifecycle || "$feature" == freertos-r2-mixed* ]]; then
    export CARGO_PROFILE_RELEASE_OPT_LEVEL=s
else
    export CARGO_PROFILE_RELEASE_OPT_LEVEL=3
fi
printf 'rust_opt_level=%s\n' "$CARGO_PROFILE_RELEASE_OPT_LEVEL"
if [[ "$feature" == freertos-r2-mixed* || "$feature" == freertos-r2-i2c-cancel-window || "$feature" == freertos-r2-uart-overflow || "$feature" == freertos-r2-uart-lifecycle ]]; then
    # Link-time elimination across Rust crates; keep all panic/assert branches
    # and the independent official C kernel, MSP and static stack pool intact.
    export CARGO_PROFILE_RELEASE_LTO=fat
    printf 'rust_lto=%s\n' "$CARGO_PROFILE_RELEASE_LTO"
fi
if [[ "$feature" == freertos-r1-periodic-200us ]]; then
    # Test the actual firmware arithmetic; retain this new source in provenance
    # even before its first commit (git diff alone omits untracked files).
    cp "$repo/examples/minimal/src/periodic_200us.rs" "$out/periodic_200us.rs"
    rustc +stable --edition=2024 --test "$out/periodic_200us.rs" -o "$out/periodic-200us-test"
    "$out/periodic-200us-test" > "$out/arithmetic-test.txt"
fi
export RP1_CONFIG="$repo/examples/minimal/rtos.toml"
if [[ "$feature" == freertos-r2-mixed* ]]; then
    cp "$repo/examples/minimal/src/freertos_mixed.rs" "$out/freertos_mixed.rs"
    cp "$repo/examples/minimal/src/mixed_repeat.rs" "$out/mixed_repeat.rs"
fi
if [[ "$feature" == freertos-r2-uart-lifecycle ]]; then
    cp "$repo/examples/minimal/src/freertos_uart_lifecycle.rs" "$out/freertos_uart_lifecycle.rs"
fi
if [[ "$feature" == freertos-r2-uart-overflow ]]; then
    cp "$repo/examples/minimal/src/freertos_uart_overflow.rs" "$out/freertos_uart_overflow.rs"
fi
cargo +stable rustc --offline --locked --release --target thumbv7m-none-eabi \
  -p rp1-example-minimal --no-default-features --features "$feature" -- -C "link-arg=-Map=$out/RP1.map"
cp "${CARGO_TARGET_DIR:-$repo/target}/thumbv7m-none-eabi/release/rp1-example-minimal" "$out/RP1.elf"
arm-none-eabi-readelf -lSW "$out/RP1.elf" > "$out/readelf.txt"
arm-none-eabi-nm -n "$out/RP1.elf" > "$out/symbols.txt"
arm-none-eabi-objdump -d "$out/RP1.elf" > "$out/disassembly.txt"
arm-none-eabi-size "$out/RP1.elf"
python3 "$repo/tools/check-freertos-elf.py" "$out/RP1.elf" > "$out/elf-validation.json"
sha256sum "$out/RP1.elf" > "$out/output.sha256"
date --iso-8601=seconds
