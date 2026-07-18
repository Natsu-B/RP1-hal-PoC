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

  .data : ALIGN(4)
  {
    *(.data .data.*);
  } > RP1_APP_SRAM

  .bss (NOLOAD) : ALIGN(4)
  {
    __sbss = .;
    *(.bss .bss.*);
    *(COMMON);
    __ebss = .;
  } > RP1_APP_SRAM

  __image_end = .;

}
