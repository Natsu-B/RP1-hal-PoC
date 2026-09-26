# Optional endpoint observation under R1

`RP1_RTOS_FEATURE=freertos-endpoint-uart tools/build-freertos-r1.sh /new/output`
builds the plain proc0 R1+read-only SCMI diagnostic, with size optimization `s`.
The builder's exit3 means BUILD checks passed but hardware admission is withheld.
Use the selected ELF's layout for bootloader/DT/host-reader admission; old addresses
must not be reused. All SRAM/MSP reservations and assertions remain unchanged.

Before scheduling, UART0 adopts the already-checked50MHz clock and configures
GPIO14/15 plus PL0111152008N1, TX only. It does not retune clock/PLL. The monitor
takes sole UART ownership, uses the existing read-only DBI sampler and206-byte
ASCII encoder, and samples once per existing one-second pass. Maximum output is
32initial/changed records plus one CAP record. No standalone proof loop is called.
No new task or ISR, no interrupt mask, endpoint write, selector write or replay.

TX uses one FIFO attempt per byte and at most50one-tick blocked waits per line.
Telemetry words60/61 count accepted/dropped lines; word63 is EP01. FIFO acceptance
does not mean final serial edge. Partial lines must be rejected by the collector.
Other IO/fault/local-stack/timing/terminal-owner configurations are excluded.

Selector0 is checked before/after seven allowlisted configuration DWORD reads.
Other selectors skip the window. Selector ABA and concurrent host configuration
updates are not atomic snapshots; aperture sizing/masks are not observed. One
second sampling can miss intermediate transitions. UART serialization adds a
nominal17.9ms per206-byte line. This is coarse lifecycle diagnosis, not timing
accuracy, SCMI completion or runtime endpoint restoration proof.

Host tests cover exact read allowlist, nonzero selector skip, selector drift,
record cap and encoding. ELF/vector/SCMI layout checks remain required. Runtime
stack high-water, intact context and external UART bytes still need commissioning.
