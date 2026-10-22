/* Exercise the real client with RPMsg's lack of writable poll notification. */
#define _GNU_SOURCE
#include <poll.h>
static int rpmsg_poll(struct pollfd *fds, nfds_t n, int timeout);
#define poll rpmsg_poll
#define main bulk_client_main
#include "linux-amp-bulk.c"
#undef main
#undef poll
static int rpmsg_poll(struct pollfd *fds, nfds_t n, int timeout) {
    for(nfds_t i=0;i<n;i++) assert(!(fds[i].events&POLLOUT));
    return poll(fds,n,timeout);
}
int main(void) { self_test(); return 0; }
