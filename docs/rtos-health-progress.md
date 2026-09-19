# Consumable warm health checkpoint (opt-in source integration)

## 2026-09-19 local monitor stack candidate: BUILD only

`freertos-r3-health-local-stack` adds an explicit proc0-only layout to the
existing health shadow. Normal build06 links ELF
`cc693d4008c275cdeb634f68a9d95a31c312d594464f863215a99e8f5b8c1f9c`.
Shared live data ends at `0x2000ddb4` (588 bytes below `0x2000e000`);
alignment padding makes the materialized shared payload exactly56KiB.
This is not hardware admission, health feeding, or a completed R3 result.

- Slot0/monitor keeps its512-word stack at proc0 DSRAM `10003800..10003fff`.
  Slots1..7 keep1792 words in shared SRAM: total2304 words is unchanged.
  The4KiB ISR/MSP stack and all task budgets are unchanged. No code relocation,
  staged copy, shared-SRAM expansion, or new watchdog write is introduced.
- This is NOBITS/PT_NULL, not a load destination. The pinned loader ignores
  its payload, but still validates header alignment. File-backed data is padded
  to8 bytes to satisfy that contract. `xTaskCreateStatic` fills every stack byte
  withA5 before initializing the exception frame, on every kernel start.
- Shared BSS clear excludes local DSRAM. The cold composite endpoint path
  precedes task creation; warm startup skips endpoint reinitialization. The
  legacy `pcie-ep-init` local initializer and proc1-worker feature are refused
  in this first opt-in variant. No runtime PCIe reinitialization is admitted.
- Fault capture accepts an additional32-byte frame only within this exact
  local stack. Existing shared frame bounds remain intact. Initial monitor
  telemetry checks its actual PSP range; actual context switching, warm refill,
  fault capture and high-water margins remain HW OPEN.

Build with `RP1_RTOS_FEATURE=freertos-r3-health-local-stack` using
`tools/build-freertos-r1.sh /new/tmpfs/output`. Intentional exit3 preserves a
fitting build while refusing hardware admission pending compiled-envelope review.
`tools/check-freertos-elf.py IMAGE --local-monitor-stack` checks the exact
NOBITS/PT_NULL region, all allocated-section ownership and both stack sizes;
default admission refuses local-stack images. Run `tools/test-local-monitor-elf.py`
with the same image for positive and mutation checks. `tools/test-static-stack.c`
exercises the unchanged shared budget plus the local slot allocation policy.
`tools/check-health-local-stack-elf.py --self-test IMAGE` pins this exact ELF
and independently reviews its changed cold masked interval:70 reachable
instructions, no calls, PRIMASK restored fromr10/spill28,100000-iteration bound.
This is not timing equivalence to BC12 and does not admit an observer or hardware.

The loader runner below now adds two real-LLD fixture tests (eight ordinary
tests total). `--image IMAGE` additionally tests the actual target ELF against
the unchanged pinned loader. These are HOST/BUILD tests, not ARM execution.
The previous staged-code/BSS overlay failed LLD's LMA overlap check even with
PT_NULL and `AT(0)`; no overlap check was disabled. That design is not used.

`examples/minimal/src/warm_health.rs` is a pure, allocation-free proc0 shadow
predicate. Feature `freertos-r3-health-shadow` connects it to the existing warm
monitor and persistent owner. The pure helper writes no register and performs
no MMIO. Integration adds existing read-only source/IRQ observations and telemetry,
not watchdog writes. Target fit and hardware acceptance are separate gates.

This follows the BC `next-health-contract.md` at
`/opt/rpi-cm5-hack/artifacts/20260913-064225-rp1-warm-persistent-owner/reports/`.
Reuse the current `warm_combined::finish` receipt/cleanup/buffer checks and the
fresh `freertos_r1::monitor` pass; do not make new drivers or weaken either path.

## Integration contract

- Initialize one `Cursor::new(nonzero_warm_epoch, r1_baseline)` only after the
  admitted warm epoch is established. Keep it exclusively in the proc0 consumer
  for that epoch. A generation reset or refused sample must not recreate it.
  Epoch identity is caller-owned; this module neither invents nor persists one.
- `Sample` is a fresh local observation, never an externally writable mailbox
  or a reconstruction from stored eligibility word161. Validate proc0/thread
  context and the warm epoch before constructing it.
- Bracket the owner fields with the existing barriers and word150 sequence.
  Supply words151..155 as generation, operation, deadline, checked count and
  last-checked tick, and receipt generations at126/134/142 in SPI/UART/I2C order.
  Read the final sequence after all evidence; odd/changed sequences refuse.
- `receipts_ok` means existing full receipt/payload/canary/error checks and the
  matching rolling receipt checks passed. `cleanup_complete`,
  `buffers_returned`, and `late_buffers_unchanged` must come from that owner's
  post-cleanup checked publication, after cancellation refusal and late-buffer
  observations. `active_generation == 0` alone proves none of these facts.
  Retain the existing source-specific cleanup and owner buffer lifetime rules.
- `quiet` requires all three real sources and owned NVIC enabled/pending/active
  bits quiet at both ends, not `healthy`'s allowance for a selected active bus.
  Sample real IRQ totals169..171 at both ends and checked totals156..158 inside
  the coherent owner observation. Both real samples must equal checked totals.
  ISR totals are not seqlock-protected: the explicit double sample is required.
- `r1_ok` is the result of the **current complete R1 pass**, including unchanged
  context, error, seven task HWM, owner HWM and MSP minimum checks. Supply its
  check tick and counters64/80/49/50. Every counter must differ from the baseline
  or last consumed R1 counters; a tick, idle count or copied true cannot replace
  spinner/queue/mutex progress. Do not obtain this flag from word161.
- Consume immediately at the same stable, completed safe point. Initial runtime
  integration uses the existing bounded g2/op7 handoff while the owner is
  blocked awaiting word159. Do not hand back to the owner and then act on an old
  sample. No waiting, unbounded retry, interrupt-masked MMIO loop, or delayed
  stored permission is introduced here. A missed checkpoint is refusal.

## Exact decision

Only quiet op0 or g2/op7 is eligible. All three checked receipts must have the
same new generation1..8 and checked count `3 * generation`. Each is therefore
newer than its generation at the previous consumption. Op6 UART ACK, op8 final
park, incomplete receipts/cleanup, active IRQ/transfer, incoherence, wrong core,
wrong/zero epoch and replay refuse without moving the cursor.

Deadline distance is strictly positive and at most the existing operation
budget:1000 ticks for op0,20000 for g2/op7. Last checked progress is at most35000
ticks old. R1 evidence is at most1000 ticks old (current monitor period), and its
counters must all progress again for a second consumption. Wrapping subtraction
handles a tick wrap; future timestamps, expired deadlines and half-range
distances refuse. These bounds assume the existing bounded warm commissioning
epoch, not gaps of a whole32-bit tick period. Changing cadence/bounds requires a
new reviewed candidate; they are not a service watchdog timeout.

`consume` returns one shadow event and immediately advances its private cursor.
It does not return a reusable hardware authorization. In particular, terminal
g8/op8 refuses even when observation word161 remains healthy. Neither acceptance
nor refusal authorizes reload, re-arm, expiry or reset during a live transfer.

## Actual integration and telemetry

The feature inherits the persistent workload and its exclusions: proc1, mixed
owners (including MD01), faults and standalone peripheral modes cannot be
combined. Monitor initializes one cursor from validated retained `EPOCH[1]` and
live R1 counters before the loop. Fresh thread-mode/MSP/PSP, recorded startup
sentinels and fault checks supplement each complete existing R1 pass. The hook
runs before readiness/packet serialization, not inside the twice-called readiness
helper. g2/op7 additionally requires actual READY, no handback, and no prior
acceptance. No caller consumes a transient op0.

One bounded observation brackets the checked publications and real IRQ totals
with source/NVIC quiet checks and the owner sequence. Post-cleanup/buffer facts
derive from the existing `finish` invariant, not reading dead stack buffers.
The monitor-owned words60..63 are BE01 magic, accepted count, immediate same-
snapshot replay-refusal count and coherent terminal-op8 refusal count. These
historical counters are never a reusable feed grant. Existing BC01 words and
type8 handoff remain unchanged; no new task, stack budget or live reset is added.

## Verification and remaining work

Current integration is experimental and **not deployable**: first complete
target link exceeded the fixed application SRAM by1580 bytes. An always-inlined
decision experiment increased it to1876; shared diagnostic failure handling
reduced that variant to1668. These failed builds are not hardware failures.
The decision body returned to one out-of-line copy; normal-05 still exceeds the
boundary by1372 bytes. The fixed GCC14.2.1 C Os/Oz comparison produced five
byte-identical objects (zero savings), so no new C flag is adopted.
ISR/task stacks, reservations and linker guards are unchanged. Do not
admit or merge this opt-in feature as a verified release until the fit and exact
linked-code boundary is resolved without weakening its checks.

The module has standalone `rustc --test` checks against the actual helper for
replay, one stale bus, tick-only progress, owner/IRQ races, cleanup/ownership,
R1 reuse, epochs, terminal operations, tick wrap and exact deadline/age bounds.
The compiler output and command are recorded under the BE artifact `build/`.
Tests use synthetic observations only and do not substitute for runtime evidence.

Target build/fit, exact linked-code review, independent deadline
and restoration review, and any actual hardware proof remain with the root
integrator. No firmware or hardware success is claimed by this source checkpoint.
Ponytail kept the change to the existing evidence contract plus a local cursor;
there is no watchdog abstraction, dependency, task, queue, or MMIO API.

## Host loader-contract check (not a memory-layout implementation)

`tools/check-rtos-staged-loader.py` compiles the pinned generic ELF parser and
includes the selected bootloader's `rp1_image.rs` intact. Only platform error
type/log plumbing is stubbed for host tests; no loader algorithm is copied.
Inputs are existing checkouts at elf97da4af74e67d9007decadb3682b53a808fff8c4 and
boot6ead136a9721148ec94e5d9cb7f838ea4162fc43. Used source/config paths are hashed
and checked unchanged; no fetch or source edits occur. Use pinned Rust1.96.0,
offline cached dependencies, and a new tmpfs output directory:

```sh
python3 -B tools/check-rtos-staged-loader.py \
  --elf-checkout /path/to/pinned-hypervisor-checkout \
  --boot-checkout /path/to/selected-bootloader-checkout \
  --out /dev/shm/rtos-loader-check-unique
```

The runner checks host disk/tmpfs capacity; additionally check RAM and per-user
tmpfs quota before running. Build jobs are one, debug info/incremental are off.
Host libtest requires `panic=unwind`; this applies only to the host parser build,
not the RP1 target or its C ABI. The runner does not edit target sources.

The original six tests cover physical staging with distinct VMA, zero-filled holes,
PT_NULL non-load handling, overlapping BSS PT_LOAD rejection in both orders,
local/out-of-window physical-load rejection, malformed sizes/alignment/file
ranges, local-entry rejection, and the generic64KiB loader's lack of RTOS
reservation policy. Test payloads are patterns and are never executed as ARM.

In particular, the generic loader accepts some destinations that overlap the
RTOS MSP/reservations. Its success is NOT permission to deploy. The current
VMA==paddr and belowe000 ELF guards stay intact. Runtime copy/zero order, local
code ownership and lifetime overlap remain OPEN for staged code. The newer
local-stack-only variant above avoids that mechanism; it has its own strict
BUILD opt-in and still requires hardware admission.
