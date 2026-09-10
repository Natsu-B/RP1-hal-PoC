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
