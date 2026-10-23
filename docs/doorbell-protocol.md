# Linux → proc1 GPIO-INTF responder (build-only diagnostic)

Only `freertos-amp-proc1-no-hooks` **and** `freertos-amp-proc1-local-stack`
include this responder. Normal `park(2)` first completes the original GPIO IRQ
stop, output-low check and status publication. Any failure keeps the old WFE
park. The normal cohort/audit must pass before running latency measurements.
No hardware behaviour or latency is verified by this build-only task.

## Shared record

DB = **0x2000f600**, 16 little-endian u32 words (64 bytes), through Linux BAR2
at **0x1f0040f600** (`0x1f00400000 + DB - 0x20000000`). The main-session
16-word override supersedes the brief's original 64-word request.

| Word | Writer | Meaning |
| --- | --- | --- |
| 0 | C1 | `DBL1` = 0x314c4244, published last after initialization |
| 1 | C1 | Applied mode: 1 WFI poll, 2 ISRAM ISR, 3 busy poll, 0xffff stopped |
| 2 | C1 | State: 1 ready/live, 2 stopped |
| 3 | C1 | Record length: 16 words |
| 4 | Linux (C1 initializes to 1) | Requested mode, applied after a handled doorbell |
| 5 | C1 | Acknowledged doorbells, stops at 10,000,000 |
| 6 | C1 | TIMER low at entry, microseconds modulo 2^32 |
| 7 | C1 | Spurious entry count |
| 8 | C1 | Fault: 0 none, 1 unknown mode, 2 event limit, 3 INTS != 0x200, 4 GPIO9 event enables nonzero |
| 9 | C1 | Mode-1 wakeups without INTS bit9, wrapping counter |
| 10 | C1 | Current VTOR: original 0x2000be00 or local 0x10002800 |
| 11 | C1 | Copied ISR Thumb address: 0x10000001 |
| 12 | C1 | Copied ISR byte length, including literal pool |
| 13 | C1 | MSP at responder entry (local DSRAM stack) |
| 14 | — | Reserved, initialized to zero |
| 15 | C1 | INTS value on a spurious entry |

The read script dumps 32 words for context. **Words 16..31 are not DB**: they
are the first half of the previous relaunch record at 0x2000f640. Never write
them. The profiler record remains at 0x2000f700..0x2000f7ff; the full relaunch
record occupies 0x2000f640..0x2000f6ff. Profile LOCAL words 0..15 are initialized
and used only by the pre-run probe in no-hooks builds; no period-loop hooks
are compiled in. They can be reused after normal completion.

The linker limits profile code to 0x2000f000..0x2000f5ff, and the RTOS record
starts at 0x2000f800. The checker verifies those section/segment bounds. The
new code source occupies the inactive shared C1 stack reservation, between
guards at 0x2000bf40 and 0x2000c748. C0 initializes only those guards in this
diagnostic. C1 fills and switches to its local stack at 0x10002000..0x100027ff
before calling Rust; the ROM-return first entry uses the ROM's own stack.
The 80-entry local vector copy at 0x10002800..0x1000293f is aligned to 512 bytes
and lies above the live stack, below the ROM stack observed at 0x100030d0.
Production X keeps its original shared stack and contains no responder.

## Client sequence and timing

1. Require normal cohort/audit PASS, DBL1, state 1, fault 0 and mode 1.
2. Serialize requests: at most one outstanding doorbell. Write DB[4], finish
   the BAR2 write with the client's device-memory barrier/readback, then write
   **0x200** to PROC1_INTF SET (RP1 0x400d2114; Linux BAR1 0x1f000d2114).
3. Poll DB[5] for its next value with a bounded timeout. After observing ack,
   order reads before reading DB[6]. Wait for DB[1] to equal the requested
   mode before measuring in that mode. A mode-change event uses the old mode;
   discard it from the new-mode latency sample set. DB[1] publication follows
   the VTOR switch and barriers.
4. Request 0xffff and ring once to stop. Require DB[2]=2, DB[1]=0xffff, VTOR
   restored, owned INTE/INTF cleared and IRQ0 disabled/pending cleared. Unknown
   modes (including optional mode 4, which is not implemented) fault and stop.

Do not clear INTF from Linux while waiting for an ack. The client must not
change GPIO9 CTRL, other pins, reset/watchdog registers or the remoteproc
provider. C1 enables only PROC1_INTE bit9 using SET, clears only its INTF/INTE
bit using CLR and configures only its own NVIC IRQ0/VTOR. It verifies GPIO9
CTRL[27:20] is already zero after the original stop; it never changes the pin.

All modes use the same handler instruction sequence. Its first data read is
TIMER low 0x400ac028, stored to DB[6] before INTS validation. Valid entry clears
INTF bit9, DSB, increments ack, DSB. A non-0x200 INTS value records a spurious
entry, masks/disables IRQ0 and returns to the thread's stop path without ack.
The 10,000,000th valid event is acknowledged, records limit fault 2, and stops.
Timestamps are 1-us resolution; Linux round-trip and ISR-entry measurements
have different costs. The timer wraps about every 71.6 minutes.

Mode 1 keeps PRIMASK=1 around WFI and polling; mode 3 never sleeps and keeps
PRIMASK=1. Mode 2 calls the copied ISR through IRQ0/VTOR and uses a
**masked check/WFI → CPSIE/ISB → exception → masked check** interlock. This is
the concrete race-free implementation of the brief's enabled-WFI shorthand:
it prevents an ack just before sleep from indefinitely delaying a requested
mode change. Its latency includes the unmask/exception entry after WFI.
It should not be reported as an enabled-throughout-WFI latency measurement.

Only record initialization (16 words), ISR copy (fixed byte length), and vector
copy (80 words) loop during setup. The documented responder loops and final
WFE park intentionally have no wall-clock timeout. The event count bounds
handled events, not idle duration. No RPMsg/DDR waits are added.

## Main-session hardware checks

Cold boot and run the full existing cohort/audit first. Check normal state 2,
guards, GPIO9 events zero, DB metadata and no relaunch/profiler corruption.
Measure modes 1/2/3 with one request outstanding, verify exact ack increments,
zero faults/spurious entries, timestamp ordering/wrap and local VTOR/handler.
Test repeated mode transitions, the stop path, an unknown-mode negative case
and the event limit on separate cold boots. Confirm INTE/INTF and NVIC cleanup
without stopping/removing the Linux provider. Capture raw logs and timeouts;
do not infer hardware PASS from build checks.
