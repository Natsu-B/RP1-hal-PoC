# Proc0 static FreeRTOS bridge

UART/I2C interruptible Driver methods take `&self`, with `UnsafeCell` IRQ state
and `Cell` for the last receipt. Callers must retain only shared Driver borrows
across requests (including enclosing helpers); one proc0 owner, no reentrancy.
Inner Context references end before enabling the IRQ. SPI keeps its separate
stack-local transfer in UnsafeCell so the waiting closure does not hold an
exclusive transfer borrow. These are serialized IRQ accesses, not cross-core
locks. `UnsafeCell` does not relax `&mut` uniqueness.
`tools/check-uart-transmit-loop.py ELF` checks the pinned optimized overflow
image reloads IRQ-mutated terminal/error fields on each transmit iteration.
The old full-prompt overflow commissioning failed this check; a build PASS
alone does not establish corrected hardware overflow/rearm behavior.

FreeRTOS-Kernel V11.3.1, commit `3a22924e0a9ddbbc8b0758881c33b3422a5cc20d`,
is compiled from the unmodified `third-party/FreeRTOS-Kernel` submodule. The build
rejects another commit, tracked vendor edits, another ARM target, or a compiler
other than `arm-none-eabi-gcc` 14.2.1. `RP1_FREERTOS_CC`/`RP1_FREERTOS_AR` may
select installed tool paths, not silently select newer versions. Compiler,
archiver, Rust compiler, target and vendor provenance go to `OUT_DIR/toolchain.txt`.
No `cc` crate, heap implementation, downloaded build input, or alternate port is used.

## Runtime contract

Selected periodic example: `freertos-r1-periodic-200us` uses the same1kHz tick,
TIMER0 IRQ26/priority6 and eighth task priority5 for100000 absolute200us slots.
HALce657adc / ELFec6bffff reproduced final accounting2/2 with99676/99678 raw
timestamp acquisitions,324/322 drops and324/323 late completion samples. This
is a negative no-miss timing result, not5kHz peripheral throughput. Only this
feature lowers stack-monitor priority to1; timeout/assert predicates remain.
ISR sampled maximum53us, end→task216us, MSP288B, acquisition stack free208words.
Independent normal recoveries passed. Full IRQ WCET/critical-section maximum,
30minute soak and integratedR2 are still OPEN. Primary evidence is rpi-cm5-hack
artifacts/20260911-111500-rp1-rtos-periodic-200us (including retained failures).

Root runtime must point vectors **directly** to `vPortSVCHandler`,
`xPortPendSVHandler`, and `xPortSysTickHandler`, and install a valid vector-table
initial MSP. No C/Rust handler wrapper or exception trampoline. The official
port verifies the SVC/PendSV vector entries. Kernel configuration is single-core,
static-only, 1 kHz 32-bit tick, preemptive, time slicing, tickless off, timers off,
stack checking level 2 and assertions enabled. NVIC uses three priority bits:
kernel lowest priority 7 (`0xe0`), highest syscall-capable priority 5 (`0xa0`).
External IRQs using `from_isr` must be logical priority 5..=7; these APIs invoke
the official priority assertion and yield via `portYIELD_FROM_ISR` themselves.

Call creates before `unsafe { start(verified_cpu_hz) }`. The supplied core clock
must be 1 MHz..=1 GHz and divisible by 1000; the official port uses it for the
SysTick reload. This range is a validation bound, **not** a hardware clock claim.
At least one task must exist. Successful start never returns. Only proc0 may use
any kernel API/storage; proc1 is outside this kernel. All Rust kernel operations
are unsafe to make that processor/context/lifetime obligation explicit.
Task callbacks are `unsafe extern "C" fn(*mut core::ffi::c_void)`, must never
return/unwind, and must keep arguments valid for the whole scheduler lifetime.
Copy handles do not synchronize access to application data.

## Fixed lifetime allocation and API

All C pools are ordinary shared `.bss`: 8 task slots, each with one TCB and a
requested 128..512-word stack from a fixed2560-word, 8-byte-aligned pool;
a separate 128-word idle stack; 4 `u32` queue slots
with capacity 1..=16; 4 binary-semaphore slots; 4 nonrecursive mutex slots.
Task requests accept 128..=512 words and priority 0..=7. Slot indices are zero
based; task IDs used in telemetry are slot+1. Names are nonempty static C strings
of at most 15 bytes. Pools cannot be created after start, deleted or reused;
duplicate slots are rejected. No LDREX/STREX-based custom spinlock is present.

`Task::create`, `Task::notification_give`, `Task::notification_give_from_isr`,
`Task::priority`, `Task::stack_high_water` (words), `start`, `delay`, `tick`,
`current_task`, and `notification_take` cover tasks and notification index 0.
`U32Queue::{create,send,receive}` handles fixed-width messages. Binary semaphores
start empty, with `create/take/give/take_from_isr/give_from_isr`. Mutexes start
available, provide `create/take/give`, inherit priority, reject recursive takes
and nonowner gives, and have no ISR API. Mutexes are explicit synchronization
primitives, not Rust data guards. Use zero timeout to poll or `WAIT_FOREVER` for
indefinite waits (except `delay`, which always remains a finite tick delay).
Queue/semaphore timeout is `Ok(false)` or `Ok(None)`, not a kernel error.

## Telemetry hooks

Optional `critical-timing` uses the **unmodified** port through linker
`--wrap=vPortEnterCritical` / `--wrap=vPortExitCritical`; consumers must supply
both flags. The minimal example's `freertos-r1-critical-timing` feature does so.
Call `critical_timing::start()` once from the first privileged proc0 PSP task,
with PRIMASK/BASEPRI clear, no enclosing critical or scheduler suspension.
`snapshot()` returns six fixed-width words; nested sections count only once.
The snapshot itself contributes to the following sample. Counts saturate and
latch an error indicator instead of silently wrapping. No cross-core atomic,
new peripheral writer, heap, or wrapped exception handler is introduced.

This measures **outer task critical body elapsed time**, after the real enter
and before exit bookkeeping / real exit. It excludes pre-scheduler execution,
inline ISR/PendSV BASEPRI regions, and instrumentation entry/exit overhead;
higher-priority interrupts may contribute elapsed time. Raw timer resolution is
1us, not absolute clock-accuracy proof; each interval must be shorter than the
32-bit wrap (~71minutes). It is not the maximum full interrupt-mask time/WCET.
The initial normal-R1 example reserves words96..112 as `CT01`, includes a nested
bounded200us calibration, and publishes count/min/max/last/depth/saturation
between matching even sequence words101/110. The maximum **includes** that
intentional hold; it cannot establish a tighter application-only maximum.
Other task8/fault workload features are rejected until separately integrated.

```sh
cc -std=c11 -Os -Wall -Wextra -Werror crates/rp1-freertos/tests/critical_timing.c -o /tmp/rp1-critical-test
/tmp/rp1-critical-test
RP1_RTOS_FEATURE=freertos-r1-critical-timing tools/build-freertos-r1.sh /new/absolute/build-dir
python3 tools/check-critical-timing.py --elf /new/absolute/build-dir/RP1.elf
```

The host C test and linked-call checker are STATIC/BUILD checks, not hardware
proof. `check-critical-timing.py UART_LOG` is an additional CT01 validator;
the enclosing normal-R1 context/sync/stack and external-GPIO validators must
also pass on that same run. Current CT01 hardware acceptance remains OPEN.

An application may replace these weak C symbols with strong `#[unsafe(no_mangle)]`
Rust `extern "C"` definitions (no task APIs, allocation or unwind inside hooks):

```c
void rp1_freertos_tick_hook(void);
void rp1_freertos_switch_hook(uint32_t task_id);
void rp1_freertos_fault_hook(uint32_t reason, uint32_t detail); /* noreturn */
```

Tick runs from SysTick. Switch runs inside scheduler selection, including the
initial selection before the first task. Task ID is slot+1, `0xffffffff` for idle,
or 0 if unknown. Fault reason 1 means assertion (`detail` is C source line),
2 means stack overflow (`detail` task ID), 3 means scheduler returned. A replacing
fault hook must disable interrupts and never return. The default fault hook
disables interrupts and waits forever. These callbacks are telemetry only,
not replacement exception handlers. Treat asserted/faulted runs as failed.

## Checks

```sh
CARGO_INCREMENTAL=0 cargo test -p rp1-freertos --target x86_64-unknown-linux-gnu
CARGO_INCREMENTAL=0 cargo check -p rp1-freertos --target thumbv7m-none-eabi
```

Host builds intentionally contain no kernel: the single host test checks only
Rust argument/error/opaque-handle boundaries. ARM checking compiles the four
official kernel sources plus the bridge with `-Wall -Wextra -Werror`; it does
not prove task scheduling or hardware behavior. Final firmware link/vector/
memory-budget checks and hardware evidence are the enclosing runtime's job.

## Mixed SPI / I2C NACK / UART selected workload

Opt-in `freertos-r2-mixed-repeat` is the finite sustained-load candidate:384
SPI/UART requests. Its monitor now uses official `delay_until(&mut previous, 1000)`
instead of adding stack-scan time to a relative delay. Initialize `previous` once
from `tick()`. The API accepts1..0x7fffffff ticks and updates the anchor even when
already due; it requires task context and the module's no-critical-section contract.
The mixed monitor counts/rebases a missed cycle, never burst-catches-up. MD01 words
60..63 are no-block count, maximum bookkeeping-through-GPIO us, maximum wake-late
ticks, and magic. IO/fault telemetry is unchanged. Other example modes keep their
existing cadence. This does not widen the original external marker tolerance.
`tools/test-delay-until.py` compiles the actual bridge and official kernel function
with scheduler mocks for input/wrap/due tests; it does not emulate task switching.
`tools/check-freertos-monitor-deadline.py LOG` adds MD01 checks to the existing
mixed-repeat numeric checks; an actual external peer/GPIO cohort is still required.

The sustained peer contract remains384
SPI/UART requests,192 absolute10s cycles with request2 at+4500ms (~32min), and
I2C0x2e NACK requests every100ticks until both owners finish. UART tags0001..0384
are monotonic, SPI wire IDs alternate1/2. A release/acquire load/store handshake
publishes input-high SPI readiness before UART READY, with no exclusive/RMW loop.
SPI waiting is bounded10100ticks; UART requests4500ticks; start lateness>=100ticks
halts rather than silently stretching the schedule. Host common request budget
remains4s (not the inadmissible early1s prototype). No maximum-throughput claim.
SPM2/ICM2/UAM2 ledger uses96..119/120..151/152..183, preserving184..255 for faults.
Rolling receipts/maxima and every-request payload checks replace growing arrays.
Build with `RP1_RTOS_FEATURE=freertos-r2-mixed-repeat tools/build-freertos-r1.sh OUT`;
run `python3 tools/test-mixed-repeat.py` for the actual arithmetic checks.
Host repeated-peer/trace/observer/controller admission and hardware remain OPEN.
This candidate is not covered by the old selected2-frame hardware evidence.

HAL0eff4ff8/image2de44161 later passed selected HW2/2 with two real SPI frames,
two UART payloads and256 I2C NACK requests under one image. Six separate normal
boots and exact ESP restore passed. This is not three-bus successful payload
reception or continuous R2 acceptance. Evidence: rpi-cm5-hack commit7bcf42be0.
The source/build-only notes below describe initial admission, not full R2.

`freertos-r2-mixed` is a dedicated proc0 example, not the union of standalone
task8 features. One shared PLL prerequisite runs before the three peripheral
setups and scheduler. Separate permanent owners reserve notification0; ISR
vectors24/35/41 call the existing I2C1/SPI0/UART0 adapters at priority6 (`0xc0`).
No Linux, proc1, extra equipment, allocator or new scheduler is involved.

| Task | ID | Priority | Stack words |
|---|---:|---:|---:|
| Monitor | 1 | 4 | 256 |
| Non-yielding register-pattern spinners | 2,3 | 1 | 128 each |
| Queue consumer / producer | 4,5 | 3 / 2 | 256 each |
| SPI / I2C NACK / UART owner | 6,7,8 | 5 | 512 each |

Total2560 words fits the existing pool exactly; idle128 words and MSP4096 bytes
remain separate. The standalone mutex pair is deliberately absent: this image
must not claim an inheritance stress result. Monitor stack reduction is a new
candidate budget, informed by earlier watermarks, still requiring mixed HW.

```sh
RP1_RTOS_FEATURE=freertos-r2-mixed bash tools/build-freertos-r1.sh /new/output
python3 -B tools/test-freertos-mixed.py
python3 -B tools/check-freertos-mixed.py /path/to/numeric-observer-records.txt
```

Build fixes Rust opt-level `s`, fat LTO and one codegen unit; C remains `-Os`.
`CARGO_TARGET_DIR` can select an existing RAM-backed build directory. The first
non-LTO link exceeded reserved memory and was rejected. Candidate LTO build:
text38472/data8/BSS13208 bytes, PT_LOAD end`0x2000d000`, unchanged MSP
`0x2000e000..0x2000f000`; direct RTOS/three IRQ vectors, no exclusive instructions.

Finite workload: two existing SPI frames, two real19-byte UART replies and256
I2C reads to the known unassigned0x2e address, verifying NACK/quiet cleanup/rearm.
I2C successful RX uses a different peer handshake on GPIO9 and is NOT enabled
with SPI MISO. Current retained ESP0.6.9 lacks the SPI peer service: hardware
admission must select the already-proven temporary peer with fresh backup and
checked restoration; do not send its commands to0.6.9 or infer compatibility.

Telemetry96..127/128..159/160..191 belongs to SPI/I2C/UART respectively;
192..255 remains fault-owned. SPI receipt start/end must fall inside the
corresponding UART armed request. This is request overlap, not simultaneous
wire payload. External SPI bit/OE records, UART peer receipts and GPIO witness
are required in addition to numeric validation. Preserve the existing I2C/UART
cleanup timing assertions under load. Timeout/cancel races, continuous mixed
stress and full R2 acceptance remain OPEN.

## SPI lifecycle workload

HAL1c8f9f4/image de7c37bc subsequently passed selected completed-cancel window
2/2 with four separate normal boots (rpi-cm5-hack3e5212db9). Injected delay was
1357us, rearmed generation4 four IRQs/1603us; input-only MISO is not payload.

## Optional I2C completed-cancel workload

`RP1_RTOS_FEATURE=freertos-r2-i2c-cancel-window tools/build-freertos-r1.sh /new/out`
uses opt-s/fat-LTO within the original SRAM/MSP budget. Proc0 only. The adapter
hook runs after terminal IRQ masking, BEFORE ticket withdrawal and cleanup;
the I2C controller may still be enabled. A two-tick sleep permits monitor4 to
cancel owner5's published generation2. The owner must return Cancelled and only
copy actual31,4e into its buffer after checked cleanup. Existing fatal precedence
remains. Old ticket reject, buffer stability and separate generation3 NACK rearm
are checked after the native ESP0.6.9 READYACK release. No new peer service.

RI02 READY stays compatible with the existing host client, but final count3 and
ICW1 extension at word182 require a new validator. Gen3 receipt96..111 and probe
112..122/180..183 do not touch ASSERT184..191/fault192..255. Existing incompatible
timer/SPI/UART example checks remain. Ordinary builds omit this hook entirely.
HALc9b19dc/image e0dc3883 passed its own selected HW2/2 and four separate normal
boots (rpi-cm5-hack616884350). Actual31,4e retained after Cancelled, gen3 NACK
rearm; ISR278/279us, wake48us, MSP808B. It is a test seam, not a production delay
or proof of arbitrary slave/active-cancel recovery.

## UART lifecycle workload

P selected source3b4772bb/image1706e912 subsequently passed formal2/2 with
four independent normal boots: empty timeout, accepted cancellation, then
actual19B payloads in requestgen3/4. This does not prove all UART lifecycle cases.

`RP1_RTOS_FEATURE=freertos-r2-uart-lifecycle tools/build-freertos-r1.sh /new/out`
adds a hook-free no-response20tick timeout and active cancellation before the
existing two USB-UART payload exchanges. Monitor waits boundedly for published
tickets, since exchange preflight cleanup can block before publication. It also
observes generation1's armed CR/IMSC/NVIC state before timeout. Empty prompts
keep the existing host peer unchanged; wire1/2 are driver generations3/4.
Failed requests retain no RX/error/overflow/residual and require checked quiet
cleanup, stale-ticket rejection and stable buffers. Source `uart0.rs` unchanged.

UL01 uses failed receipts96..119, monitor ledger120..122, existing context123..133,
canaries134..135 and successful receipts136..183; ASSERT/fault184..255 stay free.
Only the optional example uses this schema. Build uses existing opt-s/fat-LTO
within the original task/MSP budget. The fixed-image cohort above is selected;
partial-data cancel, overflow/error and fullR2 are not implied.

## UART software-ring overflow candidate

`RP1_RTOS_FEATURE=freertos-r2-uart-overflow tools/build-freertos-r1.sh /new/out`
uses the production driver unchanged. A1024B prompt starts with
`RP1U0 OVERFLOWREADY 0001\r\n`, then ASCII x padding and `\r\nEND\r\n`.
The external USB-UART peer sends one burst bytes0..79 while the owner is still
transmitting; expected RX96 exceeds the burst and the64B ring. Only Overflow
with actual IRQ41 and no physical RSR/DR error passes. Checked cleanup precedes
`\r\nRP1U0 OVERFLOWOK 0001\r\n`; a proper partial prompt is expected, not the
whole1024B. A late peer/full prompt must fail, not silently change the window.
Captured first64bytes, remaining32sentinels/canaries and post-return stability
are checked. Subsequent existing normal wire1/2 payloads are requestgen2/3.

Overflow TX waits via delay(1), so higher_priority_wakes may be0; terminal
IRQ-to-owner resumption is not called IRQ-caused wake. Normal rearms still
require positive IRQ-caused wakes. UO01 receipt96..114, buffer evidence115..121,
context123..133, normal canaries134/135 and normal receipts136..183 leave
ASSERT/fault184..255 reserved. P and Q features are mutually exclusive.
HW OPEN. This is not physical framing/overrun/BREAK recovery or sustained load.

## SPI lifecycle detail

`RP1_RTOS_FEATURE=freertos-r2-spi-lifecycle tools/build-freertos-r1.sh /new/output`
uses fixed Rust size optimization `s` (C remains `-Os`); earlier workloads keep
Rust `3`. No application/MSP/stack budget is enlarged. This optional eighth task
tests partial1tick timeout, lower-priority task cancellation, notification drain,
buffer quiet interval and successful generation3 IRQ rearm on input-only MISO.
It does not replace the separate ESP sequence/payload test or prove all R2.
`Receipt::irq_end_to_task_us` is zero when RxComplete was not observed.
Preparation errors after possible MMIO halt with local IRQ masked: the current
HAL cannot return a checked-abort handle on setup error. Only pre-MMIO argument
errors return normally; automatic recovery of all setup errors remains OPEN.

All three adapters settle an accepted cancellation after withdrawing the active
generation, before returning the caller's notification/buffer ownership. This
also covers preemption between the last successful completion check and
withdrawal. Existing errors are retained; I2C cleanup fatal evidence outranks a
late cancellation. SPI generation exhaustion halts before MMIO instead of
recycling an old ticket. `python3 tools/test-spi-cancel.py` compiles the actual C
transaction and Rust settlement/generation blocks with deterministic host checks.
These are STATIC checks, not hardware coverage of every cancellation interleaving.

`freertos-r2-spi-cancel-window` is an opt-in acceptance workload, not a production
mode. Only generation3's checked-success path calls an example-owned probe before
withdrawal. It sleeps2ticks so the lower-priority monitor can accept a cancellation
of the completed request; the owner must return Cancelled. Generation4 must rearm
successfully after notification/buffer cleanup and rejection of the old ticket.
Normal builds contain no probe call. MISO is input-only/all-ones, not real payload.

## Controlled C configASSERT acceptance

`RP1_RTOS_FEATURE=freertos-r1-assert bash tools/build-freertos-r1.sh /new/output`
selects the deliberate monitor-task assertion. The `assert-probe` C feature
enables official vTaskPrioritySet only for this image; the wrapper calls it with
NULL/current task and invalid priority8. The pinned kernel's tasks.c2857
configASSERT invokes reason1 diagnostics and a masked halt. An unexpected API
return produces distinct reason5/detail0xa551. This is not an argument-guard
panic or a recovered application fault. Do not enable it in an operational image.

Selected source d95b9d179112f422dd00396726776d8b85cbd985 / ELF6d2ccfb6 has
hardware2/2: progressing task/context/synchronization prefix, actual C assert,
27 unchanged halt snapshots and five workload GPIO edges. Each halt is followed
by a separate normal boot; no in-place fault or watchdog recovery claim.
The high-water values are the last pre-assert monitor sample, not deepest
assertion-stack usage. Actual PSP/MSP are captured separately. Root evidence
3796cda13fc926b016d4bea65f93ee032c1b4dcb contains the fixed source/build/hash,
corrected validator/refusal tests and eight-member ordered verification.
R1 200us/30minute coverage and integrated R2/R3 remain separate requirements.

## Finite normal I2C pair in the mixed example (BUILD candidate)

`RP1_RTOS_FEATURE=freertos-r2-mixed-i2c-pair bash tools/build-freertos-r1.sh /new/output`
uses the same eight task stacks, IRQ8/19/25, clock setup and MD01 monitor. It
replaces repeated I2C NACKs with two actual-read requests to0x2d, lengths2/31.
The external ESP must run the opt-in IRP2/IRT1 finite peer and admit a fresh
epoch. Before each normal host UART token, its preload/refill-ready timestamp,
request count and failure state must be verified. Readiness is not byte proof.

After validating that UART token, UART queues sequence1/2 and blocks for the
I2C owner. The owner blocks on this queue, then on its ordinary IRQ driver,
checks every byte and buffer tail/canary, performs checked cleanup, and returns
the same sequence. Only then does UART send its normal ACK. Queue slots2/3
(capacity1) are already in the static pool; driver notification0 stays separate.
No new allocation, task stack, GPIO9/32 handshake, reset or clock writer.

ICMP telemetry120..151 stores complete first2/second31 bytes at141/142..149,
actual IRQ IPSR at150 and address/count0x2d000002 at151. SPI96..119,
UART152..183, MD01 words60..63 and reserved fault184..255 remain separate.
`tools/check-freertos-i2c-mixed-pair.py` requires a full31-snapshot observer and
leaves external ESP/UART/GPIO proof mandatory. Host checks are
`tools/test-i2c-mixed-pair.py` (actual arithmetic/queue handshake) and
`tools/test-freertos-i2c-mixed-pair.py` (synthetic numeric refusals).

This finite ordering is not sustained normal I2C, simultaneous three-bus wire
activity or general NACK recovery. Hardware validation of this image is OPEN.

## Bounded normal I2C stream in the mixed example (unverified candidate)

`RP1_RTOS_FEATURE=freertos-r2-mixed-i2c-stream bash tools/build-freertos-r1.sh /new/output`
extends the same queue/IRQ/checked-completion path to384 requests. The distinct
ICMS schema requires ESP IRP3/IRT2; IRP2 payloads are not interchangeable.
Lengths alternate2/31; zero-based frame `f` has bytes0/1 `(f&255)^0x31`,
`(f>>8)^0x4e`, and subsequent byte `i` `(0xb4+i*0x1d+f*7)&255`.
Every byte/tail is checked before DONE and UART ACK. Normal task waits and50tick
I2C deadline remain; no busy-loop, new task/stack, allocation or reset is added.

The existing 10second-per-pair SPI/UART release schedule remains. I2C waits
for each validated UART grant and finishes before that grant's ACK, so this
does not claim three simultaneous wire transfers. `loaded_sequence` and
publication timestamps must be externally checked before every grant.
Telemetry139 sums all6336 received bytes;141 keeps the initial two bytes,
142..149 holds the most recent subsequent buffer including tail,150 isIPSR24,
151 is0x2d000180. All other mixed memory/IRQ ownership is unchanged.

`tools/test-i2c-mixed-pair.py` compiles both actual payload/queue modes. The
old finite-pair numeric validator intentionally rejects ICMS. A dedicated
36-sample sustained validator and external causal join are still required
before hardware admission; this candidate is not HW-proven or a release.
