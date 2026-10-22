# Bounded watchdog receipt candidate — HW OPEN

`RP1_RTOS_FEATURE=freertos-r3-watchdog-arm-receipt tools/build-freertos-r1.sh /new/output`
builds normal R1 plus one static owner task. No proc1/IO/fault-mode combination.
The corresponding opt-in bootloader observer issues one fixed WQ02 SRAM request.
It must report read-only=0; the default observer remains read-only.

WDT2 words96..175 are firmware-owned; words176..183 are host-owned and published
payload/checksum before commit token. The existing fault record184..255 and
official mailbox are untouched. The exact request is in freertos_watchdog.rs.
No generic MMIO, dynamic allocation, new IRQ or independent scheduler is used.

The selected prerequisite is CTRL0, REASON2, TICKS CTRL3/CYCLES50. The bounded
probe saves/masks PRIMASK, rechecks prerequisites, LOAD0xffffff, ENABLE bit30,
two countdown/time samples, explicit CTRL0, readback and PRIMASK restore.
There is no panic/log/RTOS API or unbounded loop in the enabled interval. Review
the actual linked probe before admission. Max-load margin is not itself a
guaranteed recovery policy. The inactive countdown value is not restored; the
accepted final state is disabled writable CTRL, unchanged reason, untouched
reset/POWER/scratch fields, followed by separate known-good recovery.

This is a retrospective same-run enabled/countdown/disabled receipt, not proof
that the host sampled the transient ARMED state. It does not implement feeding,
expiry, actual restart, reset-target routing or unattended recovery. Health
feeding must later cover the selected acquisition tasks and stopped-task cases.

`python3 -B tools/test-freertos-watchdog.py` runs self-contained synthetic
validator mutations. An optional R1 capture path also exercises its unchanged
field logic; no capture is bundled here. `check-freertos-watchdog.py` accepts
actual UART and optional external trace files. Never classify fixture success
as HW evidence. Source/BUILD acceptance precedes independent HW admission.
