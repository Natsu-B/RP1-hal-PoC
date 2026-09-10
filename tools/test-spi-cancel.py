#!/usr/bin/env python3
"""Compile the actual bridge transaction with a tiny deterministic kernel stub.

STATIC only: no claim about RP1 interrupt latency or hardware cancellation.
"""
from pathlib import Path
import subprocess
import tempfile

repo = Path(__file__).resolve().parents[1]
source = (repo/'crates/rp1-freertos/c/bridge.c').read_text()
body = source.split('int32_t rp1_freertos_cancel_notification(', 1)[1]
body = 'int32_t rp1_freertos_cancel_notification(' + body.split('\nint32_t rp1_freertos_notification_give_from_isr', 1)[0]
prefix = r'''
#include <stdint.h>
#include <stddef.h>
#include <assert.h>
typedef void *TaskHandle_t;
#define pdPASS 1
enum { INVALID=-1, UNAVAILABLE=-4 };
static int context, locked, enters, exits, notices, rearm;
static volatile uint32_t active, cancelled, waiter;
static int32_t thread_context(uint32_t run) { assert(run==1); return context; }
static TaskHandle_t task_handle(uint32_t id) { return (void *)(uintptr_t)(id>=1 && id<=8 ? id : 0); }
static void enter(void) {
    assert(!locked);
    /* Simulate owner rearm and a newer cancel immediately before acquiring. */
    if (rearm) { active=12; cancelled=12; }
    locked=1; enters++;
}
static void leave(void) { assert(locked); locked=0; exits++; }
#define taskENTER_CRITICAL() enter()
#define taskEXIT_CRITICAL() leave()
static int xTaskNotifyGive(TaskHandle_t task) {
    assert(locked && (uintptr_t)task==waiter && cancelled==active); notices++; return pdPASS;
}
'''
tests = r'''
static void init(void) {
    assert(!locked); context=enters=exits=notices=rearm=0;
    active=11; cancelled=0; waiter=8;
}
int main(void) {
    init(); assert(rp1_freertos_cancel_notification(11,&active,&cancelled,&waiter)==1);
    assert(cancelled==11 && notices==1 && enters==1 && exits==1);
    init(); rearm=1;
    assert(rp1_freertos_cancel_notification(11,&active,&cancelled,&waiter)==0);
    assert(cancelled==12 && notices==0 && enters==1 && exits==1);
    init(); assert(rp1_freertos_cancel_notification(0,&active,&cancelled,&waiter)==0);
    assert(!enters && !notices && !cancelled);
    init(); active=0; assert(rp1_freertos_cancel_notification(11,&active,&cancelled,&waiter)==0);
    assert(!notices && !cancelled && enters==exits);
    init(); waiter=0; assert(rp1_freertos_cancel_notification(11,&active,&cancelled,&waiter)==INVALID);
    assert(!notices && !cancelled && enters==exits);
    init(); context=-5; assert(rp1_freertos_cancel_notification(11,&active,&cancelled,&waiter)==-5);
    assert(!enters && !notices);
    init(); assert(rp1_freertos_cancel_notification(11,0,&cancelled,&waiter)==INVALID);
    assert(!enters && !notices);
    return 0;
}
'''
with tempfile.TemporaryDirectory(prefix='rp1-spi-cancel-') as directory:
    path = Path(directory)/'check.c'
    path.write_text(prefix + body + tests)
    binary = Path(directory)/'check'
    subprocess.run(['cc','-std=c11','-O2','-Wall','-Wextra','-Werror',str(path),'-o',str(binary)],check=True)
    subprocess.run([str(binary)],check=True)
print('STATIC: production C cancellation transaction, 7 checks PASS; no hardware')
