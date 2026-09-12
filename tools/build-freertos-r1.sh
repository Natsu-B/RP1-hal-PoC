#!/usr/bin/env bash
set -euo pipefail
repo=$(cd -- "$(dirname -- "$0")/.." && pwd)
[[ $# == 1 && "$1" == /* && ! -e "$1" ]] || { echo 'usage: build-freertos-r1.sh /new/output/directory' >&2; exit 2; }
out=$1
feature=${RP1_RTOS_FEATURE:-freertos-r1}
case "$feature" in freertos-r3-watchdog-kernel-restart|freertos-r3-watchdog-expiry-entry|freertos-r3-reset-entry-selftest|freertos-r3-watchdog-late-disable|freertos-r3-watchdog-postack|freertos-r3-watchdog-quiescence|freertos-r3-watchdog-arm-receipt|freertos-r3-proc1-worker|freertos-r1|freertos-r1-critical-timing|freertos-r1-fault|freertos-r1-panic|freertos-r1-assert|freertos-r1-timer-irq|freertos-r1-periodic-200us|freertos-r2-spi|freertos-r2-spi-lifecycle|freertos-r2-spi-cancel-window|freertos-r2-i2c-nack|freertos-r2-i2c-peer|freertos-r2-i2c-cancel-window|freertos-r2-uart|freertos-r2-uart-lifecycle|freertos-r2-uart-overflow|freertos-r2-mixed|freertos-r2-mixed-repeat|freertos-r2-mixed-i2c-pair|freertos-r2-mixed-i2c-stream) ;; *) exit 2 ;; esac
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
if [[ "$feature" == freertos-r3-proc1-worker ]]; then
    cp "$repo/examples/minimal/src/freertos_proc1.rs" "$out/freertos_proc1.rs"
    rustc +stable --edition=2024 --test "$out/freertos_proc1.rs" -o "$out/proc1-test"
    "$out/proc1-test" > "$out/proc1-host-test.txt"
fi
export RP1_CONFIG="$repo/examples/minimal/rtos.toml"
if [[ "$feature" == freertos-r3-watchdog-kernel-restart || "$feature" == freertos-r3-watchdog-expiry-entry || "$feature" == freertos-r3-reset-entry-selftest || "$feature" == freertos-r3-watchdog-arm-receipt || "$feature" == freertos-r3-watchdog-quiescence || "$feature" == freertos-r3-watchdog-postack || "$feature" == freertos-r3-watchdog-late-disable ]]; then
    cp "$repo/examples/minimal/src/freertos_watchdog.rs" "$out/freertos_watchdog.rs"
    model_flags=()
    if [[ "$feature" == freertos-r3-watchdog-kernel-restart ]]; then
        model_flags+=(--cfg 'feature="freertos-r3-watchdog-expiry-entry"')
        cp "$repo/crates/rp1-rt/src/warm_data.rs" "$out/warm_data.rs"
        rustc +stable --edition=2024 -C strip=debuginfo --test "$out/warm_data.rs" -o "$out/warm-data-test"
        "$out/warm-data-test" > "$out/warm-data-host-test.txt"
        cp "$repo/examples/minimal/src/watchdog_kernel_restart.rs" "$out/watchdog_kernel_restart.rs"
        cp "$repo/examples/minimal/src/watchdog_reset_identity.rs" "$out/watchdog_reset_identity.rs"
        rustc +stable --edition=2024 -C strip=debuginfo --test "$out/watchdog_kernel_restart.rs" -o "$out/kernel-restart-test"
        "$out/kernel-restart-test" > "$out/kernel-restart-host-test.txt"
    fi
    if [[ "$feature" == freertos-r3-watchdog-kernel-restart || "$feature" == freertos-r3-watchdog-expiry-entry || "$feature" == freertos-r3-reset-entry-selftest ]]; then
        model_flags+=(--cfg "feature=\"$feature\"")
        cp "$repo/examples/minimal/src/watchdog_reset_identity.rs" "$out/watchdog_reset_identity.rs"
        rustc +stable --edition=2024 -C strip=debuginfo --test "$out/watchdog_reset_identity.rs" -o "$out/reset-identity-test"
        "$out/reset-identity-test" > "$out/reset-identity-host-test.txt"
        python3 -B "$repo/tools/test-reset-entry.py" > "$out/reset-entry-validator-test.txt"
    fi
    if [[ "$feature" == freertos-r3-watchdog-kernel-restart || "$feature" == freertos-r3-watchdog-expiry-entry || "$feature" == freertos-r3-reset-entry-selftest || "$feature" == freertos-r3-watchdog-quiescence || "$feature" == freertos-r3-watchdog-postack || "$feature" == freertos-r3-watchdog-late-disable ]]; then
        model_flags+=(--cfg 'feature="freertos-r3-watchdog-quiescence"')
        cp "$repo/examples/minimal/src/watchdog_quiescence.rs" "$out/watchdog_quiescence.rs"
        rustc +stable --edition=2024 -C strip=debuginfo "${model_flags[@]}" --test "$out/watchdog_quiescence.rs" -o "$out/quiescence-test"
        "$out/quiescence-test" > "$out/quiescence-host-test.txt"
        python3 -B "$repo/tools/test-watchdog-quiescence.py" > "$out/quiescence-validator-test.txt"
    fi
    if [[ "$feature" == freertos-r3-watchdog-kernel-restart || "$feature" == freertos-r3-watchdog-expiry-entry || "$feature" == freertos-r3-watchdog-postack || "$feature" == freertos-r3-watchdog-late-disable ]]; then
        model_flags+=(--cfg 'feature="freertos-r3-watchdog-postack"')
        if [[ "$feature" == freertos-r3-watchdog-late-disable ]]; then
            model_flags+=(--cfg 'feature="freertos-r3-watchdog-late-disable"')
        fi
        rustc +stable --edition=2024 -C strip=debuginfo "${model_flags[@]}" --test "$out/watchdog_quiescence.rs" -o "$out/postack-ack-test"
        "$out/postack-ack-test" > "$out/postack-ack-host-test.txt"
        cp "$repo/examples/minimal/src/watchdog_postack.rs" "$out/watchdog_postack.rs"
        rustc +stable --edition=2024 -C strip=debuginfo "${model_flags[@]}" --test "$out/watchdog_postack.rs" -o "$out/postack-test"
        "$out/postack-test" > "$out/postack-host-test.txt"
        python3 -B "$repo/tools/test-watchdog-postack.py" > "$out/postack-validator-test.txt"
    fi
    if [[ "$feature" == freertos-r3-watchdog-kernel-restart || "$feature" == freertos-r3-watchdog-expiry-entry ]]; then
        cp "$repo/examples/minimal/src/watchdog_boot_entry.rs" "$out/watchdog_boot_entry.rs"
        rustc +stable --edition=2024 -C strip=debuginfo --test "$out/watchdog_boot_entry.rs" -o "$out/boot-entry-test"
        "$out/boot-entry-test" > "$out/boot-entry-host-test.txt"
        python3 -B "$repo/tools/test-expiry-entry.py" > "$out/expiry-entry-validator-test.txt"
    fi
    rustc +stable --edition=2024 -C strip=debuginfo "${model_flags[@]}" --test "$out/freertos_watchdog.rs" -o "$out/watchdog-test"
    "$out/watchdog-test" > "$out/watchdog-host-test.txt"
fi
if [[ "$feature" == freertos-r2-mixed* ]]; then
    cp "$repo/examples/minimal/src/freertos_mixed.rs" "$out/freertos_mixed.rs"
    cp "$repo/examples/minimal/src/mixed_repeat.rs" "$out/mixed_repeat.rs"
fi
if [[ "$feature" == freertos-r2-mixed-i2c-pair || "$feature" == freertos-r2-mixed-i2c-stream ]]; then
    cp "$repo/examples/minimal/src/mixed_i2c_pair.rs" "$out/mixed_i2c_pair.rs"
    cp "$repo/tools/test-i2c-mixed-pair.py" "$out/test-i2c-mixed-pair.py"
    python3 -B "$repo/tools/test-i2c-mixed-pair.py" > "$out/pair-host-test.txt"
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
if [[ "$feature" == freertos-r3-watchdog-kernel-restart || "$feature" == freertos-r3-watchdog-expiry-entry || "$feature" == freertos-r3-reset-entry-selftest || "$feature" == freertos-r3-watchdog-arm-receipt || "$feature" == freertos-r3-watchdog-quiescence || "$feature" == freertos-r3-watchdog-postack || "$feature" == freertos-r3-watchdog-late-disable ]]; then
    python3 -B "$repo/tools/test-freertos-watchdog.py" > "$out/watchdog-validator-test.txt"
    python3 -B "$repo/tools/check-freertos-watchdog-elf.py" --self-test "$out/RP1.elf" > "$out/watchdog-elf-validation.json"
fi
sha256sum "$out/RP1.elf" > "$out/output.sha256"
if [[ "$feature" == freertos-r3-reset-entry-selftest ]]; then
    python3 -B "$repo/tools/check-reset-entry-elf.py" --self-test "$out/RP1.elf" > "$out/reset-entry-elf-validation.json"
fi
if [[ "$feature" == freertos-r3-watchdog-expiry-entry ]]; then
    python3 -B "$repo/tools/check-reset-entry-elf.py" --expiry-entry --self-test "$out/RP1.elf" > "$out/reset-entry-elf-validation.json"
fi
if [[ "$feature" == freertos-r3-watchdog-kernel-restart ]]; then
    python3 -B "$repo/tools/check-kernel-restart-elf.py" --self-test "$out/RP1.elf" > "$out/kernel-restart-elf-validation.json"
    python3 -B "$repo/tools/test-kernel-restart.py" > "$out/kernel-restart-validator-test.txt"
fi
date --iso-8601=seconds
