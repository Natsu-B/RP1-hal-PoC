# WDT7 expiry-to-early-entry candidate (HW OPEN)

Build `RP1_RTOS_FEATURE=freertos-r3-watchdog-expiry-entry tools/build-freertos-r1.sh /new/output`.
Use the matching `rp1-rtos-watchdog-expiry-entry` reader, not the WDT6 software
entry selftest. Official proc0 ARM_CM3 port and static R1 tasks are unchanged.
This image excludes proc1 and integrated R2 peripherals.

The disabled256us receipt and final ACK carry the same nonzero16-bit nonce.
Only after ACK, verify ROM scratch magic/SP/consumed entry against the ELF
vectors, publish cookie, republish only encoded-entry and verify exact tuple.
Then max LOAD/ENABLE and GPIO ARMED. No new reset/POWER/PLL parameter is written.
Only owned scratch may be reclaimed; unknown CTRL is never cleared.

Real early Reset must capture valid retained cookie/REASON before BSS, disable
only known CTRL, emit typeB nonce/reason and halt before PCIe/application init.
Malformed ARM/consumed ENTRY halts. No software branch to Reset is linked.
Absent cookie is not distinguishable from cold boot; it is not positive proof.

`python3 -B tools/test-expiry-entry.py` checks synthetic mixed16/32-bit frames,
handoff, nonce, reason, timing, corruption/truncation and old WDT4/5/6 regression.
`tools/check-expiry-entry.py UART TRACE BEFORE AFTER` requires complete ARM and
nonce-matching ENTRY with selected timer REASON1/3 and16..20s edge separation.
This is a selected external envelope, not a calibrated timer/ISR latency bound.
Surviving zero is left until19s then checked disable/owned cleanup; error,
ARM-only or zero-alive outcomes never pass the expiry validator.

Source/build success is not HW proof. Even a valid early-entry packet does not
establish warm scheduler/PCIe restart or unattended recovery. External ESP
recovery, fixed image admission and whole capture including overflow checks
remain mandatory. No Linux change is part of this image.
