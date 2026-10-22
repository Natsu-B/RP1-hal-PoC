#pragma once
#include <stdint.h>

/* Fixed boot-time pool, no free/reuse and no allocation after scheduler start. */
static inline uint32_t rp1_stack_next(uint32_t used, uint32_t words, uint32_t capacity)
{
    if ((used & 1U) || used > capacity || words == 0 || words > capacity)
        return UINT32_MAX;
    uint32_t rounded = (words + 1U) & ~1U;
    if (rounded < words || rounded > capacity - used) return UINT32_MAX;
    return used + rounded;
}

/* Opt-in proc0 layout: slot0 retains 512 words locally; other slots retain
 * 1792 shared words. The total budget remains 2304, with no runtime reuse. */
static inline uint32_t rp1_stack_next_local_monitor(uint32_t used, uint32_t words, uint32_t slot)
{
    if (slot >= 8 || used > 1792 || (used & 1U)) return UINT32_MAX;
    if (slot == 0) return words == 512 ? used : UINT32_MAX;
    return rp1_stack_next(used, words, 1792);
}
