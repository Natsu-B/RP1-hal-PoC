# Reserved DDR duplex API — software qualification, 2026-10-08

Latest hardware stage F (Oct08): ELF `45178fb2…` is deployed. A same-boot
pre-claim 64 KiB symmetric sweep and owned DDR/RPMsg/UART1/Linux-load cohort pass.
Evidence and full hashes: `/opt/rpi-cm5-hack/logs/20261008-amp-bulk-F/` and
[F report](/home/hotaru/rpi-cm5-hack-amp-work/reports/amp-bulk-full-range-20261008.md).
UART0 is heartbeat only; neither multiple peripheral payloads nor 5 kHz
sensor-to-actuator acceptance is proved. The following B/C/D build records
retain their historical scope.

New G [local control / observation primitives](amp-local-control.md) are host/target
build tested and remain disconnected from F's endpoint and hardware. F's rebuild
is byte-identical; its retired bulk session is unchanged.

The opt-in `freertos-amp-bulk-ddr-diagnostics` profile exposes SWP1 in DDR RPC
word59. Only unclaimed scratch supports bounded pattern operations6/7.
Configuration2/restore5 retain their exact iATU guards; selector exploration1
is denied in this profile. HELLO refuses an outstanding diagnostic RPC, and a
successful claim permanently denies all these RPCs, including after RETIRE.
The post-claim closure test sends an in-range read without mapping/writing DDR
on Linux; firmware rejects it with zero word-transaction counter increment.

This profile reuses `rp1-freertos/task-pool-2304`: seven R1 stacks1792 plus the
unchanged OpenAMP stack512 exactly fill the pool. Each individual stack and the
4 KiB MSP remain unchanged. Executable diagnostics use `0x2000d290..0x2000d797`,
after BSS/clock profile and before fixed SCMI `0x2000d7c0`; link/ELF guards reject
overlap. Five mutations of the actual ELF diagnostic section are rejected.
This layout does not create permission to use legacy markers or reserved SRAM.

F copies remain16 bytes/direction/pass. An RP1 reply recorded RX partial sum28
and TX Writing together; it does not expose TX cursor or PCIe latency. The
transport's observed stack margin was222 words, not a static call-path bound.
F's session is permanently retired after the commissioning test; do not retry
HELLO or force remoteproc remove/reset as a reconnect strategy.

Later Oct08 GPIO continuation: candidate C uses the same bulk wire contract,
with unique bank0 GPIO ownership and atomic OUT/OE aliases. ELF SHA
`b8daca66caac92e7165f8f16c9416384b102943d67c2e22d0a1d16add094c9ee` pairs with loader
`bd4dcafba0759144d42c20ceb8d9c1e04debfe8386bcfecab7e4802435952c5f` (not deployed).
Text/data/BSS totals are 41401/56/15284 bytes. Data ends at `0x2000a1b8`, 224
bytes before fixed BSS; OpenAMP state still ends at `0x2000dffc`, only 4 bytes
below the MSP boundary. The receive function's own frame remains 744 bytes;
whole-path stack and hardware high-water evidence are still required. The
candidate B record below remains a separate preserved build.

`rp1_hal::bulk::Duplex` now performs checked payload copies through
`ReservedDdrWindow`, as well as validating ownership metadata. The opt-in
`freertos-amp-bulk` image now calls it from the sole OpenAMP worker. This new
image was **BUILD PASS, hardware NOT_TESTED at candidate B**. The then-deployed
commissioning image used the old DDR test RPC and RPMsg echo endpoint. An echo is not an
ownership acknowledgement: the new client requires the `BKR1` response schema.

## Admission and lifetime

The sole transport task may construct `Duplex` only after verifying the existing
64 KiB reservation and all `ReservedDdrWindow::from_verified_region0` safety
requirements. It consumes that window. Disable the legacy DDR test RPC for its
entire lifetime; those two users cannot operate on region 0 together.

The boot/session generation must come from an admitted session, not be adopted
blindly from the first received packet. Generation checks in this API do not
establish endpoint identity, stop old DMA, or create a Linux memory reservation.
An owning Linux client must use the matching Device/non-cacheable mapping and
follow the publication contract. This is cooperative isolation, not protection
against Linux deliberately modifying its own physical memory.

On transport loss call `retire()`. All subsequent transfer/control methods then
reject without accessing DDR. This API has no reset or session-rebind operation.
Retirement stops this pump only; it does not stop peripherals or outstanding
transactions. Reclaiming memory after either processor resets still requires
the system-wide DMA/IRQ/output/quiescence contract.

## Buffers and publication

|Direction|Offset in region 0|Capacity|Producer|Consumer|
|---|---:|---:|---|---|
|Linux → RP1|0|32 KiB|Linux|RP1|
|RP1 → Linux|32768|32 KiB|RP1|Linux|

There is one buffer per direction, no shared read-modify-write index in DDR.
The two directions progress independently. The descriptor's `owner` always
names its producer; `state` conveys the handoff. Notifications carry 64-byte
little-endian `rp1_abi::amp::Descriptor` records, not payload bytes.

Linux finishes its input writes, orders/completes them using the admitted Linux
mapping/transport contract, and sends READY. RP1 calls `accept_rx` (or `control`),
then `read_rx` in chunks of 1..64 words. Only after the full advertised length
has been copied can `release_rx` return FREE. Linux must receive that FREE before
reusing the input buffer. Retain and retry a FREE notification if RPMsg is full.

RP1 calls `begin_tx` with WRITING and an acquisition timestamp, calls `write_tx`
in chunks of 1..64 words, then `publish_tx`. Publication performs **DSB, a
non-posted read in RP1's own half, DSB**, before returning READY. It deliberately
does not read Linux's unpublished half merely to drain posted writes. Linux
reads the data and echoes the exact descriptor with only state changed to FREE.
`acknowledge_tx` checks every descriptor field before releasing it. Retain and
retry READY when the notification queue is full; do not call `publish_tx` twice.

While a buffer is occupied, a fresh begin request fails without payload access.
The policy is drop-newest; `drops()` exposes saturating counters per direction.
Counters count rejected begin **attempts**, not distinct sensor samples. The
acquisition application must also maintain sample sequence/loss counters.

## Control message subset

`control(packet, now_rp1_us)` accepts exactly 128 bytes:

- 64-byte `Command`: version 1, boot/session generation, monotonic command
  sequence, apply time, valid-until, command type, payload length 64, zero flags
  and reserved word.
- 64-byte `Descriptor`, fully validated before its ownership transition.

Type 1 (`RX_READY`) hands input to RP1. Type 2 (`TX_FREE`) acknowledges output.
These notifications are immediate: a future apply time is rejected, and
`now >= valid_until` is expired. The clock argument must be RP1 monotonic time,
not a Linux-provided timestamp. A rejected message does not advance the command
sequence or release a slot. Duplicates and older sequences are rejected; forward
gaps are accepted. Descriptor sequence is checked separately for each buffer.

`Duplex::control` is the small notification-only API. The opt-in transport below
adds session admission, a synthetic producer, polling and explicit retirement.
There is still no trajectory scheduler or physical sensor producer.

## Opt-in OpenAMP endpoint

Enable `freertos-amp-bulk` in the minimal example, not merely the HAL feature.
It retains the existing endpoint name `rp1-control`, remote address 1024, and
requires exactly 128-byte little-endian messages. Only one Linux source address
and session can bind per firmware boot. The firmware's own boot timestamp must
match; HELLO does not establish firmware identity by itself. The Linux boot
preflight must already have admitted the sealed artifact and reserved memory.

|Type|Request descriptor|Effect|
|---:|---|---|
|0 HELLO|All zero; command sequence exactly 1|Validate header/generation/deadline, verify fixed iATU windows, claim region 0|
|1 RX_READY|Linux owner, READY, offset 0|Consume input asynchronously|
|2 TX_FREE|Exact returned output descriptor, state FREE|Release output; altered metadata rejected|
|3 TX_START|RP1 owner, WRITING, offset 32768|Generate deterministic commissioning data|
|4 QUERY_RX|All zero|Return latest input descriptor with current state and checksum|
|5 QUERY_TX|All zero|Return latest output descriptor with current state|
|6 RETIRE|All zero|Permanently stop both directions for this boot|

TX_START requires timestamp 0, sync generation 0 and uncertainty `UINT32_MAX`.
RP1 replaces timestamp with its current raw timer. Output word `i` is
`(descriptor_sequence_low + i) ^ 0xa55aa55a`, wrapping at 32 bits. This is test
data, not sensor acquisition, a tensor, or a synchronized clock measurement.

The 64-byte response header contains these 16 words, followed by a descriptor:

```text
BKR1, version=1, status, command_type, command_sequence[lo,hi],
rx_drop_attempts[lo,hi], tx_drop_attempts[lo,hi], rx_wrapping_sum,
retired, response_timestamp_rp1[lo,hi], copy_words=4, reserved=0
```

Status: 0 success; 1 malformed/version/flags; 2 generation/owner/source/binding;
3 sequence or changed ACK; 4 future/expired time; 5 full; 6 retired;
7 bounds/alignment/width; 8 internal; 9 fixed-window claim unavailable.
Before a direction's first transfer its descriptor is zero. Otherwise a query
returns its last descriptor with current state, including FREE. Rejections do
not consume the command sequence. Forward sequence gaps are accepted.

Each worker pass copies at most **four words per direction**. RX and TX do not
wait for one another or for a Linux notification. The existing bounded OpenAMP
pending-response queue retains the already processed reply when TX buffers are
full; retrying the queue does not re-execute the request. Transport failure or
quiescence retires the pump. A new HELLO after binding/retirement is rejected.

The example advertises DDR RPC capability `BKB1`, not old `BUL1`. Old payload
RPC operations are disabled even before HELLO. Successful claim sets DDR word
63 and disables selector/program/restore RPC operations for the rest of the
boot. Claim checks region 0, the exact OpenAMP region 1, disabled regions 2/3,
and selector restoration while interrupts are masked. It issues no DDR payload
transaction. There is no automatic iATU restore or RP1 restart on exit.

## Linux commissioning client

`tools/linux-amp-bulk.c` uses the existing standard `rpmsg_ctrl` / `rpmsg_char`
UAPI. Hardware mode is AArch64 only. Before creating an endpoint it requires:

- Exact enabled, no-map, non-reusable live FDT reservations: 64 KiB at
  `0x21000000` (payload), and four 16 KiB regions starting at `0x21100000`
  (OpenAMP vring0, vring1, buffers, resource table). Accepts two address cells
  and one or two size cells; rejects overlap, duplicate and overflowing ranges.
  Requires 16 KiB Linux pages, matching `/proc/iomem` payload reservation and RP1 PCI IDs.
- Exclusive `/tmp/ddr.lock`, no existing named endpoint, BKB1 capability,
  unclaimed and previously programmed region 0, and online OpenAMP.
- Matching nonzero boot generations in both SRAM interfaces, a fresh RP1 timer
  sample and a nonzero random Linux session nonce.
- A new `O_EXCL` backup file, fully written and fsynced before HELLO.

It maps only the fixed 64 KiB payload reservation via `/dev/mem` with O_SYNC.
BAR2 mapping is for admission and the bounded existing timestamp RPC. It tests
both full 32 KiB directions, invalid metadata/commands, full output rejection
100 times, reverse progress while output stays owned by Linux, and exact ACKs.
The test stops initiating work after 45 seconds; command validity is 60 RP1
seconds. Each message exchange is bounded to two Linux seconds.

On exit, restoration requires a successful same-generation RETIRE response.
On timeout or uncertain ownership, retain the backup and do not write it over
DDR. Endpoint destruction does not constitute DDR release. SIGKILL and reboot
recovery remain outside this client; no reboot test is authorized by its result.

```sh
gcc -std=c11 -O2 -Wall -Wextra -Werror tools/linux-amp-bulk.c -o /tmp/amp-bulk
/tmp/amp-bulk --self-test
aarch64-linux-gnu-gcc-14 -std=c11 -Os -static -Wall -Wextra -Werror \
  tools/linux-amp-bulk.c -o /tmp/amp-bulk-aarch64
# On the admitted CM5 candidate only, after region-0 setup and OpenAMP attach:
# /amp-bulk --admitted-scratch-64k /tmp/NEW-bulk-backup.bin
```

The flag attests the independent artifact/owner preflight; it is not permission
to use this client on stock firmware or the currently deployed echo image.

## Real-time boundary and verification

No method waits for Linux to reclaim a buffer or loops on RPMsg. The base API
allows at most 256 bytes per copy; this endpoint uses 16 bytes per direction
per pass. A PCIe load/completion itself may stall. **This bounds work,
not elapsed time.** Keep this pump out of the hard periodic task and IRQ, and
keep current/next command, control state, short sample queue, stack and failsafe
in RP1 local memory. Bus-contention effects still need hardware measurement.

Run the software checks from this worktree:

```sh
CARGO_TARGET_DIR=/tmp/rp1-amp-check cargo +stable test --offline --locked -p rp1-abi -p rp1-hal
CARGO_TARGET_DIR=/tmp/rp1-amp-check cargo +stable check --offline --locked --target thumbv7m-none-eabi -p rp1-abi -p rp1-hal
```

The Linux host test maps anonymous process-local memory at the RP1 virtual
aperture address using `MAP_FIXED_NOREPLACE`; it never opens `/dev/mem`. The
same HAL copy methods process the complete 64 KiB, interleaving both directions
in 256-byte chunks. It checks payloads, incomplete/oversized copies, 100 full
buffer rejections, independent reverse progress, malformed/stale/expired control
messages, wrong ACKs, duplicate release and retirement. This is a host test,
not hardware evidence for concurrent PCIe transfers or cache/ordering behavior.

The 2026-10-08 run passed 86 ABI/HAL tests, including the actual transport parser,
pre-claim rejection without invoking the claim callback, full duplex progression,
full-buffer counters, peer binding and retirement. The C client passed host
packet-socket checks (including echo/short-response rejection) and an AArch64
static build. Host memory does not prove PCIe ordering or latency.

Both default and opt-in firmware pass existing ELF/SCMI layout checks. The bulk
build also runs the actual bulk-configured admission-gate tests. Its OpenAMP
state occupies 1148 of 1152 reserved bytes, and initialized data ends eight bytes
before the fixed BSS. The image omits verbose DBI trace emission to fit fixed
SRAM. The admission gate, config receipt, heartbeat, queue capacity/full/empty/
timeout self-tests, actual task queue and stack/fault guards remain. Queue
assertions share a compact diagnostic; word 39 becomes 1 only after the actual
tests pass. Intermediate build 06 omitted them (marker 2), was incompatible with
the existing loader admission, and is **not the current candidate**.

Disassembly shows 744 bytes in `rp1_bulk_receive` alone, plus callers/callees.
This is not a complete stack bound; the existing 2048-byte worker stack needs
compiled-call-path review and fresh hardware high-water evidence. The previous
firmware's observed free-stack margin cannot be inherited by this image.
Build wrapper exit 3 is its existing **hardware admission REFUSED** gate after
successful compilation/layout checks. At this earlier software-only stage no
new image was deployed. Neither 5 kHz integrity nor Linux+DDR+multiple-peripheral
acceptance is proven by the subsequent F subset.

Dated hashes, failed links and raw build/test results are preserved under
`/home/hotaru/rpi-cm5-hack/amp-continuation-20261008/`. Earlier build 11/B was
`1d413466…`, GPIO candidate C was `b8daca66…`; their sealed source snapshots remain.
Latest candidate D uses generated Resources in the actual bulk example and
explicit fixture idle-pin ownership. A consumed `BootHandoff` releases only
Linux UART1 before Linux starts. The build records generated files and policy
hashes under `ownership/`; set `CM5_HACK_ROOT` to the research source checkout
containing `tools/amp-deployment.py` when running `tools/build-freertos-r1.sh`.
Other firmware features retain their existing build path.

D ELF is `563911b4…`, real owner DTB `e390a83c…`, paired loader `2b29aa5b…`,
client `a045c037…`, initramfs `fc7f42bd…`. Default-feature rebuild is `a901fe89…`.
Exact hashes and the 18-item report are in the research repository's
`reports/amp-deployment-20261008.md`. None has been deployed; retained A remains
`5065da2a…`. Both ELF and DT admission remain exact fixed hashes.

The B/C client's two-64-KiB-node assumption and 64-bit-only size decoding would
have rejected the real DT before MMIO. D fixes this; 11 checks export the actual
DT properties to exercise the same C parser. `--check-reservations TREE_PATH`
is read-only host checking; it never substitutes a fixture in hardware mode.
Packet self-test alone did not establish real-DT admission. Use the matching D
client/DT/ELF/loader together; no older candidate inherits these results.
