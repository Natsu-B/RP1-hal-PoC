/* cc -std=c11 -Os -Wall -Wextra -Werror -I../c critical_timing.c -o /tmp/test */
#include <assert.h>
#include <stdlib.h>
#include <stdio.h>
#define RP1_CRITICAL_TIMING_HOST_TEST
#include "../c/critical_timing.c"

static uint32_t now, valid = 1, nesting, enters, exits;
uint32_t critical_clock(void) { assert(nesting); return now; }
uint32_t critical_context(void) { return valid && nesting == 0; }
void __real_vPortEnterCritical(void) { ++nesting; ++enters; }
void __real_vPortExitCritical(void) { assert(nesting); --nesting; ++exits; }
void rp1_freertos_assert(uint32_t line) { fprintf(stderr, "assert %u\n", line); abort(); }

int main(void)
{
    struct critical_sample out;
    assert(rp1_freertos_critical_timing_snapshot(0) == -1);
    assert(rp1_freertos_critical_timing_snapshot(&out) == -2);
    __wrap_vPortEnterCritical(); __wrap_vPortExitCritical();
    assert(!sample.count && !depth); // Boot critical sections deliberately excluded.
    valid = 0;
    assert(rp1_freertos_critical_timing_start() == -5);
    valid = 1;
    assert(rp1_freertos_critical_timing_start() == 0);
    assert(rp1_freertos_critical_timing_start() == -2);
    now = UINT32_MAX - 99;
    __wrap_vPortEnterCritical();
    __wrap_vPortEnterCritical();
    now = 100; // Unsigned wrap: 200us, two entries, exactly one sample.
    assert(rp1_freertos_critical_timing_snapshot(&out) == -5);
    __wrap_vPortExitCritical(); assert(sample.count == 0);
    __wrap_vPortExitCritical();
    assert(rp1_freertos_critical_timing_snapshot(&out) == 0);
    assert(out.count == 1 && out.min_us == 200 && out.max_us == 200);
    assert(out.last_us == 200 && out.max_nesting == 2 && !out.saturated);
    assert(sample.count == 2 && sample.min_us == 0); // Snapshot body counted next.
    __wrap_vPortEnterCritical(); now += 350; __wrap_vPortExitCritical();
    assert(sample.max_us == 350 && sample.last_us == 350 && sample.count == 3);
    sample.count = UINT32_MAX;
    __wrap_vPortEnterCritical(); now += 1; __wrap_vPortExitCritical();
    assert(sample.count == UINT32_MAX && sample.saturated == 1);
    valid = 0;
    assert(rp1_freertos_critical_timing_snapshot(&out) == -5);
    assert(enters == exits && !depth && !nesting);
    puts("PASS actual critical timing bodies: nested/wrap/boot/context/snapshot/saturation");
}
