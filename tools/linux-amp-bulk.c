/* Opt-in BKB1 firmware only. Synthetic data; NOT a real-time acceptance test.
 * Requires independently sealed firmware/loader/DT and exclusive commissioning.
 * Never maps RAM outside the admitted 64 KiB no-map reservation. */
#define _GNU_SOURCE
#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <glob.h>
#include <inttypes.h>
#include <linux/rpmsg.h>
#include <poll.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/file.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <sys/random.h>
#include <sys/socket.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>
#ifdef AMP_GPIO_UART
int uart_gpio_main(int argc,char **argv);
#endif

enum { SIZE=65536, HALF=32768, WORDS=HALF/4, HELLO=0, RX_READY=1,
       TX_FREE=2, TX_START=3, QUERY_RX=4, QUERY_TX=5, RETIRE=6,
       SET_PARAMETERS=8, STOP_CONTROL=9, QUERY_CONTROL=10,
       START_OBSERVATIONS=11, QUERY_OBSERVATIONS=12, OBS_BYTES=96 };
static const char *pci="/sys/bus/pci/devices/0002:01:00.0";
static uint64_t boot, session, sequence, valid_until, run_until;
static unsigned exchanges;
static int control_tests,gpio_io_tests,gpio_phase;
static int observation_tests,observation_active,observation_start_attempted;
static volatile sig_atomic_t cancelled;
static void stop(int sig) { (void)sig; cancelled=1; }
static uint64_t now(void) {
    struct timespec t; if(clock_gettime(CLOCK_MONOTONIC,&t)) abort();
    return (uint64_t)t.tv_sec*1000000000u+t.tv_nsec;
}
static void fence(void) {
#ifdef __aarch64__
    __asm__ volatile("dsb sy" ::: "memory");
#else
    __sync_synchronize(); /* Host self-test only; hardware main rejects this ISA. */
#endif
}
static uint32_t get(const unsigned char *p) {
    return (uint32_t)p[0]|(uint32_t)p[1]<<8|(uint32_t)p[2]<<16|(uint32_t)p[3]<<24;
}
static void put(unsigned char *p,uint32_t x) { for(unsigned i=0;i<4;i++)p[i]=(unsigned char)(x>>(8*i)); }
static uint64_t pair(const uint32_t *w,unsigned i) { return w[i]|(uint64_t)w[i+1]<<32; }
static uint64_t mmio_pair(volatile uint32_t *w,unsigned i) { uint32_t lo=w[i],hi=w[i+1]; return lo|(uint64_t)hi<<32; }
static void split(uint32_t *w,unsigned i,uint64_t v) { w[i]=(uint32_t)v; w[i+1]=(uint32_t)(v>>32); }
static void command(uint32_t *w,unsigned kind,const uint32_t *d) {
    memset(w,0,128); w[0]=1;w[1]=64;split(w,2,boot);split(w,4,session);
    split(w,6,++sequence);split(w,10,valid_until);w[12]=kind;w[13]=64;
    if(d)memcpy(w+16,d,64);
}
static void descriptor(uint32_t *d,int tx,unsigned seq) {
    memset(d,0,64);d[0]=1;d[1]=tx?1:2;split(d,2,boot);split(d,4,session);
    d[6]=seq;d[8]=tx?2:1;d[9]=tx?HALF:0;d[10]=HALF;d[14]=UINT32_MAX;
}
static int valid_reply(const uint32_t *w,const uint32_t *r,unsigned status) {
    unsigned mode=status?0:w[12]==QUERY_CONTROL?3:
        (w[12]==SET_PARAMETERS || w[12]==STOP_CONTROL)?1:
        w[12]==START_OBSERVATIONS?4:w[12]==QUERY_OBSERVATIONS?5:observation_active?4:0;
    /* START may have executed even if its response was lost. Only RETIRE can
     * close that uncertainty, under either pre-start or started reply mode. */
    int retire_mode=w[12]==RETIRE && !status && observation_start_attempted && (r[15]==0 || r[15]==4);
    return r[0]==0x31524b42 && r[1]==1 && r[2]==status && r[3]==w[12] &&
        pair(r,4)==pair(w,6) && r[14]==4 && (r[15]==mode || retire_mode);
}
static int exchange(int fd,const uint32_t *w,uint32_t *r,unsigned status) {
    unsigned char wire[128],reply[512];for(unsigned i=0;i<32;i++)put(wire+4*i,w[i]);
    uint64_t deadline=now()+2000000000ull;int sent=0;
    if(run_until && run_until<deadline)deadline=run_until;
    while(now()<deadline && !cancelled) {
        /* virtio RPMsg does not advertise POLLOUT. O_NONBLOCK write reports
         * queue pressure directly; only an unsent EAGAIN may be retried. */
        if(!sent) {
            ssize_t n=write(fd,wire,sizeof wire);
            if(n<0 && errno==EINTR)continue;
            if(n<0 && errno==EAGAIN){poll(NULL,0,1);continue;}
            if(n!=128)return -1;
            sent=1;
        }
        struct pollfd p={fd,POLLIN,0};
        int rc=poll(&p,1,20);
        if(rc<0 && errno==EINTR)continue;
        if(rc<0 || (p.revents&(POLLERR|POLLHUP|POLLNVAL)))return -1;
        if(!rc)continue;
        ssize_t n=read(fd,reply,sizeof reply);
        if(n<0 && (errno==EAGAIN || errno==EINTR))continue;
        if(n!=128)return -1;
        for(unsigned i=0;i<32;i++)r[i]=get(reply+4*i);
        exchanges++;
        if(!valid_reply(w,r,status)) {
            fprintf(stderr,"reply rejected: command=%u sequence=%"PRIu64" status=%u expected=%u\n",w[12],pair(w,6),r[2],status);
            return -1;
        }
        if(w[12]==START_OBSERVATIONS && !status)observation_active=1;
        if(w[12]==SET_PARAMETERS || w[12]==STOP_CONTROL) {
            printf("{\"%s\":%u,\"sequence\":%"PRIu64",\"status\":%u,\"mode\":%u,\"apply_time_us\":%"PRIu64",\"valid_until_us\":%"PRIu64",\"boot_generation\":\"%016"PRIx64"\",\"session_generation\":\"%016"PRIx64"\",\"parameters\":[%u,%u,%u,%u]}\n",observation_tests?"observation_control_reply":gpio_phase?"gpio_io_reply":"control_reply",w[12],pair(w,6),r[2],r[15],pair(w,8),pair(w,10),pair(w,2),pair(w,4),w[20],w[21],w[22],w[23]);
        }
        return 0;
    }
    fprintf(stderr,"RPMsg timeout/cancel: command=%u sequence=%"PRIu64"\n",w[12],pair(w,6));
    return -1; /* Never resend an ownership-changing request after uncertainty. */
}
static int call(int fd,unsigned kind,const uint32_t *d,uint32_t *r,unsigned status) {
    uint32_t w[32];command(w,kind,d);return exchange(fd,w,r,status);
}
static int property(const char *base,const char *name,unsigned char *data,size_t cap) {
    char path[512];int n=snprintf(path,sizeof path,"%s/%s",base,name);
    if(n<0 || (size_t)n>=sizeof path)return -1;
    int fd=open(path,O_RDONLY|O_CLOEXEC);if(fd<0)return -1;
    ssize_t len=read(fd,data,cap);unsigned char extra;
    if(len>=0 && read(fd,&extra,1)!=0)len=-1;
    close(fd);return (int)len;
}
static int be_cell(const unsigned char *p,uint32_t x) {
    return p[0]==(x>>24) && p[1]==((x>>16)&255) && p[2]==((x>>8)&255) && p[3]==(x&255);
}
static uint64_t be_number(const unsigned char *b,unsigned bytes) {
    uint64_t value=0;for(unsigned i=0;i<bytes;i++)value=(value<<8)|b[i];return value;
}
static int reservation(const char *base,uint32_t address,uint32_t size) {
    unsigned char b[32];
    if(property(base,"#address-cells",b,sizeof b)!=4 || !be_cell(b,2) ||
       property(base,"#size-cells",b,sizeof b)!=4 || !(be_cell(b,1) || be_cell(b,2)))return -1;
    unsigned size_bytes=be_cell(b,1)?4:8;
    if(property(base,"ranges",b,sizeof b)!=0)return -1;
    glob_t list={0};int found=0;
    char pattern[512];int n=snprintf(pattern,sizeof pattern,"%s/*",base);
    if(n<0 || (size_t)n>=sizeof pattern || glob(pattern,0,NULL,&list))return -1;
    for(size_t i=0;i<list.gl_pathc;i++) {
        const char *p=list.gl_pathv[i];
        errno=0;int bytes=property(p,"reg",b,sizeof b);
        if(bytes<0 && (errno==ENOENT || errno==ENOTDIR))continue;
        if(bytes!=(int)(8+size_bytes)){globfree(&list);return -1;}
        uint64_t start=be_number(b,8),length=be_number(b+8,size_bytes);
        if(length>UINT64_MAX-start){globfree(&list);return -1;}
        if(!length || start>=(uint64_t)address+size || start+length<=address)continue;
        if(start!=address || length!=size || property(p,"no-map",b,sizeof b)!=0) {globfree(&list);return -1;}
        errno=0;if(property(p,"reusable",b,sizeof b)>=0 || errno!=ENOENT){globfree(&list);return -1;}
        errno=0;int n=property(p,"status",b,sizeof b);
        if(!((n<0 && errno==ENOENT) || (n==5 && !memcmp(b,"okay\0",5)) ||
             (n==3 && !memcmp(b,"ok\0",3)))){globfree(&list);return -1;}
        found++;
    }
    globfree(&list);return found==1?0:-1;
}
static int reservations(const char *base) {
    if(reservation(base,0x21000000,SIZE))return -1;
    for(unsigned i=0;i<4;i++)if(reservation(base,0x21100000+i*0x4000,0x4000))return -1;
    return 0;
}
static int admission(void) {
    if(sysconf(_SC_PAGESIZE)!=16384 || reservations("/sys/firmware/devicetree/base/reserved-memory"))return -1;
    FILE *f=fopen("/proc/iomem","r");if(!f)return -1;
    char line[256];unsigned found=0;
    while(fgets(line,sizeof line,f))if(!strcmp(line,"21000000-2100ffff : reserved\n"))found++;
    fclose(f);if(found!=1)return -1;
    unsigned char b[32];
    if(property(pci,"vendor",b,sizeof b)!=7 || memcmp(b,"0x1de4\n",7) ||
       property(pci,"device",b,sizeof b)!=7 || memcmp(b,"0x0001\n",7))return -1;
    return 0;
}
/* Same rpmsg_ctrl/char UAPI as the existing linux-rpmsg-test.c, with exclusive
 * name admission. No Linux service is silently stopped/replaced. */
static int find_endpoint(char *dev,size_t cap) {
    glob_t paths={0};int found=0;
    int rc=glob("/sys/class/rpmsg/rpmsg*/name",0,NULL,&paths);
    if(rc && rc!=GLOB_NOMATCH)return -1;
    for(size_t i=0;i<paths.gl_pathc;i++) {
        FILE *f=fopen(paths.gl_pathv[i],"r");char name[64]={0};
        if(!f) {found=-1;break;}
        if(!fgets(name,sizeof name,f))name[0]=0;
        fclose(f);if(strcmp(name,"rp1-control\n"))continue;
        char *start=strstr(paths.gl_pathv[i],"/rpmsg/rpmsg");if(!start){found=-1;break;}
        start+=7;size_t n=strcspn(start,"/");
        if(snprintf(dev,cap,"/dev/%.*s",(int)n,start)>=(int)cap){found=-1;break;}
        found++;
    }
    globfree(&paths);return found;
}
static int endpoint(void) {
    char dev[80];if(find_endpoint(dev,sizeof dev)!=0)return -1;
    glob_t list={0};int rc=glob("/dev/rpmsg_ctrl*",0,NULL,&list);
    if(rc || list.gl_pathc!=1){globfree(&list);return -1;}
    int ctrl=open(list.gl_pathv[0],O_RDWR|O_CLOEXEC);globfree(&list);if(ctrl<0)return -1;
    struct rpmsg_endpoint_info info={.name="rp1-control",.src=0xffffffffu,.dst=1024};
    rc=ioctl(ctrl,RPMSG_CREATE_EPT_IOCTL,&info);close(ctrl);if(rc)return -1;
    for(unsigned i=0;i<100;i++) {
        int n=find_endpoint(dev,sizeof dev);if(n<0 || n>1)return -1;
        if(n==1){int fd=open(dev,O_RDWR|O_NONBLOCK|O_CLOEXEC);if(fd>=0)return fd;}
        struct timespec delay={0,10000000};nanosleep(&delay,NULL);
    }
    return -1;
}
static int clock_sample(volatile uint32_t *bar,uint64_t *stamp) {
    volatile uint32_t *t=bar+0xd900/4;
    if(t[0]!=0x31305354 || t[1]!=1 || t[3]!=t[4] || t[3]==UINT32_MAX)return -1;
    boot=mmio_pair(t,5);uint32_t seq=t[3]+1;
    t[3]=seq;fence();uint64_t deadline=now()+1000000000ull;
    while(t[4]!=seq && now()<deadline && !cancelled) {
        struct timespec delay={0,1000000};nanosleep(&delay,NULL);
    }
    fence();if(t[4]!=seq || t[7] || boot!=mmio_pair(t,5) || !boot)return -1;
    *stamp=mmio_pair(t,10);return 0;
}
static int same_boot(volatile uint32_t *bar) {
    fence();return mmio_pair(bar,0xd914/4)==boot && mmio_pair(bar,0xda70/4)==boot;
}
static int wait_buffer(int fd,int tx,uint32_t *r,uint64_t limit,unsigned seq) {
    while(now()<limit && !cancelled) {
        if(call(fd,tx?QUERY_TX:QUERY_RX,NULL,r,0))return -1;
        uint32_t *d=r+16;
        if(d[0]!=1 || pair(d,2)!=boot || pair(d,4)!=session || pair(d,6)!=seq ||
           d[8]!=(tx?2u:1u) || d[9]!=(tx?HALF:0) || d[10]!=HALF || d[13] ||
           d[14]!=UINT32_MAX || d[15])return -1;
        if(d[1]==(tx?2u:0u))return 0;
        if(d[1]!=(tx?1u:3u))return -1;
        struct timespec delay={0,10000000};nanosleep(&delay,NULL);
    }
    return -1;
}
static int overlap_state(const uint32_t *r,uint32_t final_sum) {
    if(r[17]!=1 || r[10]>=final_sum)return -1;
    return r[10]!=0;
}
static int observe_overlap(int fd,uint32_t *r,uint32_t final_sum,uint64_t limit) {
    for(unsigned attempt=0;attempt<100 && now()<limit;++attempt) {
        if(call(fd,QUERY_TX,NULL,r,0))return -1;
        int state=overlap_state(r,final_sum);
        printf("{\"duplex_probe\":%u,\"rx_partial_sum\":%u,\"rx_final_sum\":%u,\"tx_state\":%u,\"rp1_timestamp_us\":%"PRIu64"}\n",attempt,r[10],final_sum,r[17],pair(r,12));
        if(state<0)return -1;
        if(state>0)return 0;
        /* Requests may be drained before this tick's first bulk poll. Zero
         * progress is pending, not evidence of simultaneous active copying. */
        struct timespec delay={0,1000000};nanosleep(&delay,NULL);
    }
    return -1;
}
static void control_command(uint32_t w[32],int stop_control,uint64_t apply,uint64_t until,const uint32_t p[4]) {
    command(w,stop_control?STOP_CONTROL:SET_PARAMETERS,NULL);
    split(w,8,apply);split(w,10,until);
    w[16]=UINT32_C(0x314c5443);w[17]=1;w[18]=stop_control?0:4;
    if(p)memcpy(w+20,p,16);
}
static int control_query(int fd,uint32_t r[32]) {
    if(call(fd,QUERY_CONTROL,NULL,r,0) || r[16]!=UINT32_C(0x31525443) || r[17]!=1 || r[31]&1)return -1;
    return 0;
}
static void control_receipt(const uint32_t *r,const char *label) {
    printf("{\"%s\":\"%s\",\"observation_bound\":\"post_copy\",\"rp1_timestamp_us\":%"PRIu64",\"words\":[",observation_tests?"observation_control_receipt":gpio_phase?"gpio_io_receipt":"control_receipt",label,pair(r,12));
    for(unsigned i=16;i<32;i++)printf("%s%u",i==16?"":",",r[i]);
    puts("]}");
}
static int control_wait(int fd,uint64_t received,unsigned applied,unsigned active,const uint32_t *p,const char *label,uint32_t r[32]) {
    uint64_t end=now()+2000000000ull;
    for(unsigned tries=0;tries<300 && now()<end;tries++) {
        if(control_query(fd,r))return -1;
        if(pair(r+16,2)==received && r[22]==applied && r[26]==active && (!p || !memcmp(r+27,p,16))) {
            control_receipt(r,label);return 0;
        }
        poll(NULL,0,2);
    }
    return -1;
}
static int exercise_control(int fd,uint32_t final_sum,uint64_t limit) {
    uint32_t w[32],original[32],r[32],p[4]={11,22,33,44},next[4]={101,202,303,404},zero[4]={0};
    if(control_query(fd,r) || r[21] || r[22] || r[23] || r[26])return -1;
    uint64_t t=pair(r,12),apply=t+500000,until=apply+700000;
    control_command(w,0,apply,until,p);memcpy(original,w,128);uint64_t first=pair(w,6);
    if(exchange(fd,w,r,0) || r[16]!=UINT32_C(0x31455551) || pair(r+16,2)!=first)return -1;
    if(control_wait(fd,first,0,0,zero,"pending",r) || r[20] || r[21]!=1)return -1;
    /* A second local target is admitted to the queue, then rejected by C1's
     * one-pending-target rule. Its receipt is distinct from the QUEUED ACK. */
    control_command(w,0,apply+100000,until,next);uint64_t second=pair(w,6);
    if(exchange(fd,w,r,0) || control_wait(fd,second,0,0,zero,"target_full",r) || r[20]!=5 || r[21]!=1 || r[23]!=1 || !r[24])return -1;
    if(exchange(fd,original,r,3))return -1;
    for(unsigned i=0;i<9;i++) {
        control_command(w,0,0,until,p);
        unsigned expected=1;
        switch(i) {
        case 0:w[0]=2;break;case 1:w[13]=63;break;
        case 2:w[2]^=1;expected=2;break;case 3:w[4]^=1;expected=2;break;
        case 4:split(w,10,t);expected=4;break;case 5:w[20]=65536;expected=7;break;
        case 6:w[14]=1;break;case 7:w[19]=1;break;
        case 8:control_command(w,1,until,until+1,NULL);expected=4;break;
        }
        if(exchange(fd,w,r,expected))return -1;
    }
    if(control_wait(fd,second,1,1,p,"first_applied",r) || r[20]!=5 || r[21]!=1 || pair(r,12)<apply)return -1;
    t=pair(r,12);control_command(w,0,0,t+250000,next);uint64_t third=pair(w,6);
    if(exchange(fd,w,r,0) || control_wait(fd,third,2,1,next,"atomic_update",r) || r[20] || r[21]!=2)return -1;
    if(control_wait(fd,third,2,0,zero,"expired",r) || r[25]!=1)return -1;
    t=pair(r,12);control_command(w,0,t+2000000,t+3000000,p);uint64_t fourth=pair(w,6);
    if(exchange(fd,w,r,0) || control_wait(fd,fourth,2,0,zero,"next_pending",r) || r[21]!=3)return -1;
    control_command(w,1,0,t+3000000,NULL);uint64_t stopped=pair(w,6);
    if(exchange(fd,w,r,0) || control_wait(fd,stopped,2,0,zero,"stopped",r) || r[20] || r[21]!=4)return -1;
    if(observe_overlap(fd,r,final_sum,limit))return -1;
    puts("{\"control_observation_contract\":\"post_copy\",\"query_mode\":3}");
    printf("{\"control_check\":\"PASS\",\"boot_generation\":\"%016"PRIx64"\",\"session_generation\":\"%016"PRIx64"\",\"header_payload_rejections\":10,\"queued_not_applied_ack\":1,\"future_target\":1,\"atomic_four_parameters\":1,\"expired_inactive\":1,\"stop_clears_pending\":1,\"async_full\":1,\"ddr_still_bidirectional\":1,\"rx_partial_sum\":%u,\"rx_final_sum\":%u,\"tx_state\":%u,\"rp1_timestamp_us\":%"PRIu64",\"actuator\":\"NOT_TESTED\"}\n",boot,session,r[10],final_sum,r[17],pair(r,12));
    return 0;
}
static int proc1_snapshot(volatile uint32_t *bar,uint32_t w[32]) {
    volatile uint32_t *s=bar+0xba00/4;uint64_t end=now()+100000000;
    while(now()<end && !cancelled) {
        uint32_t epoch=s[31];if(epoch&1)continue;
        fence();for(unsigned i=0;i<32;++i)w[i]=s[i];fence();
        if(epoch!=s[31] || w[31]!=epoch)continue;
        if((w[0]!=0x35413150 && w[0]!=0x36413150 && w[0]!=0x37413150 && w[0]!=0x38413150 && w[0]!=0x39413150) || w[1]!=1 || w[2] || w[3] || w[6]!=1 || pair(w,16)!=boot || pair(w,18)!=session)return -1;
        return 0;
    }
    return -1;
}
static int gpio_status(volatile uint32_t *bar,unsigned transitions,unsigned level,const char *phase) {
    uint64_t end=now()+100000000;
    while(now()<end && !cancelled) {
        uint32_t w[32];if(proc1_snapshot(bar,w))return -1;
        if(w[29]==transitions && w[30]==level) {
            printf("{\"gpio_io_status\":\"%s\",\"transitions\":%u,\"level\":%u,\"epoch\":%u,\"host_monotonic_ns\":%"PRIu64"}\n",phase,transitions,level,w[31],now());
            return 0;
        }
        poll(NULL,0,1);
    }
    return -1;
}
static int exercise_gpio_io(int fd,volatile uint32_t *bar,uint32_t sum,uint64_t limit) {
    uint32_t w[32],r[32],p[4]={1,0,0,0},zero[4]={0};gpio_phase=1;
    if(control_query(fd,r) || r[21]!=4 || r[22]!=2 || r[26] || pair(r,12)>UINT64_MAX-2000000)return -1;
    uint64_t until=pair(r,12)+2000000;
    control_command(w,0,0,until,p);uint64_t applied=pair(w,6);
    if(exchange(fd,w,r,0) || control_wait(fd,applied,3,1,p,"applied",r) || r[21]!=5 ||
       gpio_status(bar,3,1,"before_uart") || observe_overlap(fd,r,sum,limit))return -1;
    uint32_t before_sum=r[10];uint64_t before=pair(r,12);
    char b[17],s[17];snprintf(b,sizeof b,"%016"PRIx64,boot);snprintf(s,sizeof s,"%016"PRIx64,session);
    fflush(stdout);pid_t child=fork();if(child<0)return -1;
    if(!child){execl("/bin/amp-bulk","amp-bulk","--gpio-window",b,s,(char *)NULL);_exit(127);}
    int status=0,done=0;uint64_t end=now()+1000000000;
    while(now()<end && !cancelled) {
        pid_t rc=waitpid(child,&status,WNOHANG);
        if(rc==child){done=1;break;}if(rc<0 && errno!=EINTR)break;poll(NULL,0,2);
    }
    if(!done){kill(child,SIGTERM);while(waitpid(child,&status,0)<0 && errno==EINTR){}return -1;}
    if(!WIFEXITED(status) || WEXITSTATUS(status) || gpio_status(bar,3,1,"after_uart") ||
       observe_overlap(fd,r,sum,limit) || r[10]<=before_sum || pair(r,12)>=until)return -1;
    uint32_t after_sum=r[10];uint64_t after=pair(r,12);
    control_command(w,1,0,until,NULL);uint64_t stopped=pair(w,6);
    if(exchange(fd,w,r,0) || control_wait(fd,stopped,3,0,zero,"stopped",r) || r[21]!=6 ||
       gpio_status(bar,4,0,"after_stop"))return -1;
    uint64_t stop_stamp=pair(r,12);
    if(observe_overlap(fd,r,sum,limit))return -1;
    printf("{\"gpio_io_check\":\"PASS\",\"boot_generation\":\"%016"PRIx64"\",\"session_generation\":\"%016"PRIx64"\",\"applied_sequence\":%"PRIu64",\"stop_sequence\":%"PRIu64",\"before_rp1_us\":%"PRIu64",\"after_rp1_us\":%"PRIu64",\"stop_rp1_us\":%"PRIu64",\"valid_until_us\":%"PRIu64",\"rx_sum_before\":%u,\"rx_sum_after\":%u,\"rx_final_sum\":%u,\"tx_state\":%u,\"rp1_timestamp_us\":%"PRIu64",\"realtime\":\"NOT_TESTED\"}\n",
        boot,session,applied,stopped,before,after,stop_stamp,until,before_sum,after_sum,sum,r[17],pair(r,12));
    gpio_phase=0;return 0;
}
static int observation_record(const uint32_t *w,const uint32_t *d) {
    if(w[0]!=0x3153424f || w[1]!=1 || !boot || !session || pair(w,2)!=boot || pair(w,4)!=session ||
       !pair(w,6) || w[23] || (w[22]&~15u) || pair(w,10)>pair(w,12) || pair(w,12)>pair(w,14) ||
       pair(w,14)>pair(w,16) || pair(w,18)>pair(w,12))return -1;
    if(w[22]&1) {if(pair(w,8)>pair(w,10) || pair(w,8)>pair(w,18))return -1;}
    else if(pair(w,8))return -1;
    if(w[22]&8) {if(!w[20] || w[21]==UINT32_MAX)return -1;}
    else if(w[20] || w[21]!=UINT32_MAX)return -1;
    if(d && (pair(w,6)!=pair(d,6) || pair(w,18)!=pair(d,11) || w[20]!=d[13] || w[21]!=d[14]))return -1;
    return 0;
}
static int observation_descriptor(const uint32_t *d) {
    return d[0]==1 && d[1]==2 && pair(d,2)==boot && pair(d,4)==session && pair(d,6)>0 &&
        d[8]==2 && d[9]==HALF && d[10]==OBS_BYTES && !d[15] ? 0:-1;
}
static int observation_counters(const uint32_t *r) {
    const uint32_t *c=r+16;
    return r[0]==0x31524b42 && r[1]==1 && !r[2] && r[3]==QUERY_OBSERVATIONS && pair(r,4)>0 &&
        r[11]<=1 && r[14]==4 && r[15]==5 && c[0]==0x3152424f && c[1]==1 && pair(c,2)==boot && pair(c,4)==session &&
        c[11]==OBS_BYTES && c[12]<=1 && !c[13] && !c[14] && !c[15] ? 0:-1;
}
static int write_all(int fd,const unsigned char *b,size_t size) {
    while(size) {
        ssize_t n=write(fd,b,size);
        if(n<0 && errno==EINTR)continue;
        if(n<=0)return -1;
        b+=n;size-=(size_t)n;
    }
    return 0;
}
/* Read-only cross-language fixture validation. No MMIO/serial endpoint opens. */
static int check_observation_wire(const char *path,const char *reply_path) {
    FILE *f=fopen(path,"rb");if(!f)return -1;
    unsigned char bytes[OBS_BYTES];uint32_t w[24],r[32];unsigned count=0,e2e=0;
    uint64_t last_sequence=0,last_timestamp=0;int result=-1;
    for(;;) {
        size_t n=fread(bytes,1,sizeof bytes,f);
        if(!n) {if(ferror(f))goto done;break;}
        if(n!=sizeof bytes || count==4096)goto done;
        for(unsigned i=0;i<24;i++)w[i]=get(bytes+4*i);
        if(!count) {boot=pair(w,2);session=pair(w,4);}
        if(observation_record(w,NULL) || pair(w,6)<=last_sequence || pair(w,18)<last_timestamp)goto done;
        last_sequence=pair(w,6);last_timestamp=pair(w,18);count++;e2e+=(w[22]&7)==7;
    }
    if(!count)goto done;
    fclose(f);f=fopen(reply_path,"rb");if(!f)return -1;
    unsigned char reply[128];
    if(fread(reply,1,sizeof reply,f)!=sizeof reply || fgetc(f)!=EOF || ferror(f))goto done;
    for(unsigned i=0;i<32;i++)r[i]=get(reply+4*i);
    if(observation_counters(r) || pair(r+16,6)<count)goto done;
    printf("HOST_PASS: OBS1 frames=%u bytes=%u end_to_end_flags=%u published=%"PRIu64" faults=%"PRIu64" drop_newest=%u telemetry_retired=%u; no hardware access\n",
        count,count*OBS_BYTES,e2e,pair(r+16,6),pair(r+16,8),r[26],r[28]);result=0;
done:
    fclose(f);return result;
}
static int hold_observation(int fd,volatile uint32_t *ddr,volatile uint32_t *bar,const uint32_t *d,const uint32_t *words) {
    uint32_t before[32],after[32],r[32],counter_before[32],last_descriptor[16],last_payload[24];
    if(proc1_snapshot(bar,before) || before[29]!=1 || before[30]!=1 ||
       call(fd,QUERY_OBSERVATIONS,NULL,r,0) || observation_counters(r) || pair(r+16,8) || r[28])return -1;
    uint64_t published=pair(r+16,6),start=now(),end=start+100000000;
    uint32_t drops=r[26];
    memcpy(counter_before,r,128);
    do {
        if(!same_boot(bar) || call(fd,QUERY_TX,NULL,r,0) || memcmp(r+16,d,64))return -1;
        fence();for(unsigned i=0;i<24;i++)if(ddr[WORDS+i]!=words[i])return -1;fence();
        poll(NULL,0,2);
    } while(now()<end && !cancelled);
    memcpy(last_descriptor,r+16,64);
    fence();for(unsigned i=0;i<24;i++)last_payload[i]=ddr[WORDS+i];fence();
    if(memcmp(last_payload,words,OBS_BYTES))return -1;
    if(cancelled || proc1_snapshot(bar,after) || after[29]!=1 || after[30]!=1 ||
       !(uint32_t)(after[7]-before[7]) || (uint32_t)(after[7]-before[7])>=0x80000000u ||
       call(fd,QUERY_OBSERVATIONS,NULL,r,0) || observation_counters(r) || pair(r+16,6)!=published ||
       pair(r+16,8) || r[28] || !(uint32_t)(r[26]-drops) || (uint32_t)(r[26]-drops)>=0x80000000u)return -1;
    uint64_t finish=now();
    printf("{\"observation_ready_hold\":\"PASS\",\"host_hold_ns\":%"PRIu64",\"host_before_ns\":%"PRIu64",\"host_after_ns\":%"PRIu64",\"cycles_before\":%u,\"cycles_after\":%u,\"drop_newest_before\":%u,\"drop_newest_after\":%u,\"published_held\":%"PRIu64",\"descriptor_payload_unchanged\":1,\"gpio22_level\":1,\"realtime\":\"NOT_TESTED\"",
        finish-start,start,finish,before[7],after[7],drops,r[26],published);
    const char *names[]={"proc1_before","proc1_after","counters_before","counters_after","descriptor_before","descriptor_after","payload_before","payload_after"};
    const uint32_t *values[]={before,after,counter_before,r,d,last_descriptor,words,last_payload};
    const unsigned widths[]={32,32,32,32,16,16,24,24};
    for(unsigned a=0;a<8;a++) {
        printf(",\"%s\":[",names[a]);
        for(unsigned i=0;i<widths[a];i++)printf("%s%u",i?",":"",values[a][i]);
        putchar(']');
    }
    puts("}");return 0;
}
static int exercise_observations(int fd,volatile uint32_t *ddr,volatile uint32_t *bar,uint64_t limit,const char *path) {
    uint32_t r[32],d[16],words[24];unsigned char wire[128];char reply_path[4096];
    int len=snprintf(reply_path,sizeof reply_path,"%s.reply",path);
    if(len<0 || (size_t)len>=sizeof reply_path)return -1;
    int trace=open(path,O_WRONLY|O_CREAT|O_EXCL|O_CLOEXEC|O_NOFOLLOW,0600);if(trace<0)return -1;
    int counters=open(reply_path,O_WRONLY|O_CREAT|O_EXCL|O_CLOEXEC|O_NOFOLLOW,0600),result=-1;
    unsigned captured=0,e2e=0;uint64_t last_sequence=0,last_timestamp=0,min_end=0,until=0,applied=0;
    uint32_t w[32],parameters[4]={1,2,3,4},zero[4]={0};
    if(counters<0)goto done;
    observation_start_attempted=1;
    if(call(fd,START_OBSERVATIONS,NULL,r,0))goto done;
    if(control_tests) {
        if(control_query(fd,r) || r[21] || r[22] || r[23] || r[26] || pair(r,12)>UINT64_MAX-2000000)goto done;
        until=pair(r,12)+2000000;control_command(w,0,0,until,parameters);applied=pair(w,6);
        if(exchange(fd,w,r,0) || control_wait(fd,applied,1,1,parameters,"applied",r) || r[20] || r[21]!=1)goto done;
        min_end=now()+1000000000;
        printf("{\"observation_control_started\":1,\"applied_sequence\":%"PRIu64",\"rp1_timestamp_us\":%"PRIu64",\"valid_until_us\":%"PRIu64",\"host_monotonic_ns\":%"PRIu64"}\n",applied,pair(r,12),until,now());fflush(stdout);
    }
    while((captured<32 || now()<min_end) && now()<limit && !cancelled) {
        if(!same_boot(bar) || call(fd,QUERY_TX,NULL,r,0))goto done;
        if(r[17]==0 || r[17]==1) {poll(NULL,0,1);continue;}
        memcpy(d,r+16,64);
        if(observation_descriptor(d))goto done; /* Reject before any payload access. */
        if(captured==4096)goto done; /* Same finite bound as the independent decoder. */
        fence();for(unsigned i=0;i<24;i++)words[i]=ddr[WORDS+i];fence();
        if(observation_record(words,d) || pair(words,6)<=last_sequence || pair(words,18)<last_timestamp)goto done;
        if(control_tests && !captured && hold_observation(fd,ddr,bar,d,words))goto done;
        for(unsigned i=0;i<24;i++)put(wire+4*i,words[i]);
        if(write_all(trace,wire,OBS_BYTES))goto done;
        last_sequence=pair(words,6);last_timestamp=pair(words,18);captured++;e2e+=(words[22]&7)==7;
        d[1]=0;
        if(call(fd,TX_FREE,d,r,0) || r[17])goto done;
        if(control_tests)printf("{\"observation_frame\":%u,\"sequence\":%"PRIu64",\"sample_timestamp_rp1\":%"PRIu64",\"host_monotonic_ns\":%"PRIu64"}\n",captured,last_sequence,last_timestamp,now());
    }
    if(cancelled || captured<32 || now()<min_end)goto done;
    if(control_tests) {
        if(control_query(fd,r) || pair(r+16,2)!=applied || r[21]!=1 || r[22]!=1 || r[26]!=1 ||
           memcmp(r+27,parameters,16) || pair(r,12)>=until)goto done;
        control_receipt(r,"before_stop");
        uint64_t before_stop=pair(r,12);control_command(w,1,0,until,NULL);uint64_t stopped=pair(w,6);
        if(exchange(fd,w,r,0) || control_wait(fd,stopped,1,0,zero,"stopped",r) || r[20] || r[21]!=2 ||
           gpio_status(bar,2,0,"observation_stopped"))goto done;
        printf("{\"observation_control_check\":\"PASS\",\"applied_sequence\":%"PRIu64",\"stop_sequence\":%"PRIu64",\"before_stop_rp1_us\":%"PRIu64",\"stop_rp1_us\":%"PRIu64",\"valid_until_us\":%"PRIu64",\"host_monotonic_ns\":%"PRIu64",\"gpio22_final_level\":0,\"realtime\":\"NOT_TESTED\"}\n",applied,stopped,before_stop,pair(r,12),until,now());
    }
    if(call(fd,QUERY_OBSERVATIONS,NULL,r,0) || observation_counters(r) ||
       pair(r+16,6)<captured || pair(r+16,8) || r[28])goto done;
    for(unsigned i=0;i<32;i++)put(wire+4*i,r[i]);
    if(write_all(counters,wire,128) || fsync(trace) || fsync(counters))goto done;
    printf("{\"observation_frames\":%u,\"bytes\":%u,\"end_to_end_flags\":%u,\"published\":%"PRIu64",\"faults\":%"PRIu64",\"drop_newest\":%u,\"realtime\":\"NOT_TESTED\"}\n",
        captured,captured*OBS_BYTES,e2e,pair(r+16,6),pair(r+16,8),r[26]);result=0;
done:
    if(counters>=0)close(counters);
    close(trace);return result;
}
static int exercise(int fd,volatile uint32_t *ddr,volatile uint32_t *bar,uint64_t limit) {
    uint32_t w[32],r[32],d[16],ack[16];
    command(w,QUERY_RX,NULL);w[2]^=1;if(exchange(fd,w,r,2))return -1;
    command(w,QUERY_RX,NULL);w[0]=2;if(exchange(fd,w,r,1))return -1;
    command(w,QUERY_RX,NULL);w[13]=63;if(exchange(fd,w,r,1))return -1;
    command(w,QUERY_RX,NULL);w[10]=1;w[11]=0;if(exchange(fd,w,r,4))return -1;
    command(w,QUERY_RX,NULL);if(exchange(fd,w,r,0) || exchange(fd,w,r,3))return -1;
    descriptor(d,0,1);d[9]=2;if(call(fd,RX_READY,d,r,7))return -1;
    descriptor(d,0,1);d[10]=HALF+4;if(call(fd,RX_READY,d,r,7))return -1;
    uint32_t sum=0;
    for(unsigned i=0;i<WORDS;i++){ddr[i]=i;sum+=i;}fence();
    descriptor(d,0,1);if(call(fd,RX_READY,d,r,0))return -1;
    descriptor(d,1,1);if(call(fd,TX_START,d,r,0))return -1;
    /* One RP1 reply snapshots partial RX progress and a still-writing TX slot. */
    if(observe_overlap(fd,r,sum,limit))return -1;
    printf("{\"duplex_overlap\":\"PASS\",\"rx_partial_sum\":%u,\"rx_final_sum\":%u,\"tx_state\":1,\"rp1_timestamp_us\":%"PRIu64"}\n",r[10],sum,pair(r,12));
    if(control_tests && exercise_control(fd,sum,limit))return -1;
    if(gpio_io_tests && exercise_gpio_io(fd,bar,sum,limit))return -1;
    if(wait_buffer(fd,0,r,limit,1) || r[10]!=sum)return -1;
    if(wait_buffer(fd,1,r,limit,1) || !pair(r+16,11))return -1;
    memcpy(ack,r+16,64);ack[1]=0;
    fence();for(unsigned i=0;i<WORDS;i++)if(ddr[WORDS+i]!=((1+i)^0xa55aa55au))return -1;
    fence();
    uint64_t before=pair(r,8);
    for(unsigned i=0;i<100;i++) {descriptor(d,1,2);if(call(fd,TX_START,d,r,5))return -1;}
    if(pair(r,8)!=before+100)return -1;
    /* Linux deliberately holds the output while the reverse direction runs. */
    sum=0;for(unsigned i=0;i<WORDS;i++){uint32_t v=i^0x5555aaaau;ddr[i]=v;sum+=v;}fence();
    descriptor(d,0,2);if(call(fd,RX_READY,d,r,0) || wait_buffer(fd,0,r,limit,2) || r[10]!=sum)return -1;
    fence();for(unsigned i=0;i<WORDS;i++)if(ddr[WORDS+i]!=((1+i)^0xa55aa55au))return -1;
    fence();
    ack[11]^=1;if(call(fd,TX_FREE,ack,r,3))return -1;ack[11]^=1;
    if(call(fd,TX_FREE,ack,r,0) || r[17]!=0)return -1;
    printf("{\"bulk_check\":\"PASS\",\"linux_to_rp1_bytes\":65536,\"rp1_to_linux_bytes\":32768,\"tx_full_rejected\":100,\"simultaneous_bidirectional\":1,\"realtime\":\"NOT_TESTED\"}\n");
    return 0;
}
static void self_test(void) {
    uint32_t w[32],r[32]={0},d[16];unsigned char wire[128];
    boot=0x1020304050607080ull;session=29;sequence=0;valid_until=100000;
    descriptor(d,1,7);command(w,TX_START,d);
    for(unsigned i=0;i<32;i++){put(wire+4*i,w[i]);assert(get(wire+4*i)==w[i]);}
    assert(!memcmp(wire+8,"\x80\x70\x60\x50\x40\x30\x20\x10",8));
    assert(w[6]==1 && w[12]==3 && w[13]==64 && w[25]==HALF && w[26]==HALF && w[30]==UINT32_MAX);
    r[0]=0x31524b42;r[1]=1;r[3]=3;r[4]=1;r[14]=4;
    assert(valid_reply(w,r,0));r[4]++;assert(!valid_reply(w,r,0));r[4]--;r[2]=5;
    assert(!valid_reply(w,r,0) && valid_reply(w,r,5));r[15]=1;assert(!valid_reply(w,r,5));
    w[12]=SET_PARAMETERS;r[3]=SET_PARAMETERS;r[2]=0;
    assert(valid_reply(w,r,0));r[15]=0;assert(!valid_reply(w,r,0));
    w[12]=QUERY_CONTROL;r[3]=QUERY_CONTROL;r[15]=3;assert(valid_reply(w,r,0));
    r[15]=2;assert(!valid_reply(w,r,0));
    r[15]=1;assert(!valid_reply(w,r,0));
    w[12]=START_OBSERVATIONS;r[3]=START_OBSERVATIONS;r[15]=4;assert(valid_reply(w,r,0));
    r[15]=0;assert(!valid_reply(w,r,0));
    w[12]=QUERY_OBSERVATIONS;r[3]=QUERY_OBSERVATIONS;r[15]=5;assert(valid_reply(w,r,0));
    r[15]=4;assert(!valid_reply(w,r,0));
    observation_active=1;w[12]=QUERY_TX;r[3]=QUERY_TX;assert(valid_reply(w,r,0));
    r[15]=0;assert(!valid_reply(w,r,0));
    observation_active=0;observation_start_attempted=1;w[12]=RETIRE;r[3]=RETIRE;
    assert(valid_reply(w,r,0));r[15]=4;assert(valid_reply(w,r,0));r[15]=5;assert(!valid_reply(w,r,0));
    observation_start_attempted=0;
    uint32_t obs[24]={0};obs[0]=0x3153424f;obs[1]=1;split(obs,2,boot);split(obs,4,session);obs[6]=3;
    for(unsigned i=10;i<=18;i+=2)obs[i]=100;
    obs[21]=UINT32_MAX;
    memset(d,0,sizeof d);d[0]=1;d[1]=2;split(d,2,boot);split(d,4,session);d[6]=3;d[8]=2;d[9]=HALF;d[10]=OBS_BYTES;d[11]=100;d[14]=UINT32_MAX;
    assert(!observation_descriptor(d) && !observation_record(obs,d));
    for(unsigned i=0;i<16;i++) {
        if(i==6 || i==7 || i==11 || i==12 || i==13 || i==14)continue;
        uint32_t saved=d[i];d[i]^=1;assert(observation_descriptor(d));d[i]=saved;
    }
    for(unsigned i=0;i<24;i++) {
        if(i==7 || (i>=10 && i<=17) || i==19)continue;
        uint32_t saved=obs[i];obs[i]^=i==22?16:1;assert(observation_record(obs,d));obs[i]=saved;
    }
    obs[22]=7;obs[8]=98;assert(!observation_record(obs,d));obs[8]=101;assert(observation_record(obs,d));
    obs[8]=0;obs[22]=0;
    r[17]=1;r[10]=0;assert(overlap_state(r,100)==0);
    r[10]=1;assert(overlap_state(r,100)==1);
    r[10]=99;assert(overlap_state(r,100)==1);
    r[10]=100;assert(overlap_state(r,100)==-1);
    r[10]=101;assert(overlap_state(r,100)==-1);
    for(unsigned state=0;state<6;state++)if(state!=1){r[17]=state;r[10]=1;assert(overlap_state(r,100)==-1);}
    unsigned char be[]={0x21,0x10,0,0};assert(be_cell(be,0x21100000) && !be_cell(be,0x21000000));
    /* Exercise the actual nonblocking message path with a local packet socket;
     * an echo and a short reply must never count as protocol acceptance. */
    int sockets[2];assert(!socketpair(AF_UNIX,SOCK_SEQPACKET|SOCK_CLOEXEC,0,sockets));
    pid_t child=fork();assert(child>=0);
    if(!child) {
        close(sockets[0]);unsigned char request[128],response[128];
        for(unsigned i=0;i<3;i++) {
            assert(read(sockets[1],request,128)==128);
            assert(get(request)==1 && get(request+48)==QUERY_RX);
            memcpy(response,request,128);
            if(i==0) {
                memset(response,0,128);put(response,0x31524b42);put(response+4,1);
                put(response+12,QUERY_RX);memcpy(response+16,request+24,8);put(response+56,4);
            }
            unsigned n=i==2?16:128;assert(write(sockets[1],response,n)==n);
        }
        close(sockets[1]);_exit(0);
    }
    close(sockets[1]);assert(!fcntl(sockets[0],F_SETFL,O_NONBLOCK));
    assert(!call(sockets[0],QUERY_RX,NULL,r,0));
    assert(call(sockets[0],QUERY_RX,NULL,r,0)==-1);
    assert(call(sockets[0],QUERY_RX,NULL,r,0)==-1);
    close(sockets[0]);int status;assert(waitpid(child,&status,0)==child && WIFEXITED(status) && !WEXITSTATUS(status));
    puts("PASS: wire endian/layout, generation, sequence/status, FDT cell, packet transport, echo/short rejection; no hardware access");
}
int main(int argc,char **argv) {
#ifdef AMP_GPIO_UART
    if(argc>=2 && !strcmp(argv[1],"--gpio-window")) {
        if(argc==3 && !strcmp(argv[2],"--self-test"))return uart_gpio_main(2,argv+1);
        return uart_gpio_main(argc,argv);
    }
#endif
    if(argc==2 && !strcmp(argv[1],"--self-test")){self_test();return 0;}
    if(argc==2 && !strcmp(argv[1],"--monotonic-ns")){printf("%"PRIu64"\n",now());return 0;}
    if(argc==4 && !strcmp(argv[1],"--check-observation-wire"))return check_observation_wire(argv[2],argv[3])?1:0;
    /* Read-only host/target check. This path never opens MMIO or uses the
     * supplied tree as an override for hardware-mode admission. */
    if(argc==3 && !strcmp(argv[1],"--check-reservations")) {
        int rc=reservations(argv[2]);puts(rc ? "STATIC_RESERVATIONS_FAIL" : "STATIC_RESERVATIONS_PASS");return rc?1:0;
    }
    observation_tests=(argc==5 || argc==6) && !strcmp(argv[3],"--observations");
    if((argc!=3 && argc!=4 && argc!=5 && argc!=6) || strcmp(argv[1],"--admitted-scratch-64k") ||
       (argc==6 && (!observation_tests || strcmp(argv[5],"--control"))) ||
       (!observation_tests && ((argc>=4 && strcmp(argv[3],"--control")) || (argc==5 && strcmp(argv[4],"--gpio-io"))))) {
        fprintf(stderr,"usage: %s --self-test | --monotonic-ns | --check-reservations TREE_PATH | --check-observation-wire FRAMES COUNTERS | --admitted-scratch-64k NEW_BACKUP_PATH [--control [--gpio-io] | --observations NEW_TRACE_PATH [--control]]\n",argv[0]);return 2;
    }
    control_tests=observation_tests?argc==6:argc>=4;gpio_io_tests=!observation_tests && argc==5;
#ifndef AMP_GPIO_UART
    if(gpio_io_tests){fprintf(stderr,"GPIO/UART helper is not linked\n");return 2;}
#endif
#ifndef __aarch64__
    fprintf(stderr,"hardware mode requires the reviewed AArch64 Linux mapping contract\n");return 2;
#endif
    int result=1,retired=0,restored=0,started=0,fd=-1,lock=-1,save=-1;
    const char *phase="exclusive lock / FDT / reservation / PCI admission";
    volatile uint32_t *bar=MAP_FAILED,*ddr=MAP_FAILED;uint32_t backup[SIZE/4],r[32];
    signal(SIGINT,stop);signal(SIGTERM,stop);
    lock=open("/tmp/ddr.lock",O_CREAT|O_RDWR|O_CLOEXEC|O_NOFOLLOW,0600);
    if(lock<0 || flock(lock,LOCK_EX|LOCK_NB) || admission())goto done;
    phase="nonce / BAR2 / BKB1 capability / generation";
    unsigned char nonce[8];if(getrandom(nonce,sizeof nonce,0)!=sizeof nonce)goto done;
    session=(uint64_t)get(nonce)|(uint64_t)get(nonce+4)<<32;if(!session)goto done;
    int mem=open("/sys/bus/pci/devices/0002:01:00.0/resource2",O_RDWR|O_SYNC|O_CLOEXEC);
    if(mem<0)goto done;
    bar=mmap(NULL,SIZE,PROT_READ|PROT_WRITE,MAP_SHARED,mem,0);close(mem);if(bar==MAP_FAILED)goto done;
    fence();volatile uint32_t *rpc=bar+0xd940/4,*control=bar+0xda40/4;
    if(rpc[0]!=0x31524444 || rpc[1]!=2 || rpc[56]!=0x31424b42 || rpc[63] ||
       rpc[10]!=2 || rpc[2]!=rpc[3] || control[6]!=0x31504d41 || control[11]!=1)goto done;
    uint64_t stamp;if(clock_sample(bar,&stamp) || !same_boot(bar) || stamp>UINT64_MAX-60000000)goto done;
    valid_until=stamp+60000000;
    phase="exclusive backup / fixed DDR mapping";
    save=open(argv[2],O_WRONLY|O_CREAT|O_EXCL|O_CLOEXEC|O_NOFOLLOW,0600);if(save<0)goto done;
    mem=open("/dev/mem",O_RDWR|O_SYNC|O_CLOEXEC);if(mem<0)goto done;
    ddr=mmap(NULL,SIZE,PROT_READ|PROT_WRITE,MAP_SHARED,mem,0x21000000);close(mem);if(ddr==MAP_FAILED)goto done;
    for(unsigned i=0;i<SIZE/4;i++)backup[i]=ddr[i];
    fence();
    if(write(save,backup,SIZE)!=SIZE || fsync(save))goto done;
    close(save);save=-1;
    phase="exclusive RPMsg endpoint";
    fd=endpoint();if(fd<0)goto done;
    uint64_t limit=now()+45000000000ull;
    run_until=limit;
    printf("{\"boot_generation\":\"%016"PRIx64"\",\"session_generation\":\"%016"PRIx64"\",\"rp1_start_us\":%"PRIu64"}\n",boot,session,stamp);fflush(stdout);
    started=1;
    phase="HELLO / region claim";
    if(call(fd,HELLO,NULL,r,0) || !same_boot(bar) || rpc[63]!=1)goto done;
    phase=observation_tests?"OBS1 stream / ownership / counters":"duplex / ownership / rejection checks";
    result=(observation_tests?exercise_observations(fd,ddr,bar,limit,argv[4]):exercise(fd,ddr,bar,limit))!=0;
done:
    /* Only an explicit same-generation RETIRE acknowledgement releases both
     * directions for backup restoration. On uncertainty keep the backup and
     * leave DDR untouched. Never restore iATU/remove remoteproc/reset RP1 here. */
    cancelled=0;
    run_until=0;
    if(result)fprintf(stderr,"failed at: %s\n",phase);
    if(started && fd>=0 && same_boot(bar) && !call(fd,RETIRE,NULL,r,0) && r[11]==1 && same_boot(bar)) {
        retired=1;
        for(unsigned i=0;i<SIZE/4;i++)ddr[i]=backup[i];
        fence();
        restored=1;for(unsigned i=0;i<SIZE/4;i++)if(ddr[i]!=backup[i])restored=0;
        fence();
    }
    if(fd>=0){if(ioctl(fd,RPMSG_DESTROY_EPT_IOCTL)<0)result=1;close(fd);}
    if(save>=0)close(save);
    if(ddr!=MAP_FAILED)munmap((void *)ddr,SIZE);
    if(bar!=MAP_FAILED)munmap((void *)bar,SIZE);
    if(lock>=0)close(lock);
    result=result || !retired || !restored;
    printf("{\"result\":\"%s\",\"rpmsg_replies\":%u,\"retired\":%d,\"restored\":%d}\n",result?"FAIL":"PASS",exchanges,retired,restored);
    return result;
}
