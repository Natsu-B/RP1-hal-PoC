/* Host fixture, not the runtime linker script. No ownership is granted here. */
MEMORY {
  SHARED (rwx) : ORIGIN = 0x20000000, LENGTH = 56K
  LOCAL (rw)   : ORIGIN = 0x10003800, LENGTH = 2K
}
PHDRS {
  shared PT_LOAD FLAGS(7);
  local PT_NULL FLAGS(6);
}
ENTRY(Reset)
SECTIONS {
  .vector_table ORIGIN(SHARED) : { KEEP(*(.vector_table)) } > SHARED :shared
  .text : ALIGN(4) { *(.text .text.*) } > SHARED :shared
  /* The pinned parser validates alignment even on PT_NULL. Pad the final
     file-backed section so the following zero-file-size header is congruent. */
  .data : ALIGN(8) { *(.data .data.*) . = ALIGN(8); } > SHARED :shared
  .bss (NOLOAD) : ALIGN(8) {
    __bss_start = .;
    *(.bss .bss.*)
    __bss_end = .;
  } > SHARED :shared
  /* No loader copy/zero to proc-local memory. A future runtime owner must
     initialize this entire stack before use. No lifetime/LMA overlap. */
  .local_stack ORIGIN(LOCAL) (NOLOAD) : {
    KEEP(*(.local_stack))
  } > LOCAL :local
  ASSERT(SIZEOF(.vector_table) == 320, "fixture vectors")
  ASSERT(SIZEOF(.local_stack) == 2048, "fixture stack budget")
  ASSERT(__bss_end <= ORIGIN(SHARED) + LENGTH(SHARED), "reserved shared SRAM")
  ASSERT((ADDR(.local_stack) & 7) == 0, "stack alignment")
}
