# Proc0 static FreeRTOS bridge

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

## Mixed SPI / I2C NACK / UART candidate (BUILD, HW OPEN)

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
