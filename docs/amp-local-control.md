# Local control and observation primitives — Oct08 G, HOST / BUILD only

These primitives are not connected to deployed F's RPMsg endpoint, peripherals
or proc1. F still rejects future commands in its current dispatcher. Adding a
host-tested type does not establish a sensor-to-actuator deadline.

`local_control::LocalControl` holds current and next parameters, generations,
sequence admission, expiry and failsafe decisions in its owning RP1 actor. It
performs no DDR access, allocation or MMIO. Construct once for the admitted
boot/session; retirement and regressed time permanently close it. `step` returns
inactive for expired commands, stale/wrong-generation samples and repeated or
reordered sample numbers. The application must apply its declared safe output.
This primitive cannot choose the physical safe state of an unknown actuator.

The existing AMP64 command header is unchanged. Types8 SET_PARAMETERS and9 STOP
use a64-byte CTL1 payload: magic, version1, count4/0, flags0, four parameter words,
eight reserved zero words. The application validates all four meanings and ranges
before committing. SET can arrive ahead of its RP1 apply time. One pending target
is allowed; another valid SET returns Full without consuming sequence or changing
either target. STOP requires apply_time <= now and clears both targets even when
the local pending target is full. An upstream mailbox may still be full and must
report that condition rather than promise immediate receipt.

Eight tests cover header/schema/application mutations, future atomic application,
full behavior, STOP, expiry, samples and permanent retirement. The counter
`sample_sequence_gaps` is a gap between validated acquisition numbers, not an
independent IRQ-loss measurement. Sampling is accounted for on command expiry;
expiry is not silently reported as acquisition loss.
One combined host test keeps the collector stopped for10000 simulated decisions,
records9992 explicit drops, preserves the first eight records and still expires
the command. Those timestamps are simulated; this is not a5kHz hardware result.

`local_queue::LocalQueue<T,N>` has fixed local SRAM storage and non-cloneable
Producer/Consumer handles obtained through an exclusive split. N is a positive
power of two, at most2^30. Push/pop never wait. Full uses drop-newest, preserves
unread records, returns Full and increments a saturating u32 drop counter.
Release/acquire u32 loads/stores protect multiword slots; no compare-exchange,
fetch-add or shared exclusive monitor is required. **This is not a cache-coherent
Linux DDR queue.** Never reset storage while a handle lives; new sessions need
the deployment's stop/restart contract rather than zeroing live cursors.

Four host tests cover collector stall/1000 explicit rejections, index wrap,
saturation and100000 concurrent multiword copies. The actual Cortex-M3 probe has
four DMB instructions and no LDREX/STREX; its only queue helper is
`__aeabi_memcpy4`. Four validator tests reject removed barriers, an exclusive
instruction, an unadmitted helper and a missing probe. This is compiled
publication evidence; two RP1 cores and SRAM contention remain untested.

`rp1_abi::observation::Observation` encodes96 little-endian bytes as OBS1:
magic/version, boot/session, sequence, independent edge, IRQ entry, peripheral
completion, calculation completion, actuator update, RP1 sample acquisition time,
sync generation/uncertainty, flags, reserved0. All u64 values are low/high words.
Unknown sync uses generation0 and uncertainty u32::MAX. Established sync requires
a positive generation and finite uncertainty.

Flags1/2/4 mean independently captured edge, completed actual transfer and actual
actuator update; flag8 means established time synchronization. All three physical
boundary flags are needed for an end-to-end assessment. A timer-only record can
be encoded without them and cannot be promoted to sensor end-to-end evidence.
The codec validates length, generations, sequence, flags and time ordering;
hardware capture must still prove the actual boundaries.

Logs: `/opt/rpi-cm5-hack/logs/20261008-amp-control-G/`. The104 ABI/HAL host tests
and target library/codegen pass. Rebuilding F produces the identical ELF
`45178fb243790978de5fcc03d45f13531aea0e9b82d8cdeebb2bae343b27abdf`, with original
memory/vector/layout guards. These unused primitives do not change deployed F.
