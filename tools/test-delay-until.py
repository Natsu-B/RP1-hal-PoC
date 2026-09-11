#!/usr/bin/env python3
"""Compile the actual bridge and pinned kernel function with scheduler mocks.

Checks boundary/arithmetic/control flow, NOT target context switches or time.
"""
from pathlib import Path
import subprocess
import tempfile

if not __debug__:
    raise SystemExit('tests require assertions')
ROOT = Path(__file__).resolve().parents[1]

def function(path, signature):
    text = path.read_text(); begin = text.index(signature); brace = text.index('{', begin)
    depth = 0
    for end in range(brace, len(text)):
        depth += (text[end] == '{') - (text[end] == '}')
        if depth == 0:
            return text[begin:end+1]
    raise AssertionError('unterminated function')

kernel = function(ROOT/'third-party/FreeRTOS-Kernel/tasks.c', 'BaseType_t xTaskDelayUntil(')
bridge = function(ROOT/'crates/rp1-freertos/c/bridge.c', 'int32_t rp1_freertos_delay_until(')
program = r'''
#include <assert.h>
#include <stddef.h>
#include <stdint.h>
typedef uint32_t TickType_t;
typedef int32_t BaseType_t;
enum { pdFALSE=0, pdTRUE=1, INVALID=-1 };
static uint32_t xTickCount, uxSchedulerSuspended, blocked, yields, calls;
static int32_t context, resume_result;
static int32_t thread_context(uint32_t running) { assert(running==1); return context; }
#define configASSERT(x) assert(x)
#define traceENTER_xTaskDelayUntil(a,b) ((void)0)
#define traceRETURN_xTaskDelayUntil(x) ((void)0)
#define traceTASK_DELAY_UNTIL(x) ((void)0)
#define mtCOVERAGE_TEST_MARKER() ((void)0)
#define taskYIELD_WITHIN_API() (++yields)
static void vTaskSuspendAll(void) { ++uxSchedulerSuspended; ++calls; }
static BaseType_t xTaskResumeAll(void) { --uxSchedulerSuspended; return resume_result; }
static void prvAddCurrentTaskToDelayedList(TickType_t ticks, BaseType_t indefinitely) {
    assert(ticks && !indefinitely); blocked=ticks;
}
''' + kernel + '\n' + bridge + r'''
static void check(uint32_t previous, uint32_t now, uint32_t period,
                  int32_t result, uint32_t next, uint32_t delay) {
    xTickCount=now; blocked=yields=calls=0;
    assert(rp1_freertos_delay_until(&previous,period)==result);
    assert(previous==next && blocked==delay && !uxSchedulerSuspended);
    assert(calls==1 && yields==(uint32_t)!resume_result);
}
int main(void) {
    uint32_t previous=123;
    context=-5; assert(rp1_freertos_delay_until(&previous,10)==-5 && previous==123);
    context=-2; assert(rp1_freertos_delay_until(&previous,10)==-2 && previous==123);
    context=0;
    assert(rp1_freertos_delay_until(NULL,10)==INVALID);
    assert(rp1_freertos_delay_until(&previous,0)==INVALID && previous==123);
    assert(rp1_freertos_delay_until(&previous,0x80000000)==INVALID && previous==123);
    assert(rp1_freertos_delay_until(&previous,UINT32_MAX)==INVALID && previous==123);
    assert(calls==0);
    check(100,110,20,1,120,10);
    check(100,120,20,0,120,0);
    check(100,121,20,0,120,0);
    check(0xfffffff0,0xfffffff5,32,1,16,27);
    check(0xfffffff0,5,32,1,16,11);
    check(0xfffffff0,16,32,0,16,0);
    check(0xfffffff0,32,32,0,16,0);
    check(0,0,0x7fffffff,1,0x7fffffff,0x7fffffff);
    resume_result=1; check(100,110,20,1,120,10);
}
'''
with tempfile.TemporaryDirectory(prefix='rp1-delay-until-', dir='/dev/shm') as tmp:
    source=Path(tmp)/'test.c'; source.write_text(program)
    binary=str(Path(tmp)/'test')
    subprocess.run(['cc','-std=c11','-Os','-Wall','-Wextra','-Werror',str(source),'-o',binary],check=True)
    subprocess.run([binary],check=True)
print('STATIC PASS: actual bridge and official delay-until body; context/invalid/due/past/wrap/resume; scheduling HW OPEN')
