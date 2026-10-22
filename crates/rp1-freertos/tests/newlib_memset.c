/* Instruction-level Cortex-M3 model check; this is not RP1 hardware evidence. */
#include <stddef.h>
#include <stdint.h>

extern void *memset(void *, int, size_t);
extern void __aeabi_memset(void *, size_t, int);
extern void __aeabi_memset4(void *, size_t, int);
extern void __aeabi_memset8(void *, size_t, int);
extern void __aeabi_memclr(void *, size_t);
extern void __aeabi_memclr4(void *, size_t);
extern void __aeabi_memclr8(void *, size_t);
static volatile uint8_t destination[576] __attribute__((aligned(8)));
static volatile unsigned current_kind, current_length, current_value, current_dst;

static void semihost(unsigned operation, const void *argument) {
    register unsigned r0 __asm__("r0") = operation;
    register const void *r1 __asm__("r1") = argument;
    __asm__ volatile("bkpt 0xab" : "+r"(r0), "+r"(r1) : : "memory", "cc");
}

static void hex(unsigned value) {
    char text[10];
    for (unsigned i = 0; i < 8; ++i)
        text[i] = "0123456789abcdef"[(value >> (28 - 4 * i)) & 15];
    text[8] = ' '; text[9] = 0;
    semihost(4, text);
}

__attribute__((noreturn)) static void finish(unsigned status) {
    const unsigned arguments[] = {0x20026, status};
    semihost(0x20, arguments);
    for (;;) {}
}

static void failure_context(void) {
    semihost(4, "kind,length,value,dst(hex)=");
    hex(current_kind); hex(current_length); hex(current_value); hex(current_dst);
    semihost(4, "\n");
}

void fault(void) {
    semihost(4, "TRAP cfsr(hex)=");
    hex(*(volatile unsigned *)0xe000ed28);
    semihost(4, "\n");
    failure_context();
    finish(2);
}

void reset(void) {
    *(volatile unsigned *)0xe000ed14 = (1u << 9) | (TRAP_MODE << 3);
    __asm__ volatile("dsb\nisb" : : : "memory");
    unsigned cases = 0;
    const int values[] = {0, 1, 0xa5, 0x12345678, -1};
    for (unsigned kind = 0; kind < 7; ++kind) {
        unsigned alignment = kind == 2 || kind == 5 ? 4 :
                             kind == 3 || kind == 6 ? 8 : 1;
        for (unsigned index = 0; index < 260; ++index) {
            unsigned length = index < 258 ? index : index + 253;
            for (unsigned v = 0; v < (kind < 4 ? 5u : 1u); ++v) {
                for (unsigned d = 0; d < 4; ++d) {
                    unsigned start_dst = 16 + d * alignment;
                    int value = values[v];
                    current_kind = kind; current_length = length;
                    current_value = value; current_dst = start_dst;
                    for (unsigned i = 0; i < sizeof(destination); ++i)
                        destination[i] = 0xa5;
                    void *to = (void *)&destination[start_dst];
                    unsigned bad = 0;
                    switch (kind) {
                    case 0: bad = memset(to, value, length) != to; break;
                    case 1: __aeabi_memset(to, length, value); break;
                    case 2: __aeabi_memset4(to, length, value); break;
                    case 3: __aeabi_memset8(to, length, value); break;
                    case 4: __aeabi_memclr(to, length); break;
                    case 5: __aeabi_memclr4(to, length); break;
                    case 6: __aeabi_memclr8(to, length); break;
                    }
                    for (unsigned i = 0; i < sizeof(destination); ++i) {
                        unsigned wanted = i >= start_dst && i < start_dst + length
                            ? (unsigned char)value : 0xa5;
                        bad |= destination[i] != wanted;
                    }
                    if (bad) {
                        semihost(4, "FAIL fill/return/canary\n");
                        failure_context();
                        finish(1);
                    }
                    ++cases;
                }
            }
        }
    }
    semihost(4, "PASS cases(hex)="); hex(cases); semihost(4, "\n");
    finish(0);
}

__attribute__((section(".vectors"), used))
const uintptr_t vectors[] = {
    0x20010000, (uintptr_t)reset, (uintptr_t)fault, (uintptr_t)fault,
    (uintptr_t)fault, (uintptr_t)fault, (uintptr_t)fault,
};
