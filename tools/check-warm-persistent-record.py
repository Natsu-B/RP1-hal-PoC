#!/usr/bin/env python3
"""Decode two WQLATE samples of the BC01 completed eight-generation schema, not whole HW admission.

Caller must separately join image/source identity, current nonce, full type8,
external peer, host-observed pre-read completion and recovery. Individual volatile
words/identity brackets are not an atomic 256-word snapshot.
"""
import json
from pathlib import Path
import re
import sys

assert __debug__
IDENTITY = [0,1,2,3,4,96,97,98,99,100,124,125]
STACK_WORDS = [512,128,128,256,256,256,256,512]
HWM = [32,33,34,35,36,37,38,160]
PROGRESS = [8,9,14,15,17,49,50,64,80,162]
IPSRS = [35,41,24]
LIMITS = [5,84,132]

def require(ok, message):
    if not ok:
        raise ValueError(message)

def delta(a, b):
    value = (b-a) & 0xffffffff
    require(0 < value < 0x80000000, 'stopped/reversed/ambiguous progress')
    return value

def receipt_indices(kind):
    return list(range(126+8*kind,134+8*kind))

def payload(kind, generation):
    require(kind in (0,1,2) and 1 <= generation <= 8, 'payload identity')
    if kind == 0:
        return bytes([0x69,0x96,0x3c,(generation-1)%2+1])
    if kind == 1:
        return f'HOST2RP1 IRQ {generation:04d}\r\n'.encode('ascii')
    frame = generation-1
    return bytes((frame^0x31 if i == 0 else (frame>>8)^0x4e if i == 1
                  else (0xb4+i*0x1d+frame*7)&255) for i in range(2 if generation%2 else 31))

def digest(data):
    value=0x811c9dc5
    for byte in data:
        value=((value^byte)*0x01000193)&0xffffffff
    return value

def check_words(w, nonce, warm_len, warm_digest):
    require(len(w) == 256 and all(type(v) is int and 0 <= v <= 0xffffffff for v in w), 'word shape')
    require(w[:5] == [0x31305452,1,5,0,0], 'RT01/stage/fault')
    require(w[96:101] == [0x384e524b,nonce,1,warm_len,warm_digest], 'warm epoch/reason/data identity')
    require(w[124:126] == [int.from_bytes(b'BC01','little'),8], 'BC01/incomplete owner')
    require(w[150] > 0 and w[150]%2 == 0 and w[151:153] == [8,8] and w[154] == 24,
            'completed coherent owner publication')
    require(w[159] == w[161] == 1 and w[163] == w[150], 'type8 handback/health admission')
    require(0 < (w[165]-w[164])&0xffffffff <= 80_000, 'bounded service interval')
    require((w[165]-w[155])&0xffffffff <= 1000, 'last completion before finish')
    require((w[162]-w[165])&0xffffffff < 0x80000000, 'health predates completed owner')
    require((w[8]-w[162])&0xffffffff <= 2000, 'stale monitor health')
    require(not any(w[166:169]), 'BC reserved words')
    require(w[19] == 0x13579bdf and w[20] == 0 and w[39] == 1, 'startup/sync sentinels')
    require(not any(w[i] for i in [56,57,58,59,70,86,183]), 'fault/context error')
    # RFT1 publishes its body before magic192; reject partial fault publication too.
    require(not any(w[105:108]+w[184:256]), 'warm reserved/fault publication')
    require(w[14] >= 5 and w[8] >= 5000 and w[9] >= 5000, 'warm progress gate')
    require(w[101] >= 5000 and w[102] >= 5000 and w[103] >= 20 and w[104] >= 10, 'latched warm gate')
    require(0 < w[18] <= 3968 and w[18] % 4 == 0, 'MSP watermark')
    for i, capacity in zip(HWM, STACK_WORDS):
        require(32 <= w[i] <= capacity, 'task watermark')
    contexts = [w[28:32], w[65:69], w[81:85], w[175:178]]
    for c in contexts:
        require(c[0] == 0 and c[1] & 3 == 2 and c[2] % 8 == 0 and 0x20000000 <= c[2] < 0x2000e000, 'task IPSR/CONTROL/PSP')
        if len(c) == 4:
            require(c[3] % 8 == 0 and 0x2000e000 <= c[3] <= 0x2000f000, 'MSP range')
    require(len({c[2] for c in contexts}) == len(contexts), 'distinct PSPs')
    receipts = []
    for k in range(3):
        raw=[w[i] for i in receipt_indices(k)]
        require(raw[0] == 8, 'rolling receipt generation')
        r=raw[1:]; expected=payload(k,8)
        require(r[0] == 0xa5000000 | k<<16 | len(expected)<<8, 'receipt header without short generation')
        require(r[1] == digest(expected), 'receipt payload digest')
        require(0 < r[2] <= LIMITS[k] and r[3] & 255 == IPSRS[k] and r[3] >> 8 <= r[2] and r[4] > 0, 'receipt IRQ/IPSR/time')
        if k == 0:
            require(r[3] >> 8 == 0 and r[5] <= r[4] and r[6] <= r[4], 'SPI compact bounds')
        elif k == 1:
            require(r[3] >> 8 > 0 and r[5] >= 8000 and 4 <= r[6] <= 0xffff, 'UART cleanup')
        else:
            require(r[5] >= 4000 and r[6] & 0xffff >= 2 and r[6] >> 16 < 10000, 'I2C cleanup')
        require(8 <= w[156+k] <= 8*LIMITS[k] and w[156+k] >= r[2]+7, 'cumulative checked IRQ bound')
        receipts.append(raw)
    require(w[169:172] == w[156:159], 'quiet direct/checked IRQ totals')
    require(w[172:175] == IPSRS, 'direct IRQ IPSRs')
    require(12000 <= w[178] <= 14000 and 28000 <= w[179] <= 30000, 'owner pulse range')
    return receipts

def validate(text, nonce, warm_len, warm_digest):
    require(0 < nonce <= 0xffff and 0 < warm_len <= 0x10000 and 0 <= warm_digest <= 0xffffffff, 'expected identity')
    require('observer-quiesced no-more-rp1-access=1' not in text, 'old no-access claim')
    lines = []
    for line in text.splitlines():
        if '[WQLATE]' in line:
            require(line.count('[WQLATE] ') == 1, 'marker framing')
            lines.append(line.split('[WQLATE] ',1)[1])
    at = 0
    def take(pattern):
        nonlocal at
        require(at < len(lines), 'missing WQLATE row')
        m = re.fullmatch(pattern, lines[at]); at += 1
        require(m is not None, 'malformed/out-of-order WQLATE row')
        return m.groups()
    n, hz, ack = map(int, take(r'armed version=1 nonce=(\d+) counter_hz=(\d+) ack_counter=(\d+) delays_s=120,122 addr=0x2000f800 bytes=1024 samples=2 bracket_words=12 post-ack-rp1-read=1 post-ack-rp1-write=0 no-reinit=1'))
    # Selected commissioning explicitly excludes wrapping the local u64 clock.
    require(n == nonce and 0 < hz <= 0xffffffff and 0 <= ack < 2**64-hz*124, 'observer identity/clock/no-wrap')
    samples = []
    for sample in range(2):
        deadline, begin = map(int, take(rf'sample={sample} BEGIN deadline_counter=(\d+) begin_counter=(\d+)'))
        require(deadline == ack+hz*(120+2*sample) and deadline <= begin < deadline+hz, 'absolute schedule')
        before = [int(x,16) for x in take(rf'sample={sample} identity-before ((?:[0-9a-f]{{8}} ){{11}}[0-9a-f]{{8}})')[0].split()]
        words = []
        for row in range(0,256,4):
            words += [int(x,16) for x in take(rf'sample={sample} words {row:03} ((?:[0-9a-f]{{8}} ){{3}}[0-9a-f]{{8}})')[0].split()]
        after = [int(x,16) for x in take(rf'sample={sample} identity-after ((?:[0-9a-f]{{8}} ){{11}}[0-9a-f]{{8}})')[0].split()]
        read_begin, read_end = map(int, take(rf'sample={sample} END read_begin_counter=(\d+) read_end_counter=(\d+)'))
        require(begin <= read_begin <= read_end < begin+hz, 'read interval')
        require(before == after == [words[i] for i in IDENTITY], 'identity bracket change')
        receipts = check_words(words, nonce, warm_len, warm_digest)
        samples.append(dict(words=words, begin_counter=begin, read_begin_counter=read_begin,
            read_end_counter=read_end, receipts=receipts, msp_used_bytes=words[18],
            task_stack_free_words=[words[i] for i in HWM],
            task_stack_used_bytes=[4*(size-words[i]) for i,size in zip(HWM,STACK_WORDS)]))
    take(r'observer-complete samples=2 post-ack-rp1-read=1 post-ack-rp1-write=0 no-reinit=1')
    require(at == len(lines), 'duplicate/trailing WQLATE rows')
    a, b = samples
    require(a['read_end_counter'] < b['begin_counter'], 'overlapping reads')
    stable = IDENTITY + list(range(96,108)) + list(range(124,162)) + list(range(163,184))
    require(all(a['words'][i] == b['words'][i] for i in stable), 'warm identity/receipt/quiet IRQ changed')
    require(all(x >= y for x,y in zip(a['task_stack_free_words'],b['task_stack_free_words'])), 'minimum-free HWM increased')
    require(b['msp_used_bytes'] >= a['msp_used_bytes'], 'MSP used decreased')
    progress = {str(i):delta(a['words'][i],b['words'][i]) for i in PROGRESS}
    # Compare old publication to the later complete copy, not simultaneity of
    # volatile live fields within one copy. Publication fields are stable above.
    for latched, live in [(101,8),(102,9),(103,49),(104,50)]:
        require((b['words'][live]-a['words'][latched]) & 0xffffffff < 0x80000000, 'latched counter ahead of later live progress')
    return dict(result='WARM_EIGHT_GENERATION_RECORD_VALID', classification='OBSERVATION_ONLY',
        hardware_acceptance=False, nonce=nonce, counter_hz=hz, ack_counter=ack,
        sample_gap_us=(b['read_begin_counter']-a['read_begin_counter'])*1e6/hz,
        progress_deltas=progress, samples=samples, atomic_snapshot_proven=False,
        boundary='Only terminal generation8 quiet copies, not in-flight atomicity/health feeding/30-minute service. Needs same-run image/type8/peer/host-observation-order/recovery joins. Returned fixed BAR2 samples only; not uninterrupted link or full R1/R2/R3 admission.')

if __name__ == '__main__':
    require(len(sys.argv)==5,'usage: UART nonce warm-data-bytes warm-data-fnv1a')
    print(json.dumps(validate(Path(sys.argv[1]).read_text(),*(int(v,0) for v in sys.argv[2:])),indent=2))
