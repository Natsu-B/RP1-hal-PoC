# WDT8 proc0 fresh-kernel candidate

Use the normal `freertos-r3-watchdog-kernel-restart` example feature, not an
artifact-only source copy. It depends on the WDT7 known expiry/start tuple but
replaces the early-entry halt with guarded new kernel initialization.

```sh
RP1_RTOS_FEATURE=freertos-r3-watchdog-kernel-restart \
  CARGO_TARGET_DIR=/dev/shm/rp1-wdt8-example-build \
  bash tools/build-freertos-r1.sh /absolute/new/output
```

FreeRTOS V11.3.1 commit3a22924e0a9ddbbc8b0758881c33b3422a5cc20d and official
GCC/ARM_CM3 port remain unchanged. `rp1-freertos/build.rs` rejects a changed
Rust1.96.0/ac68faa20 or GCC14.2.1 compiler. Proc0 only, O3 Rust, static objects,
1kHz tick after raw-timer CPU calibration, no dynamic heap/tickless/SMP.

Cold PT_LOAD `.data` contains DATA_SENTINEL, official critical nesting and the
LDR8/complement token. Before BSS clear it is copied into a linker-bounded NOLOAD
shadow (12-byte header + data length) after BSS, before the aligned inbound dummy
section. Initializers are not inferred from missing runtime copies. Cold capture
consumes LDR8 into RUN8; running test mutates its data sentinel deliberately.
Warm entry requires a valid nonce cookie and consumed token, validates the
shadow checksum, restores the initializers, consumes the token and clears BSS.

Vector/MSP/PSP contracts stay intact. Warm entry rejects active/pending/enabled
external NVIC state and unexpected core faults; it clears only known SysTick /
PendSV pending state, creates seven fresh R1 tasks and new queues/semaphores/
mutex, then uses the official SVC/PendSV/tick handlers. It bypasses PCIe init.
No warm watchdog task is created, hence no implicit second arm or feed policy.

One GPIO22 type-C packet is emitted after five new monitor passes, >=5000 ticks
and switches, both non-yielding register-pattern tasks, queue/inheritance
progress, clean context/error/sentinel checks and MSP canary checks. Packet
delays themselves block on the new kernel. Nonce/reason1 or3/XOR encoding differs
from WDT7's early typeB. The companion WQ08/QA08 bare-metal observer stops host
RP1 accesses after ACK; this is not a Linux kernel/config replacement.

The source-gated packet does not expose exact warm counters or stack usage.
Shadow integrity is accidental-corruption protection, not authentication or MPU
protection. Selected proof is not PCIe/peripheral recovery, full R2 recovery,
automatic feeding, or unattended safety. Missing/overflow/error packets refuse.
Use the separately frozen single-lock cohort with known held-image preflight
and final recovery; do not turn this into a generic watchdog reset command.
