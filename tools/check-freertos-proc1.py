#!/usr/bin/env python3
"""P1R1 ABI v2 under proc0 FreeRTOS. Pure validation; no MMIO/transport."""
if not __debug__:
    raise SystemExit('Evidence admission requires assertions; refuse -O')

EXPECTED_TRACE=[(1,1,1,1),(1,2,2,2),(0,3,4,3),(2,1,3,4),
                (1,3,4,5),(2,2,1,6),(2,2,5,7),(2,3,6,8),
                (2,3,2,9),(3,1,3,10),(3,2,1,11),(3,3,2,12)]

def require(ok,why):
    if not ok:raise ValueError(why)

def protocol(w):
    """Agreed 12-request response trace only; linked/runtime checks still required."""
    require(w[25:30]==[3,3,2,12,12],'last acquired epoch/seq/PAUSED/count/token')
    require(w[39:43]==[2,1,1,0xfff],'rejections and complete expected-trace bitmap')
    expected=[e<<24|s<<16|status<<8|token for e,s,status,token in EXPECTED_TRACE]
    require(w[43:55]==expected,'epoch/seq/status/token sequence')
    require(w[55]==0 and w[56]==2000,'failure phase and proc0 post-service progress')

def runtime(w, layout):
    """Validate acquired record against reviewed linked layout, not guessed bounds."""
    protocol(w)
    require(w[:6]==[0x31523150,0x00400002,0,1,1,0x412fc231],'ABI and independent core identities')
    require(w[6:10]==[0x100030d0,0,0,0],'observed ROM entry MSP/PSP/CONTROL/IPSR tuple')
    require(w[10]==layout['stack_top'] and w[11]==layout['body_msp'],'installed stack and linked body frame')
    require(w[12:15]==[layout['vectors'],layout['stack_low'],layout['stack_top']],'private vector/stack bounds')
    require(w[15:19]==[0xa17ecafe]*4 and w[30]==1,'both canaries unchanged and complete guard check')
    require(w[19]==1 and w[20]>=13 and w[21:23]==[1,13],'finite proc0 work and twelve proc1 transactions advanced')
    require(w[23:25]==[0,0],'no latched fault')
    require(w[31:39]==[0x13579bdf,0x13579be7,0x13579beb,0x13579bed,
                       0x13579bdf,0,0x13579bec,1],'three work results and app data/BSS reinitialization')
    require(w[57:61]==[0xb007c0de,layout['stack_top'],layout['encoded_entry'],0],'known boot tuple and entry consumption')
    require(w[61]==layout['proc0_vtor'] and layout['proc0_stack_floor']<=w[62]<=0x2000f000,'proc0 private stack/vector retained')

def check(samples, layout):
    require(len(samples) == 31 and all(len(w) == 256 for w in samples), 'complete RT01 snapshots')
    previous = None
    for w in samples[5:]:
        p = w[96:160]
        require(p[63] == 0x31454e44, 'P1R1 worker not complete')
        runtime(p, layout)
        require(w[160] == 0 and w[161] & 3 == 2 and w[162] & 7 == 0, 'proc0 owner uses PSP/thread mode')
        require(0x20000000 <= w[162] < 0x2000e000 and not layout['stack_low'] <= w[162] < layout['stack_top'], 'owner stack separate from proc1')
        require(w[163] == p[62] and 0x2000e000 <= w[163] <= 0x2000f000, 'owner MSP retained')
        require(2000 <= (w[165]-w[164]) & 0xffffffff < 6500, 'kernel tick advanced during IPC')
        require(0 < (w[167]-w[166]) & 0xffffffff < 0x80000000, 'kernel switches advanced during IPC')
        require(w[168] > 0 and w[169] >= 32, 'owner alive with guarded task stack')
        require(2000000 <= (w[171]-w[170]) & 0xffffffff < 6500000, 'bounded observed service interval')
        require(0 < w[172] <= 2048-128, 'guarded proc1 stack high water')
        if previous:
            require(p == previous[96:160], 'immutable terminal receipt changed')
            require(0 < (w[168]-previous[168]) & 0xffffffff < 0x80000000, 'owner stopped after PAUSE')
        previous = w
    w = samples[-1]
    return dict(classification='HW', result='RP1_FREERTOS_PROC1_WORKER_SELECTED_PASS',
                requests=w[124], epochs=w[121], cooperative_paused=w[123] == 2,
                proc1_stack_used_bytes=w[172], owner_stack_free_words=w[169],
                owner_psp=hex(w[162]), proc1_body_msp=hex(w[107]),
                ticks_during_service=(w[165]-w[164]) & 0xffffffff,
                switches_during_service=(w[167]-w[166]) & 0xffffffff,
                service_interval_us=(w[171]-w[170]) & 0xffffffff,
                owner_progress=w[168], cross_core_rtos_objects=False,
                notification='SEV plus bounded 1-tick observations, not IRQ/FromISR',
                hardware_reset_restart='OPEN', proc1_fault_recovery='OPEN', R3='PARTIAL')

def selftest():
    # Synthetic words, never reported as hardware evidence.
    layout=dict(stack_top=0x20006000,stack_low=0x20005800,body_msp=0x20005fe4,
                vectors=0x20004000,encoded_entry=0x6ff80000,proc0_vtor=0x20000000,proc0_stack_floor=0x2000e000)
    p=[0]*64
    p[:6]=[0x31523150,0x00400002,0,1,1,0x412fc231]
    p[6:15]=[0x100030d0,0,0,0,layout['stack_top'],layout['body_msp'],layout['vectors'],layout['stack_low'],layout['stack_top']]
    p[15:19]=[0xa17ecafe]*4; p[19:23]=[1,26,1,13]
    p[25:30]=[3,3,2,12,12];p[30]=1
    p[31:39]=[0x13579bdf,0x13579be7,0x13579beb,0x13579bed,0x13579bdf,0,0x13579bec,1]
    p[39:43]=[2,1,1,0xfff]
    p[43:55]=[e<<24|s<<16|status<<8|token for e,s,status,token in EXPECTED_TRACE];p[56]=2000
    p[57:64]=[0xb007c0de,layout['stack_top'],layout['encoded_entry'],0,0x20000000,0x2000f000,0x31454e44]
    w=[0]*256;w[96:160]=p
    w[160:173]=[0,2,0x20009000,0x2000f000,2000,5200,200,900,1,90,2000000,5200000,28]
    samples=[]
    for n in range(31):
        row=w.copy();row[168]=n+1;samples.append(row)
    check(samples,layout)
    failures=0
    def refused(rows):
        nonlocal failures
        try:check(rows,layout)
        except ValueError:failures+=1
        else:raise AssertionError('corrupt receipt accepted')
    for i in list(range(2,19))+[23,24]+list(range(25,43))+list(range(43,62))+[63]:
        bad=[r.copy() for r in samples];bad[-1][96+i]^=1;refused(bad)
    for i,v in [(115,0),(116,1),(158,0x2000d000),(160,1),(161,0),(162,0x20005804),(163,0),
                (165,2000),(167,200),(168,1),(169,0),(171,2000000),(172,2048)]:
        bad=[r.copy() for r in samples];bad[-1][i]=v;refused(bad)
    refused(samples[:-1])
    # Modular timestamp deltas remain valid across tick/raw-low wrap.
    wrap=[r.copy() for r in samples]
    for r in wrap:r[164:166]=[0xfffffff0,3184];r[170:172]=[0xfffffff0,3199984]
    check(wrap,layout)
    print(f'BUILD synthetic proc1 validator: positive2 negative{failures}; HW OPEN')

if __name__ == '__main__':
    selftest()
