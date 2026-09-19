/* Link/loader fixture ONLY: not a startup implementation or deployable image. */
.syntax unified
.cpu cortex-m3
.thumb
.section .vector_table,"a",%progbits
.balign 4
.word 0x2000f000
.word Reset
.space 312

.section .text.Reset,"ax",%progbits
.global Reset
.type Reset,%function
.thumb_func
Reset:
1:  b 1b
.size Reset, .-Reset

.section .local_stack,"aw",%nobits
.balign 8
.space 2048

.section .data.fixture,"aw",%progbits
.balign 4
.word 0x13579bdf

.section .bss.fixture,"aw",%nobits
.balign 8
.space 4096
