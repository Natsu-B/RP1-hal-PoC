/* RP1 single-owner OpenAMP platform. Upstream owns all vring/RPMsg operations.
 * One task calls these functions; the IRQ only acknowledges mailbox sources. */
#include <openamp/rpmsg_virtio.h>
#include <metal/sys.h>
#include <string.h>
#include <limits.h>

#define STATE __attribute__((section(".openamp_state")))
#define CONTROL ((volatile uint32_t *)0x2000da40u)
#define TELE ((volatile uint32_t *)0x2000da80u)
#define TABLE ((volatile uint32_t *)0x8200c000u)
#define POOL ((void *)0x82008000u)
extern void rp1_openamp_complete(void);
extern void rp1_openamp_notify(unsigned int queue);
extern void rp1_openamp_delay(unsigned int usec);
struct metal_state _metal STATE;
struct pending { void *data; uint32_t src; uint32_t len; };
static struct {
    struct rpmsg_virtio_device rpmsg;
    struct virtio_device virtio;
    struct virtqueue queue[2];
    struct virtio_vring_info ring[2];
    struct metal_io_region io;
    struct rpmsg_endpoint endpoint;
    metal_phys_addr_t phys;
    struct pending pending[16];
    uint32_t online, last_sequence;
    uint32_t seal[22];
} state STATE;
_Static_assert(sizeof(state)+sizeof(_metal)<=0x480, "OpenAMP SRAM reservation");

static void barrier(void) { __asm__ volatile("dsb sy" ::: "memory"); }
static __attribute__((noreturn)) void fail(unsigned int code) {
    CONTROL[5]=0x80000000u|code; barrier();
    for (;;) rp1_openamp_delay(1000);
}
void __assert_func(const char *file, int line, const char *fn, const char *expr) {
    (void)file; (void)fn; (void)expr; fail(0x10000u|(unsigned)line);
}
static int ddr_range(const void *p, unsigned int len) {
    uintptr_t a=(uintptr_t)p;
    return a>=0x82000000u && a<=0x8200c000u && len<=0x8200c000u-a;
}
/* Admission checks MPU CTRL == 0 and the observed Cortex-M3 memory attributes.
 * No cache maintenance controller is present on this path. These hooks retain
 * ordering and drain PCIe posted writes; they are deliberately not empty. */
void metal_machine_cache_flush(void *addr, unsigned int len) {
    if (!ddr_range(addr,len)) fail(1);
    rp1_openamp_complete();
}
void metal_machine_cache_invalidate(void *addr, unsigned int len) {
    if (!ddr_range(addr,len)) fail(2);
    barrier();
}
void metal_sys_io_mem_map(struct metal_io_region *io) {
    if (io->virt!=POOL || io->size!=0x4000) fail(3);
}
static uint8_t status(struct virtio_device *v) { (void)v; barrier(); return TABLE[11]&255; }
static uint32_t features(struct virtio_device *v) { (void)v; return TABLE[9]; }
static void notify(struct virtqueue *vq) { rp1_openamp_notify(vq->vq_queue_index); }
static const struct virtio_dispatch dispatch = {.get_status=status,.get_features=features,.notify=notify};
void rp1_openamp_cold_reset(void) { memset(&state,0,sizeof(state)); }
void rp1_openamp_quiesce(void) {
    uint32_t sequence=state.last_sequence;
    memset(&state,0,sizeof(state)); state.last_sequence=sequence;
}
static uint32_t checksum(const unsigned char *p, size_t n) {
    uint32_t sum=2166136261u;
    while (n--) sum=(sum^*p++)*16777619u;
    return sum;
}
static uint32_t word(const void *p) { uint32_t v; memcpy(&v,p,4); return v; }
static int received(struct rpmsg_endpoint *ept, void *data, size_t len, uint32_t src, void *priv) {
    (void)priv;
    unsigned char *p=data;
    TELE[0]++;
    if (len<28 || len>496 || word(p)!=0x31504d52 || word(p+4)!=1 || word(p+20)!=len) { TELE[2]++; return 0; }
    if (word(p+8)!=CONTROL[12] || word(p+12)!=CONTROL[13]) { TELE[3]++; return 0; }
    if (word(p+len-4)!=checksum(p,len-4)) { TELE[4]++; return 0; }
    uint32_t sequence=word(p+16);
    if (sequence<=state.last_sequence) { TELE[5]++; return 0; }
    if (state.last_sequence && sequence!=state.last_sequence+1) { TELE[6]++; return 0; }
    state.last_sequence=sequence;
    int rc=rpmsg_trysendto(ept,data,len,src);
    if (rc==(int)len) { TELE[1]++; return 0; }
    if (rc!=RPMSG_ERR_NO_BUFF) { fail(4); }
    TELE[7]++;
    for (unsigned i=0;i<16;i++) if (!state.pending[i].data) {
        rpmsg_hold_rx_buffer(ept,data);
        state.pending[i]=(struct pending){data,src,len};
        return 0;
    }
    fail(5); return 0;
}

/* Validate the currently available descriptors before upstream dereferences
 * them. Only the kernel-owned 16 KiB pool, direct descriptors and 16 slots. */
static int validate_queue(unsigned q, uint16_t consumed) {
    volatile struct vring_desc *d=(void *)(0x82000000u+q*0x4000u);
    volatile uint16_t *a=(void *)((uintptr_t)d+16*sizeof(*d));
    barrier(); uint16_t end=a[1];
    if ((uint16_t)(end-consumed)>16) return 0;
    for (uint16_t n=consumed;n!=end;n++) {
        uint16_t head=a[2+(n&15)];
        if (head>=16) return 0;
        uint64_t addr=d[head].addr;
        uint32_t len=d[head].len;
        if (addr<state.phys || addr>=state.phys+0x4000 || len<16 || len>512 ||
            len>state.phys+0x4000-addr || (d[head].flags&~VRING_DESC_F_WRITE)) return 0;
        if (q==1) {
            volatile uint16_t *hdr=(void *)(0x82008000u+(uintptr_t)(addr-state.phys));
            if (hdr[6]>len-16) return 0;
        }
    }
    return 1;
}
int rp1_openamp_start(uint32_t buffer_dma) {
    /* DMA convention is supplied by the validated actual Linux descriptors,
     * with exactly these two translations allowed for this fixed reservation. */
    if (buffer_dma!=0x21108000 && buffer_dma!=0x82008000) return -1;
    if (state.online) return -2;
    if (status(NULL)!=7 || features(NULL)!=1 || TABLE[12]!=0x82000000 || TABLE[17]!=0x82004000 ||
        TABLE[13]!=16 || TABLE[14]!=16 || TABLE[18]!=16 || TABLE[19]!=16) return -3;
    rp1_openamp_quiesce(); memset(&_metal,0,sizeof(_metal));
    if (CONTROL[15]==0) for (unsigned i=0;i<10;i++) TELE[i]=0;
    state.phys=buffer_dma;
    if (!validate_queue(0,0) || !validate_queue(1,0)) return -4;
    metal_io_init(&state.io,POOL,&state.phys,0x4000,UINT_MAX,0,NULL);
    state.virtio.role=VIRTIO_DEV_DEVICE;
    state.virtio.id.device=VIRTIO_ID_RPMSG;
    state.virtio.features=1;
    state.virtio.func=&dispatch;
    state.virtio.vrings_num=2;
    state.virtio.vrings_info=state.ring;
    for (unsigned i=0;i<22;i++) state.seal[i]=TABLE[i];
    for (unsigned i=0;i<2;i++) {
        state.ring[i].vq=&state.queue[i];
        state.ring[i].info=(struct vring_alloc_info){.vaddr=(void *)(0x82000000u+i*0x4000u),.align=16,.num_descs=16};
        state.ring[i].notifyid=TABLE[i?20:15];
    }
    int rc=rpmsg_init_vdev(&state.rpmsg,&state.virtio,NULL,&state.io,NULL);
    if (rc) return rc;
    rc=rpmsg_create_ept(&state.endpoint,&state.rpmsg.rdev,"rp1-control",1024,RPMSG_ADDR_ANY,received,NULL);
    if (rc) return rc;
    state.online=1;
    CONTROL[15]++;
    return 0;
}
int rp1_openamp_poll(void) {
    if (!state.online) return 0;
    if (!(status(NULL)&VIRTIO_CONFIG_STATUS_DRIVER_OK)) {
        /* Do not send NS destroy or touch old descriptors after reset. */
        rp1_openamp_quiesce(); TELE[9]++; return 1;
    }
    for (unsigned i=0;i<22;i++) if (TABLE[i]!=state.seal[i]) return -1;
    for (unsigned i=0;i<2;i++) if (!validate_queue(i,state.queue[i].vq_available_idx)) return -2;
    for (unsigned i=0;i<16;i++) if (state.pending[i].data) {
        struct pending *p=&state.pending[i];
        int rc=rpmsg_trysendto(&state.endpoint,p->data,p->len,p->src);
        if (rc==RPMSG_ERR_NO_BUFF) break;
        if (rc!=(int)p->len) return -3;
        rpmsg_release_rx_buffer(&state.endpoint,p->data); p->data=NULL; TELE[1]++;
    }
    virtqueue_notification(&state.queue[0]);
    virtqueue_notification(&state.queue[1]);
    TELE[8]++;
    return 0;
}
