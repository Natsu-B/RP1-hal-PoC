# WDL1 cold enabled-LOAD refresh candidate

Source-only reduced proof: actual hardware is **NOT PROVEN** by this change.
`freertos-r3-watchdog-refresh` selects the existing bounded arm receipt plus the
2304-word task pool and R1 synchronization pool. Seven R1 tasks (1792 words),
the sole slot7 watchdog owner (512 words), and the 4096-byte MSP stay unchanged.
Quiescence, post-ACK, reset/expiry entry, and warm feature combinations are
compile-time errors; there is no ACK or later long arm.

The exact eight-word request at words176..183 is
`[WQL1, 10, 1, 1, 0x00ffffff, 256, nonce, checksum]`, where nonce is 1..65535
and checksum is `0x57445432 ^ 10 ^ 1 ^ 1 ^ 0x00ffffff ^ 256 ^ nonce`.
The accepted nonce is published at word139 without arming a reset cookie.
Identity is WDL1/version10; old schemas are not refresh evidence.

The existing initial gate, first/second CTRL samples, 256us/100000-iteration
masked loop and words104..115 retain their meaning. Only enabled near-LOAD
descent with elapsed256..1000us and iterations1..100000 permits one additional
write of the SAME LOAD, a DSB, and one fresh CTRL read. That fresh value controls
the known-CTRL disable guard (top byte0 or0x40); refused refresh uses the second
sample for guarded cleanup. Unknown CTRL is never cleared speculatively.
No assertion, telemetry, RTOS call, allocation or panic-capable indexing is added
inside the armed interval.

Before restoring PRIMASK, the final CTRL must have a known disabled top byte0.
Otherwise execution remains in a masked call-free stop, including skipped
cleanup, failed disable and already-enabled initial state. No RTOS call or
panic is permitted without this confirmation. Recovery uses the external
bounded observer/controller path, not a speculative extra register write.

After disable/readback and PRIMASK restoration, the receipt additionally requires
fresh CTRL top byte0x40 and fresh low24 strictly greater than second low24.
The fresh sample is appended at word116; words117..127 remain zero. All 13 probe
words are then published, followed by existing resumed-work/stack checks and
the permanent disabled terminal state4 loop. No second request is consumed.

The elapsed field remains retrospective and excludes arm/first-read and
second-read/refresh/disable overhead. A stuck timer is iteration-bounded, not
protected against a stuck bus/CPU. New binary/timing admission and retained
real hardware receipts are required before claiming even the narrow upward
counter jump. This does not prove sustained post-refresh descent, repeated
feeding, task-health supervision, reset/expiry behavior, or warm integration.
