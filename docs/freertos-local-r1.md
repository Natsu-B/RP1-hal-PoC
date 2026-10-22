# proc0 local monitor stack: independent R1 control and fault probe

These opt-in examples reuse the official single-core FreeRTOS ARM_CM3 port,
the existing seven-task R1 workload, and the existing halt-only UDF probe.
They do not require a watchdog request, warm restart, proc1, or peripheral
peer. They are BUILD-tested candidates, not a new hardware-qualified release.

`freertos-r1-local-stack` moves only the monitor's 512-word stack to proc0
DSRAM `0x10003800..0x10003fff`. The remaining 1792-word task pool stays in
shared SRAM. MSP remains `0x2000e000..0x2000efff`. Static task creation fills
the local NOBITS/PT_NULL stack; the loader must not upload a payload there.
Do not combine this layout with the legacy local PCIe initializer or proc1
worker. Endpoint initialization precedes the scheduler as before.

`freertos-r1-local-stack-fault` adds the existing fifth-monitor-pass
`UDF #0x51`. Its naked handler records the exception and halts; it neither
advances PC nor tries to recover a task. Hardware admission requires an
independently observed subsequent known-good boot.

## Build and host checks

Use the repository's pinned toolchain and FreeRTOS revision; do not update
them for this probe. Use distinct new output paths on a sufficiently sized
tmpfs, and check both filesystem space and the user quota before building.

```sh
RP1_RTOS_FEATURE=freertos-r1-local-stack CARGO_TARGET_DIR=/dev/shm/rp1-r1-target \
  bash tools/build-freertos-r1.sh /dev/shm/rp1-r1-control-output
RP1_RTOS_FEATURE=freertos-r1-local-stack-fault CARGO_TARGET_DIR=/dev/shm/rp1-r1-target \
  bash tools/build-freertos-r1.sh /dev/shm/rp1-r1-fault-output
python3 -B tools/check-local-r1-record.py --self-test /dev/shm/rp1-r1-fault-output/RP1.elf
python3 -B tools/check-local-r1-record.py --control --self-test /dev/shm/rp1-r1-control-output/RP1.elf
```

Successful builds intentionally exit **3**, with BUILD/PASS JSON and hardware
admission false. Other failures are not this success convention. The ELF
check verifies the local segment, direct vectors, stack budgets, absence of
warm-entry code, and both naked fault code and its separate literal pool.
The fault variant also verifies the emitted UDF instruction.

The record checker accepts all 31 fixed observer copies, not a filtered
subset. It requires a healthy prefix, a full aligned local exception frame,
the ELF-derived fault PC, register patterns, and a stable halt record. Its
result is OBSERVATION_ONLY. It does **not** establish same-boot ELF identity,
GPIO behavior, complete R1 acceptance, observer integrity, or recovery.
Those joins must be checked independently before any hardware claim.

`--control` uses the shared R1 runtime validator for context, progress, tick,
queue/mutex and stack checks, then requires the monitor PSP in the local range
and HWM values below the actual seven task budgets. Both modes reuse its
fixed31-copy decoder, including rejection of mixed observer completion modes.
Control19 and fault22 synthetic refusal cases pass; they are not hardware runs.

This control/fault pair does not substitute for the combined R2 image,
BE warm-health/local-stack cohort, watchdog recovery, or R3 completion.
