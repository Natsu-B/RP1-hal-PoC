# WDT5 late-disable positive control — HW pending

Use `RP1_RTOS_FEATURE=freertos-r3-watchdog-late-disable` with the normal build
script, and paired observer `rp1-rtos-watchdog-late-disable`. ABI is WDT5/WQ05/QA05,
version5/sequence1. WDT3/4 features remain distinct; no old ACK can arm this mode.

Purpose: after WDT4's ARMED-only/inconclusive observation, exercise long RTOS
waiting and terminal reporting while avoiding nominal16.78s expiry. The same
maxLOAD/ENABLE and256us preflight are used; no reset/POWER field is added.

After ARMED=A21C, require CTRL highbyte40, remaining count1.5..2M, raw elapsed
15.0..15.2s, then explicitly disable and check readback. Actual disable completion
time must also lie in15.0..15.2s; a delayed completion reports error11, never
LATE_COUNT_DISABLED=A618. This terminal event is NOT ZERO=A31D or reset evidence.
The selected enabled phase has a raw16s timeout, including the first3.1s packet.
RTOS delay/preemption/MMIO stalls defeat a hard wall-clock bound; independent
ESP recovery remains mandatory. Terminal transmission occurs after disable and
is outside the armed-phase timeout. No new task, stack, vector or IRQ priority.

The normal GPIO validator accepts `--late-disable`, exact ordered A21C/A618,
first-edge delta15..15.2s, full suffix, count and overflow checks. First packet
qualification requires the known magic-leading ONE (125..175ms); boot's guarded
~39ms ZERO must not masquerade as its start. This is a selected diagnostic
protocol, not authenticated traffic. Existing WDT4 decoder refusal is retained
in historical evidence; reanalysis alone must not create formal success.

`python3 -B tools/test-watchdog-postack.py` exercises old framing negatives,
synthetic boot-noise reproduction, WDT5 type/version/timing rejection, and
explicit inconclusive forms. Rust host tests check cutoff/count/time boundaries.
Build/HOST tests are not hardware qualification. No Linux change or new equipment.
