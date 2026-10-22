# RP1 OpenAMP platform

RP1-hal owns fixed outbound mapping, BAR2 control, mailbox notification and
posted-write completion in `crates/rp1-hal/src/openamp.rs`. `openamp/rp1.c`
adapts pinned upstream OpenAMP virtio/RPMsg through C ABI, with static queues
and one `rp1-control` endpoint. Wire format and ring ownership remain upstream.

The `prepare(boot_epoch)` / `service()` API has no FreeRTOS dependency.
The example uses one proc0 task, priority1, 512 stack words, polling every tick.
Equal priority preserves the existing low-priority mutex workload. Both original
context-checking spin tasks remain. No proc1 or ISR caller may enter OpenAMP.
The shared ISR ACKs mailbox channels1–3; channel0 SCMI is unchanged.

Build from the exact clean checkouts in
`$CM5_HACK_ROOT/tools/openamp-lock.json` (default root `/opt/rpi-cm5-hack`):

```sh
python3 tools/build-openamp.py --rp1 /new/absolute/library-output
RP1_OPENAMP_PREFIX=/new/absolute/library-output/install \
RP1_RTOS_FEATURE=freertos-openamp-rpmsg \
bash tools/build-freertos-r1.sh /new/absolute/firmware-output
```

`--lock` selects another lock-file location. No build fetches or edits upstream.
`--rp1` replaces generated mutex/sleep headers with single-owner checked locks
and real delay, enables ordering/cache hooks, and preserves assertions.
`rp1.c` supplies bounded IO/completion hooks. Unused template condition/IRQ code
remains in the archive but is not linked. The candidate ELF contains zero
LDREX/STREX instructions. The fixed remote device needs no dynamic allocation.

Linux uses unmodified `stm32-rproc` manual attach, `rp1-mailbox`,
`virtio_rpmsg_bus`, `rpmsg_ctrl` and `rpmsg_char`. Firmware implements the
provider's configurable state/resource-table/holdboot interface; this does not
identify RP1 silicon as STM32. Linux6.12 needs COMPILE_TEST to expose that
standard provider on arm64. There is no Linux C patch or custom module.

The sealed DT reserves PA0x21100000..0x2110ffff:16KiB each for vring0, vring1,
buffers and the resource table. Region1 maps that exact64KiB at M3 0x82000000,
PCIe 0x1021100000. Region0 and the established DDR reservation remain intact.
The backend admits only this pool's descriptors and seals the negotiated table.
Observed Linux buffer DMA addresses are CPU PA0x21108000..0x2110bfff.

M3 MPU TYPE=0x800, CTRL=0. Linux's reservation has no linear-map cacheable alias;
standard drivers map rings/table WC and use a dedicated coherent buffer pool.
The Cortex-M3 path has no data-cache maintenance controller. Hooks retain DSB
ordering and the proven same-window non-posted read completion before the host
doorbell. They are not empty stubs. The system default DMA pool is unchanged.

BAR2 control words at0xda40: holdboot, running-state, table-DA, command,
command-ACK, error, magic, worker-progress, mapping-ready, kick-count,
last-kick, online-state, epoch-low/high, notify-count, transport-generation.
Command1 maps only after Linux reservation admission;2 observes the negotiated
table;3 starts OpenAMP;4 quiesces and invalidates queues before virtio unbind.
Commands7/8 suppress/resume only queue0 notification for bounded exhaustion
commissioning. Callers must resume on error; SCMI/queue1 remain enabled.

The attached provider holds the enforced firmware re-entry latch low.
Automatic remoteproc recovery is disabled. **Do not stop/remove the provider
or trigger its watchdog reset handle**: M3-only restart is unverified.
Virtio driver unbind/rebind uses prior quiescence. Linux resets status0 then
restores ACK1 before unbind returns; old sequence rejection survives rebind.

Real IRQ through rpmsg_recv_done, NS,32/256/496B echo,100 messages, stale boot
rejection, buffer exhaustion and virtio rebind passed. Formal A/B each completed
10000 round trips with zero error deltas on the same candidate. Formal results,
exact seals and limitations are recorded in the canonical
`/opt/rpi-cm5-hack/reports/openamp-rpmsg.md`.
