# Config-only transaction: HOST/STATIC, not admitted

`endpoint-config-foundation` only compiles `endpoint_config.rs`. It is absent
from every default/observer feature closure and has **no firmware caller**.
`WINDOW_CANDIDATE_ONLY` still authorizes no writes. Enabling this feature alone
does not run a repair. The unsafe `ReviewedWindow` constructor is a separate
future admission boundary, not a conversion of gate code 1.

One persistent `Transaction` consumes its first request, including disabled or
rejected requests, and never rearms. A future caller must retain it for the boot
epoch and run before UART emission. It must separately establish the fresh low
timestamp, finite host exclusion margin, proc0 accessor/exception scope and an
external known-good recovery path. Neither the diagnostic 5000/2500us bounds nor
the host's nominal 100ms sleep establish that admission. No margin is selected
by the firmware here; invalid zero/half-range budgets are rejected.

The protected implementation saves actual PRIMASK, masks configurable local
interrupts, rechecks PERSTN/CORE_ALIVE, selector0, PROC1 CTRL0 bit31 set and DONE0
bit31 clear, identity 00011de4, command MSE/BME clear, classrev2, zero BARs, clear
RO bit0 and stable initial MONITOR2 level bits. These are actual reads, not
values borrowed from earlier boot logs. It then runs only this finite recipe:

1. Save DBI+8bc; set bit0 only. Write class dword+008=02000000.
2. Selector1; write +010/+014/+018 masks 3fff/3fffff/ffff.
3. Selector0; write normal BAR0..2=fffffff0; restore saved RO.
4. Execute normal-view readback: identity unchanged, MSE/BME clear,
   class02000000, BARffffc000/ffc00000/ffff0000, selector0 and original RO.

Each config access has readiness/PROC1/selector and deadline checks. There are
no DBI2 reads, shadow-zero rollback, reset/POWER/CONTROL/iATU/INTR/INTE changes,
RTOS calls, waits, allocation, formatting, or panic assertions in execution.
No requirement that a DBI2 read equal the programmed mask is introduced.

On a returned failure after a mutation, cleanup may restore **only** selector0
and saved RO, and only while readiness, PROC1 exclusion and deadline checks
still pass. Expired/unsafe cleanup is explicitly deferred. Cleanup failure is
distinct from cleanup restoration; neither claims to roll back class or BARs.
Any incomplete mutation sequence or failed interrupt-mask restoration requires
external recovery. There are no retries. Receipt `Option` values distinguish
executed tuple readbacks from unexecuted fields; recipe/cleanup attempted and
returned write bitmaps are separate. A returned store is not write acceptance.

These checks cannot close the check-to-access race with external reset/host
actors. PRIMASK does not mask NMI/HardFault or protect against proc1/host cycles.
An actual MMIO load/store can stall or fault without returning; deadlines and
cleanup cannot interrupt it. Host callback errors are artificial returned
failures, not target fault recovery. Silicon acceptance, runtime stack cost,
write-to-host timing and Linux resource admission remain unproved.

Run actual-source host checks without adding dependencies:

```sh
rustc +stable --edition=2024 --test examples/minimal/src/endpoint_config.rs \
  -o /tmp/endpoint-config-test
/tmp/endpoint-config-test
```

Tests cover exact reads/write recipe, both saved interrupt-mask states,
zero-write precondition rejection, every mutation's returned failure/ignored
write/reset-loss/deadline, explicit cleanup failure/defer, partial-read metadata,
wrap-safe expiry and no rearm. ARM codegen and unchanged observer ELF identity
are separate BUILD evidence; none is hardware admission.
