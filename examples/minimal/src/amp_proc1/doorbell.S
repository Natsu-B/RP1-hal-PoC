.syntax unified
.thumb
.section .amp_proc1.doorbell_text,"ax",%progbits
.balign 4
.global AmpProc1DoorbellStart
.type AmpProc1DoorbellStart,%function
.thumb_func
AmpProc1DoorbellStart:
    cpsid i
    ldr r4,=0x2000f600
    movs r0,#0
    movs r1,#16
    mov r2,r4
1:  str r0,[r2],#4
    subs r1,#1
    bne 1b
    movs r0,#16
    str r0,[r4,#12]
    movs r5,#1
    str r5,[r4,#16]
    movs r6,#0
    ldr r7,=__amp_proc1_vectors
    str r7,[r4,#40]
    // Copy the complete position-independent handler, including its literal pool.
    ldr r0,=AmpProc1DoorbellIrq
    bic r0,r0,#1
    ldr r1,=0x10000000
    ldr r2,=AmpProc1DoorbellIrqEnd
2:  ldr r3,[r0],#4
    str r3,[r1],#4
    cmp r0,r2
    blo 2b
    ldr r0,=0x10000001
    str r0,[r4,#44]
    sub r1,r1,#0x10000000
    str r1,[r4,#48]
    // 80 vectors = 320 bytes, requiring 512-byte VTOR alignment.
    mov r0,r7
    ldr r1,=0x10002800
    movs r2,#80
3:  ldr r3,[r0],#4
    str r3,[r1],#4
    subs r2,#1
    bne 3b
    ldr r1,=0x10002840
    ldr r0,=0x10000001
    str r0,[r1]
    mrs r0,MSP
    str r0,[r4,#52]
    dsb sy
    isb sy
    // gpio_irq::stop() already removed all GPIO9 event enables. Refuse drift.
    ldr r0,=0x400d004c
    ldr r0,[r0]
    tst r0,#0x0ff00000
    bne .Ldb_gpio_fault
    ldr r0,=0x400d2110
    mov.w r1,#512
    str r1,[r0]
    ldr r0,=0x400d3114
    str r1,[r0]
    ldr r0,=0xe000e280
    str r5,[r0]
    ldr r0,=0xe000e100
    str r5,[r0]
    str r5,[r4,#4]
    str r5,[r4,#8]
    dsb sy
    ldr r0,=0x314c4244 // DBL1, published last.
    str r0,[r4]
    dsb sy
.Ldb_poll:
    cmp r5,#1
    bne .Ldb_busy
    dsb sy
    wfi
    ldr r0,=0x400d0118
    ldr r0,[r0]
    tst.w r0,#512
    bne .Ldb_handle
    ldr r0,[r4,#36]
    adds r0,#1
    str r0,[r4,#36]
    b .Ldb_poll
.Ldb_busy:
    ldr r0,=0x400d0118
    ldr r0,[r0]
    tst.w r0,#512
    beq .Ldb_busy
.Ldb_handle:
    bl AmpProc1DoorbellIrq
.Ldb_dispatch:
    cpsid i
    // Restore the normal fault vectors before publishing a polling/stop mode.
    ldr r0,=0xe000ed08
    str r7,[r0]
    str r7,[r4,#40]
    dsb sy
    isb sy
    ldr r0,=0xe000e280
    movs r1,#1
    str r1,[r0]
    ldr r6,[r4,#20]
    ldr r0,[r4,#32]
    cbnz r0,.Ldb_stop
    ldr r5,[r4,#16]
    movw r0,#65535
    cmp r5,r0
    beq .Ldb_stop
    cmp r5,#1
    beq .Ldb_publish_poll
    cmp r5,#3
    beq .Ldb_publish_poll
    cmp r5,#2
    bne .Ldb_unknown
    ldr r0,=0xe000ed08
    ldr r1,=0x10002800
    str r1,[r0]
    str r1,[r4,#40]
    dsb sy
    isb sy
    str r5,[r4,#4]
    dsb sy
.Ldb_isr_wait:
    // Mask/check/WFI/unmask prevents a lost mode-change wakeup. IRQ0 is enabled.
    cpsid i
    ldr r0,[r4,#32]
    cmp r0,#0
    bne .Ldb_dispatch
    ldr r0,[r4,#20]
    cmp r0,r6
    bne .Ldb_dispatch
    dsb sy
    wfi
    cpsie i
    isb sy
    b .Ldb_isr_wait
.Ldb_publish_poll:
    str r5,[r4,#4]
    dsb sy
    b .Ldb_poll
.Ldb_gpio_fault:
    movs r0,#4
    b .Ldb_fault
.Ldb_unknown:
    movs r0,#1
.Ldb_fault:
    str r0,[r4,#32]
.Ldb_stop:
    cpsid i
    ldr r0,=0x400d3110
    mov.w r1,#512
    str r1,[r0]
    ldr r0,=0x400d3114
    str r1,[r0]
    movs r1,#1
    ldr r0,=0xe000e180
    str r1,[r0]
    ldr r0,=0xe000e280
    str r1,[r0]
    ldr r0,=0xe000ed08
    str r7,[r0]
    str r7,[r4,#40]
    dsb sy
    isb sy
    movw r0,#65535
    str r0,[r4,#4]
    movs r0,#2
    str r0,[r4,#8]
4:  dsb sy
    wfe
    b 4b
.size AmpProc1DoorbellStart,.-AmpProc1DoorbellStart
.ltorg
.balign 4
.global AmpProc1DoorbellIrq
.type AmpProc1DoorbellIrq,%function
.thumb_func
AmpProc1DoorbellIrq:
    // First data read is TIMER low; timestamp store precedes source validation.
    ldr r1,=0x400ac028
    ldr r2,[r1]
    ldr r0,=0x2000f600
    str r2,[r0,#24]
    ldr r1,=0x400d0118
    ldr r2,[r1]
    cmp.w r2,#512
    bne 6f
    ldr r1,=0x400d3114
    mov.w r2,#512
    str r2,[r1]
    dsb sy
    ldr r1,[r0,#20]
    adds r1,#1
    str r1,[r0,#20]
    dsb sy
    ldr r2,=10000000
    cmp r1,r2
    bhs 5f
    bx lr
5:  movs r1,#2
    b 7f
6:  str r2,[r0,#60]
    ldr r1,[r0,#28]
    adds r1,#1
    str r1,[r0,#28]
    movs r1,#3
7:  cpsid i
    str r1,[r0,#32]
    ldr r1,=0xe000e180
    movs r2,#1
    str r2,[r1]
    dsb sy
    bx lr
.size AmpProc1DoorbellIrq,.-AmpProc1DoorbellIrq
.ltorg
.balign 4
.global AmpProc1DoorbellIrqEnd
AmpProc1DoorbellIrqEnd:
