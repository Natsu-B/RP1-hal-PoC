/* Real capture loop over anonymous shared memory/local packet sockets. No MMIO. */
#define _GNU_SOURCE
#define main bulk_client_main
#include "linux-amp-bulk.c"
#undef main

static void server(int fd,volatile uint32_t *ddr,volatile uint32_t *bar,int controlled,int mutation) {
    unsigned ready=0,published=0,drops=0,accepted=0,active=0;uint64_t control_sequence=0,start=now();
    uint32_t d[16]={0},w[32],r[32],parameters[4]={0};unsigned char wire[128];
    while(read(fd,wire,128)==128) {
        for(unsigned i=0;i<32;i++)w[i]=get(wire+i*4);
        assert(w[0]==1 && w[1]==64 && pair(w,2)==boot && pair(w,4)==session);
        uint64_t timestamp=1000+(now()-start)/1000;
        if(controlled) {
            volatile uint32_t *s=bar+0xba00/4;s[31]=1;fence();
            s[0]=0x39413150;s[1]=1;s[6]=1;s[7]=mutation==2?1:1+(unsigned)(timestamp/200);
            s[4]=0x2000c700;s[5]=0x2000be00;s[9]=s[7]>8?s[7]-8:0;s[10]=s[11]=200;s[12]=s[7];
            s[14]=(uint32_t)timestamp;s[20]=0x2000bf48;s[21]=0x2000c748;
            s[16]=mutation==4?(uint32_t)boot+1:(uint32_t)boot;s[18]=(uint32_t)session;
            s[29]=active?1:accepted?2:0;s[30]=active;fence();s[31]=s[7]*2;
        }
        memset(r,0,sizeof r);r[0]=0x31524b42;r[1]=1;r[3]=w[12];split(r,4,pair(w,6));r[14]=4;r[15]=4;
        if(w[12]==QUERY_TX) {
            if(!ready) {
                if(controlled)poll(NULL,0,1); /* Synthetic producer, not an IRQ/performance model. */
                assert(controlled || published<32);published++;ready=1;
                uint32_t frame[24]={0};frame[0]=0x3153424f;frame[1]=1;split(frame,2,boot);split(frame,4,session);
                frame[6]=published;for(unsigned i=10;i<=18;i+=2)split(frame,i,controlled?timestamp:published*200);frame[21]=UINT32_MAX;
                for(unsigned i=0;i<24;i++)ddr[WORDS+i]=frame[i];
                fence();
                memset(d,0,sizeof d);d[0]=1;d[1]=2;split(d,2,boot);split(d,4,session);d[6]=published;
                d[8]=2;d[9]=mutation==1?HALF+4:HALF;d[10]=OBS_BYTES;split(d,11,controlled?timestamp:published*200);d[14]=UINT32_MAX;
            } else {
                assert(controlled);
                if(mutation!=5)drops++;
                if(mutation==3)ddr[WORDS+6]++;
                if(mutation==6)d[6]++;
            }
            memcpy(r+16,d,64);
        } else if(w[12]==TX_FREE) {
            assert(ready);d[1]=0;assert(!memcmp(w+16,d,64));ready=0;memcpy(r+16,d,64);
        } else if(w[12]==QUERY_OBSERVATIONS) {
            assert(controlled || (published==32 && !ready));r[15]=5;r[16]=0x3152424f;r[17]=1;
            split(r+16,2,boot);split(r+16,4,session);r[22]=published;r[27]=OBS_BYTES;
            r[26]=drops;
        } else if(controlled && (w[12]==SET_PARAMETERS || w[12]==STOP_CONTROL)) {
            control_sequence=pair(w,6);accepted++;active=w[12]==SET_PARAMETERS;
            if(active)memcpy(parameters,w+20,16);else memset(parameters,0,16);
            r[15]=w[12]==STOP_CONTROL && mutation==7?4:1;r[16]=0x31455551;split(r+16,2,control_sequence);
        } else if(controlled && w[12]==QUERY_CONTROL) {
            r[15]=3;split(r,12,timestamp);r[16]=0x31525443;r[17]=1;split(r+16,2,control_sequence);
            r[21]=accepted;r[22]=accepted?1:0;r[26]=active;memcpy(r+27,parameters,16);
            r[31]=accepted*2;
        } else assert(w[12]==START_OBSERVATIONS && !published && !ready);
        for(unsigned i=0;i<32;i++)put(wire+4*i,r[i]);
        assert(write(fd,wire,128)==128);
    }
    close(fd);_exit(0);
}
static void capture_test(const char *path,int controlled,int mutation) {
    volatile uint32_t *ddr=mmap(NULL,SIZE,PROT_READ|PROT_WRITE,MAP_SHARED|MAP_ANONYMOUS,-1,0);
    volatile uint32_t *bar=mmap(NULL,SIZE,PROT_READ|PROT_WRITE,MAP_SHARED|MAP_ANONYMOUS,-1,0);
    assert(ddr!=MAP_FAILED && bar!=MAP_FAILED);bar[0xd914/4]=(uint32_t)boot;bar[0xda70/4]=(uint32_t)boot;
    int sockets[2];assert(!socketpair(AF_UNIX,SOCK_SEQPACKET|SOCK_CLOEXEC,0,sockets));
    pid_t child=fork();assert(child>=0);
    if(!child) {close(sockets[0]);server(sockets[1],ddr,bar,controlled,mutation);}
    close(sockets[1]);assert(!fcntl(sockets[0],F_SETFL,O_NONBLOCK));
    if(mutation==1)assert(!mprotect((void *)ddr,SIZE,PROT_NONE));
    sequence=0;observation_active=0;observation_start_attempted=0;observation_tests=1;control_tests=controlled;
    assert(exercise_observations(sockets[0],ddr,bar,now()+3000000000ull,path)==(mutation?-1:0));
    close(sockets[0]);int status;assert(waitpid(child,&status,0)==child && WIFEXITED(status) && !WEXITSTATUS(status));
    if(!mutation) {
        char reply[4096];assert(snprintf(reply,sizeof reply,"%s.reply",path)>0);
        assert(!check_observation_wire(path,reply));
    }
    assert(!munmap((void *)ddr,SIZE));assert(!munmap((void *)bar,SIZE));
}
int main(int argc,char **argv) {
    assert(argc==2);boot=17;session=29;valid_until=100000;
    char path[4096],bad[4096],reply[4096];
    assert(snprintf(path,sizeof path,"%s/capture",argv[1])>0);
    assert(snprintf(bad,sizeof bad,"%s/bad",argv[1])>0);
    capture_test(path,0,0);capture_test(bad,0,1);
    char combined[4096];assert(snprintf(combined,sizeof combined,"%s/combined",argv[1])>0);
    capture_test(combined,1,0);
    for(unsigned mutation=2;mutation<=7;mutation++) {
        assert(snprintf(combined,sizeof combined,"%s/combined-bad-%u",argv[1],mutation)>0);
        capture_test(combined,1,mutation);
    }
    int fd=open(path,O_RDONLY|O_CLOEXEC);assert(fd>=0);unsigned char saved[96];assert(read(fd,saved,96)==96);close(fd);
    observation_start_attempted=0;
    assert(exercise_observations(-1,NULL,NULL,now()+2000000000ull,path)==-1 && !observation_start_attempted);
    fd=open(path,O_RDONLY|O_CLOEXEC);assert(fd>=0);unsigned char after[96];assert(read(fd,after,96)==96 && !memcmp(saved,after,96));close(fd);
    assert(snprintf(path,sizeof path,"%s/collision",argv[1])>0);
    assert(snprintf(reply,sizeof reply,"%s/collision.reply",argv[1])>0);
    fd=open(reply,O_WRONLY|O_CREAT|O_EXCL|O_CLOEXEC,0600);assert(fd>=0 && write(fd,"keep",4)==4);close(fd);
    assert(exercise_observations(-1,NULL,NULL,now()+2000000000ull,path)==-1 && !observation_start_attempted);
    fd=open(reply,O_RDONLY|O_CLOEXEC);assert(fd>=0 && read(fd,after,4)==4 && !memcmp(after,"keep",4));close(fd);
    puts("HOST_PASS: capture32/exact FREE/counters, combined control/100ms held READY/1s stream/STOP, six combined mutations refused, invalid descriptor before PROT_NONE DDR read, O_EXCL collisions; no hardware");
    return 0;
}
