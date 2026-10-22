# Selected watchdog-to-UART restart candidate

Status: hardware OPEN until a matching formal cohort. Prior AU size/fatLTO
commissioning failed cold host admission before WDT9; it was not a UART proof.
Base: AU `762d99d6ae9c70a00825063452af0014bb0e303d`. Official FreeRTOS
ARM_CM3 remains pinned by the existing vendor/build contract. Proc0 only.

Build with `RP1_RTOS_FEATURE=freertos-r3-watchdog-warm-uart` and
`bash tools/build-freertos-r1.sh /absolute/new/output`. This feature fixes Rust
O3 and fat LTO; O3 without LTO exceeded the unchanged SRAM limit, while this
combination fits. The selected source/MMIO behavior is unchanged; O3 cold-start
instruction timing is compared with the earlier working watchdog image.
The exact ELF checker rejects a changed compiler image until reviewed again.

Cold boot runs seven R1 tasks and the existing watchdog task. A fresh warm
kernel instead creates one UART owner in slot7 (512 words, 2304/2560 pool total).
Warm preparation snapshots known registers before writing and admits only
PLL_SYS/UART0 reset-asserted, DONE-clear state. It reuses known exact cold
PLL/UART helpers, never re-enters main/PCIe or replays retained configuration.
Unexpected state is E40..4f (except E45); preparation errors are E50..54.
The PLL lock timeout depends on the raw timer continuing to advance.

UART0 IRQ25/IPSR41, priority6, uses the existing FromISR notification adapter.
The actual USB-UART peer answers two READY markers with the existing 19-byte
sequence payloads. Task-side validation checks payload, generations, IRQ/wake
counts, canaries, bounded checked cleanup and withdrawn ownership. No mock
payload, new ESP firmware, Linux modification or polling substitute is used.

Warm telemetry: 108..123 preparation; 124..159 UART receipts; 160 slot7 free
stack words. The monitor asserts >=32 free words before every success check.
Fault184..255 and KRN8 96..104 remain separate. Exact warm telemetry values
are local, not claimed externally sampled by the no-post-ACK-read observer.

D packet requires both UART exchanges AND the existing five fresh kernel
monitor checks; C from AT is rejected, E remains diagnostic, not success.
The runner must independently validate real peer capture/receipt and GPIO D.
This is selected warm UART recovery, not warm SPI/I2C, repeated watchdog
recovery, autonomous health feeding, PCIe recovery, Linux coexistence or R3.
