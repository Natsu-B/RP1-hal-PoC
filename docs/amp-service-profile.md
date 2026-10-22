# AG01 phase0 service profile

`freertos-amp-control` selects two proc0 tasks: the1 ms service task owns
UART0, time synchronization and commissioning DDR RPC; the OpenAMP task owns
RPMsg and the bulk pump. Each gets512 stack words from a1024-word static pool.
The older R1 spin, producer, consumer, queue and mutex tasks are not linked.
Proc1 remains held. This profile does not run the local controller or a5 kHz IRQ.

The ELF contract `.amp_control_contract` contains `AG01,1,2,1024,200,0`.
The200 us value is the future target; phase0 is not a measured periodic loop.
The ELF validator requires explicit `--amp-control`, read-only loaded contract
bytes, the actual symbol/address, the task allocation and existing vector/SCMI/
DDR/MSP checks. Legacy admission rejects this image. No legacy R1 proof changes.

BAR2 telemetry at RP1 SRAM`0x2000f800` distinguishes the profiles:

|Words|AG01 meaning|
|---|---|
|0,1|AG01 magic, version1|
|2..4|health stage, fault reason/detail|
|8,9,14|actual ticks, scheduler switches, service scans|
|18|MSP high-water usage|
|19..22,28..31|startup sentinels, interrupt grouping, reload, service task context|
|32,33|actual two-task free stack words|
|34..38,45,64,80|zero; removed R1 tasks must not supply artificial counters|
|42..44|task count2, pool1024, phase0|

The paired loader feature is `rp1-amp-service`. Its31-sample admission checks
this distinct contract, context, both stack margins, ongoing counters and mean
tick against the unchanged local timer. ELF/DT hashes and the actual SCMI vector
are fixed in source and tested against the selected artifacts before build.
These checks cover service continuity, not end-to-end control deadlines.

The Linux commissioning client requires `--amp-service` for AG01, before any
RPC publication. Its progress check uses actual scheduler word9 because the R1
producer word49 no longer exists. The retained output label`producer` names the
selected progress counter; in AG01 it is not a data-producing task. Default mode
still requires RT01 and its original producer. Payload validation, generations,
timeouts, posted completion and reserved-window limits are unchanged.

Pre-claim scratch diagnostics still close permanently after the unique bulk
claim, including after retirement. A new test session requires a qualified
whole-system boot. No proc1 reset, watchdog, remoteproc stop/remove or automatic
reconnect is added by this profile.
