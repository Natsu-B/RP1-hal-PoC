/* Opt-in R1 + peripheral-free proc1 worker. No proc-local reservation. */
.proc1_text : ALIGN(4) {
  __proc1_text_start = .;
  KEEP(*(.proc1.text .proc1.text.*));
  KEEP(*(.proc1.init));
  __proc1_text_end = .;
} > RP1_APP_SRAM
.proc1_vectors : ALIGN(512) {
  __proc1_vectors_start = .;
  KEEP(*(.proc1.vectors));
  __proc1_vectors_end = .;
} > RP1_APP_SRAM
.proc1_data : ALIGN(4) {
  __proc1_data_start = .;
  KEEP(*(.proc1.data));
  __proc1_data_end = .;
} > RP1_APP_SRAM
.proc1_bss (NOLOAD) : ALIGN(4) {
  __proc1_bss_start = .;
  KEEP(*(.proc1.bss));
  __proc1_bss_end = .;
} > RP1_APP_SRAM
.proc1_lifecycle (NOLOAD) : ALIGN(4) {
  __proc1_lifecycle_start = .;
  KEEP(*(.proc1.lifecycle));
  __proc1_lifecycle_end = .;
} > RP1_APP_SRAM
.proc1_request (NOLOAD) : ALIGN(4) {
  __proc1_request_start = .;
  KEEP(*(.proc1.request));
  __proc1_request_end = .;
} > RP1_APP_SRAM
.proc1_response (NOLOAD) : ALIGN(4) {
  __proc1_response_start = .;
  KEEP(*(.proc1.response));
  __proc1_response_end = .;
} > RP1_APP_SRAM
.proc1_fault (NOLOAD) : ALIGN(4) {
  __proc1_fault_start = .;
  KEEP(*(.proc1.fault));
  __proc1_fault_end = .;
} > RP1_APP_SRAM
.proc1_stack (NOLOAD) : ALIGN(8) {
  __proc1_guard_low = .; . += 8;
  __proc1_stack_low = .; . += 2048;
  __proc1_stack_top = .;
  __proc1_guard_high = .; . += 8;
  __proc1_stack_end = .;
} > RP1_APP_SRAM

/* Reserve the upper 4 KiB for proc0: no image/dummy page may enter it. */
__proc0_stack_floor = 0x2000e000;
ASSERT(__image_end <= __proc0_stack_floor, "proc0 stack floor overlaps image")
ASSERT(_stack_start == 0x2000f000, "RTOS MSP contract changed")
ASSERT(__proc0_stack_floor < _stack_start, "empty proc0 stack")
ASSERT(_stack_start <= __rp1_debug_diag_start, "proc0 stack overlaps diagnostic")
ASSERT(__proc1_stack_end <= 0x2000e000, "proc1 overlaps RTOS MSP/telemetry")
ASSERT(__proc1_vectors_start % 512 == 0, "proc1 VTOR alignment")
ASSERT(__proc1_vectors_end - __proc1_vectors_start == 320, "proc1 vector count")
ASSERT(__proc1_stack_low % 8 == 0 && __proc1_stack_top % 8 == 0, "proc1 SP alignment")
ASSERT(__proc1_stack_top - __proc1_stack_low == 2048, "proc1 stack budget")
ASSERT(__proc1_text_end <= __proc1_vectors_start, "proc1 code/vector overlap")
ASSERT(__proc1_vectors_end <= __proc1_data_start, "proc1 vector/data overlap")
ASSERT(__proc1_data_end <= __proc1_bss_start, "proc1 data/BSS overlap")
ASSERT(__proc1_bss_end <= __proc1_lifecycle_start, "proc1 BSS/lifecycle overlap")
ASSERT(__proc1_lifecycle_end <= __proc1_request_start, "proc1 lifecycle/request overlap")
ASSERT(__proc1_request_end <= __proc1_response_start, "proc1 request/response overlap")
ASSERT(__proc1_response_end <= __proc1_fault_start, "proc1 response/fault overlap")
ASSERT(__proc1_fault_end <= __proc1_guard_low, "proc1 fault/stack overlap")
ASSERT(__proc1_stack_end <= ADDR(.text), "proc1 stack/proc0 text overlap")
ASSERT(__proc1_stack_end <= __sbss, "proc1 runtime in global BSS clear")
ASSERT(__proc1_stack_end <= __inbound_dummy_page_start, "proc1 runtime/dummy overlap")

