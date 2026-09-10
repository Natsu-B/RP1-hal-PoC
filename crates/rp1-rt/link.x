INCLUDE rp1-memory.x

ENTRY(Reset);

SECTIONS
{
  .vector_table ORIGIN(RP1_APP_SRAM) : ALIGN(4)
  {
    KEEP(*(.vector_table .vector_table.*));
  } > RP1_APP_SRAM

  .text : ALIGN(4)
  {
    *(.text .text.*);
    *(.rodata .rodata.*);
  } > RP1_APP_SRAM

  .data : ALIGN(8)
  {
    *(.data .data.*);
  } > RP1_APP_SRAM

  .bss (NOLOAD) : ALIGN(8)
  {
    __sbss = .;
    *(.bss .bss.*);
    *(COMMON);
    __ebss = .;
  } > RP1_APP_SRAM

  .inbound_dummy_page (NOLOAD) : ALIGN(4096)
  {
    __inbound_dummy_page_start = .;
    KEEP(*(.inbound_dummy_page));
    __inbound_dummy_page_end = .;
  } > RP1_APP_SRAM

  __image_end = .;
  ASSERT(__image_end <= __app_limit, "RP1 image overlaps reserved SRAM/ISR stack")
  ASSERT((_stack_start & 7) == 0, "RP1 MSP must be 8-byte aligned")
  ASSERT((ADDR(.vector_table) & 511) == 0, "RP1 VTOR must be 512-byte aligned")

}
