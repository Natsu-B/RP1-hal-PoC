# Proc0 static FreeRTOS bridge

FreeRTOS-Kernel V11.3.1, commit `3a22924e0a9ddbbc8b0758881c33b3422a5cc20d`,
is compiled from the unmodified `third-party/FreeRTOS-Kernel` submodule. The build
rejects another commit, tracked vendor edits, another ARM target, or a compiler
other than `arm-none-eabi-gcc` 14.2.1. `RP1_FREERTOS_CC`/`RP1_FREERTOS_AR` may
select installed tool paths, not silently select newer versions. Compiler,
archiver, Rust compiler, target and vendor provenance go to `OUT_DIR/toolchain.txt`.
No `cc` crate, heap implementation, downloaded build input, or alternate port is used.

## Runtime contract

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

All C pools are ordinary shared `.bss`: 8 task slots, each with one TCB and an
8-byte-aligned 512-word stack; a separate 128-word idle stack; 4 `u32` queue slots
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
