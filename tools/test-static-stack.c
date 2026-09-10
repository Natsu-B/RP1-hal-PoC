/* cc -std=c11 -Wall -Wextra -Werror tools/test-static-stack.c -o /tmp/rp1-stack-test */
#include "../crates/rp1-freertos/c/static_stack.h"
#include <assert.h>
int main(void)
{
    uint32_t used = 0;
    const uint32_t normal[] = {512,128,128,256,256,256,256,512};
    for (unsigned i=0; i<sizeof(normal)/sizeof(normal[0]); ++i) {
        used = rp1_stack_next(used,normal[i],2560);
        assert(used != UINT32_MAX && !(used & 1U));
    }
    assert(used == 2304);
    assert(rp1_stack_next(used,256,2560) == 2560);
    assert(rp1_stack_next(used,257,2560) == UINT32_MAX);
    assert(rp1_stack_next(0,129,2560) == 130);
    assert(rp1_stack_next(1,128,2560) == UINT32_MAX);
    assert(rp1_stack_next(0,0,2560) == UINT32_MAX);
    assert(rp1_stack_next(UINT32_MAX,1,2560) == UINT32_MAX);
    assert(rp1_stack_next(0,UINT32_MAX,UINT32_MAX) == UINT32_MAX);
    for (uint32_t start=0; start<=2562; ++start)
        for (uint32_t n=0; n<=2570; ++n) {
            uint32_t end = rp1_stack_next(start,n,2560);
            if (end != UINT32_MAX) assert(end >= start+n && end <=2560 && !(end&1U));
        }
}
