/* Single-task RP1 port. No ISR or proc1 may call OpenAMP.
 * Fail on reentrancy instead of spinning on unsupported shared exclusives. */
#ifndef __METAL_GENERIC_MUTEX__H__
#define __METAL_GENERIC_MUTEX__H__
#include <metal/assert.h>
typedef struct { unsigned int v; } metal_mutex_t;
#define METAL_MUTEX_INIT(m) {0}
#define METAL_MUTEX_DEFINE(m) metal_mutex_t m = METAL_MUTEX_INIT(m)
static inline void __metal_mutex_init(metal_mutex_t *m) { m->v=0; }
static inline void __metal_mutex_deinit(metal_mutex_t *m) { metal_assert(!m->v); }
static inline int __metal_mutex_try_acquire(metal_mutex_t *m) {
    if (m->v) return 0;
    m->v=1; __asm__ volatile("" ::: "memory"); return 1;
}
static inline void __metal_mutex_acquire(metal_mutex_t *m) { metal_assert(__metal_mutex_try_acquire(m)); }
static inline void __metal_mutex_release(metal_mutex_t *m) {
    metal_assert(m->v); __asm__ volatile("" ::: "memory"); m->v=0;
}
static inline int __metal_mutex_is_acquired(metal_mutex_t *m) { return m->v != 0; }
#endif
