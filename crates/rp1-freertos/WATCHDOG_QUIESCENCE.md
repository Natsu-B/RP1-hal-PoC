# WDT3 disabled observation handoff — HW OPEN

Build `freertos-r3-watchdog-quiescence` with tools/build-freertos-r1.sh.
It inherits the bounded watchdog receipt and its normal-R1-only feature guards.
This is instrumentation, not deliberate expiry, feeding or reset routing.

The first fixed WQ03/version3/sequence1 request performs the same maxLOAD,
256us bounded enable/countdown/explicit-disable probe. After the disabled
receipt and task-progress checks, the host observer records one snapshot and
publishes a fixed QA03/sequence1 observation ACK. It does not read ACK back;
the selected host caller halts immediately after the terminal message.

Firmware validates all8 ACK words only after state4/disabled receipt. It publishes
state6 to the existing GPIO22 monitor owner. No second GPIO handle is created.
Monitor emits state7/type1 packet A50101FF: magicA5/type1/sequence1/XOR-with5A.
MSB first; low preamble500ticks, bit HIGH50 or150/LOW50ticks, trailerHIGH400ticks.
Total packet routine delays5500ticks, then state8 and normal1s heartbeat resumes.
Only task delays are used; context/queue tasks continue. This changes the marker
cadence deliberately and is not a200us scheduling/performance test.

Words145..151 record ACK valid/sequence/time, frame, event start/end and count.
The immutable disabled receipt104..144 and fault184..255 are unchanged. Word98
ownership transfers from worker to monitor at state6; the worker never rearms.
This is one fresh boot/sequence1, not a persistent cross-reset epoch protocol.

`tools/test-watchdog-quiescence.py` supplies a synthetic UART/trace fixture and
65 rejection cases; the prior44 WDT2 cases still pass after sharing the disabled
record validator. Decoder requires exactly one typed frame and36 regular tail
edges; it does not interpret fast boot transients as a payload or reset event.
Timing tolerance is25ms for packet symbols, not an accuracy/WCET guarantee.

HW acceptance requires the actual disabled receipt, actual issued ACK and GPIO
packet. Host post-ACK silence is bounded source/caller-to-halt evidence, not an
arbitrary PCIe-traffic absence measurement. A GPIO stop alone is never a reset
proof. No Linux kernel/config/C/module/daemon or extra equipment is required.
