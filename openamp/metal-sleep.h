#ifndef __METAL_GENERIC_SLEEP__H__
#define __METAL_GENERIC_SLEEP__H__
extern void rp1_openamp_delay(unsigned int usec);
static inline int __metal_sleep_usec(unsigned int usec) { rp1_openamp_delay(usec); return 0; }
#endif
