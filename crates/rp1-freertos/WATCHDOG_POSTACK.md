# WDT4 post-ACK countdown — hardware qualification pending

The distinct [WDT5 late-disable control](WATCHDOG_LATE_DISABLE.md) stops while
the count is still nonzero; its A618 event must never be interpreted as ZERO.

Opt-in `freertos-r3-watchdog-postack` inherits the selected R1-only WDT3
256us enable/countdown/disable prerequisite. It does not include the integrated
R2 peripheral workload. WDT3 behavior and its 32-bit packet remain unchanged.

```
RP1_RTOS_FEATURE=freertos-r3-watchdog-postack bash tools/build-freertos-r1.sh /new/absolute/output
python3 -B tools/test-watchdog-postack.py
```

Use the paired host observer feature `rp1-rtos-watchdog-postack`. It uses WDT4,
WQ04 and QA04, version4/sequence1. The old WDT3/QA03 observer must not arm WDT4.
The observer verifies the disabled receipt and known GEM NCR stop fields,
records the receipt, writes QA04 magic last, and performs no later RP1 access.
The stop fields are not proof of all fabric/DMA requests having drained.

The existing monitor task validates QA04, PRIMASK0, disabled CTRL high fields,
TICKS3/CYCLES50 and REASON2. It then writes only known LOAD0xffffff and
CTRL.ENABLE bit30. No reset/POWER/routing writes are added. It verifies enable
and decrement, then waits for count0 in the selected nominal16.78s interval.
A raw20s deadline starts before enable (including the first packet), with a
20,000-iteration cap. Interrupts remain enabled. No new task, vector, memory
reservation, allocation or armed-path unwrap/assert is introduced.

GPIO22 is owned by the monitor. Packets are 16bits MSB-first, leadingLOW500ticks,
bitHIGH50/150 + LOW50, delimiterHIGH400 followed by a falling edge. ARMED=A21C,
ZERO_ALIVE_DISABLED=A31D, ERROR=A41A; XOR nibble with seed5 is diagnostic only.
Accept only ARMED then ZERO, require full capture with no lost/overflow events,
and validate the selected16..20s first-rising-edge interval. ERROR or ARMED-only
are explicit inconclusive results. Normal heartbeat stops after terminal state;
there is no second arm or second event sequence in the same boot.

Telemetry words152..169 hold initial CTRL/ticks/cycles/reason, enabled samples,
arm/start/end times, last count, elapsed/iteration, zero reason, disable readback,
packet completion, error code/disable validity. Words145..148 retain the ACK
receipt;148 is the legacy frame word, not the WDT4 packet. Host must not read
this post-ACK telemetry during the armed interval. Word98 states6/7/8/9 are
handoff/armed/zero-event-complete/error. Immutable short-probe words stay intact.

`tools/check-watchdog-postack.py UART TRACE BEFORE AFTER` consumes the full
transport envelope and suffix; no successful middle window can hide a later
partial packet. Symbol tolerance is25ms, timestamp resolution1us; neither is
an absolute accuracy or WCET guarantee. Expected140/192 events is based on an
observed prefix, not a universal bound. Overflow invalidates the experiment.

This can establish counter0, continued task execution and explicit disable,
not restart identity or reset propagation. Missing markers never prove reset.
Scheduler/MMIO stalls defeat a local task deadline: independent bounded ESP
normal recovery must remain available and logged for every hardware attempt.
Do not use this candidate as unattended watchdog recovery or feeding policy.
Linux source/config/kernel and extra equipment are not required or changed.
