/* Task-side outer critical BODY only: not complete interrupt-mask duration.
 * Link --wrap=vPortEnterCritical and --wrap=vPortExitCritical. Do not wrap
 * the official naked exception handlers. The pinned port stays unmodified. */
#include "FreeRTOSConfig.h"
#include <stdint.h>

struct critical_sample {
    uint32_t count, min_us, max_us, last_us, max_nesting, saturated;
};
_Static_assert(sizeof(struct critical_sample) == 24, "six u32 FFI words");
static struct critical_sample sample;
static uint32_t enabled, depth, entered;
void __real_vPortEnterCritical(void);
void __real_vPortExitCritical(void);

#ifdef RP1_CRITICAL_TIMING_HOST_TEST
uint32_t critical_clock(void);
uint32_t critical_context(void);
#else
static uint32_t critical_clock(void)
{
    return *(volatile const uint32_t *)0x400ac028U;
}
static uint32_t critical_context(void)
{
    uint32_t ipsr, control, basepri, primask;
    __asm volatile("mrs %0, ipsr\nmrs %1, control\nmrs %2, basepri\nmrs %3, primask"
        : "=r"(ipsr), "=r"(control), "=r"(basepri), "=r"(primask) :: "memory");
    return ipsr == 0 && (control & 3U) == 2 && basepri == 0 && primask == 0;
}
#endif

void __wrap_vPortEnterCritical(void)
{
    __real_vPortEnterCritical();
    if (enabled) {
        configASSERT(depth != UINT32_MAX);
        ++depth;
        if (depth > sample.max_nesting) sample.max_nesting = depth;
        if (depth == 1) entered = critical_clock();
    }
}

void __wrap_vPortExitCritical(void)
{
    if (enabled) {
        configASSERT(depth != 0);
        if (--depth == 0) {
            /* Timestamp precedes bookkeeping and the real unmask/tail. */
            uint32_t elapsed = critical_clock() - entered;
            sample.last_us = elapsed;
            if (elapsed < sample.min_us) sample.min_us = elapsed;
            if (elapsed > sample.max_us) sample.max_us = elapsed;
            if (sample.count == UINT32_MAX) sample.saturated = 1;
            else ++sample.count;
        }
    }
    __real_vPortExitCritical();
}

/* First running privileged PSP task only, with interrupts enabled and no
 * enclosing critical section. Never reset statistics after measurement starts. */
int32_t rp1_freertos_critical_timing_start(void)
{
    if (!critical_context()) return -5;
    __real_vPortEnterCritical();
    int32_t result = enabled ? -2 : 0;
    if (!enabled) {
        sample.min_us = UINT32_MAX;
        enabled = 1;
    }
    __real_vPortExitCritical();
    return result;
}

int32_t rp1_freertos_critical_timing_snapshot(struct critical_sample *out)
{
    if (out == 0) return -1;
    if (!critical_context()) return -5;
    if (!enabled) return -2;
    __wrap_vPortEnterCritical();
    *out = sample;
    /* This snapshot's own critical body appears in the NEXT snapshot. */
    __wrap_vPortExitCritical();
    return 0;
}
