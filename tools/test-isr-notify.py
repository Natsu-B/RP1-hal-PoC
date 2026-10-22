#!/usr/bin/env python3
"""Compile actual ISR wrapper: retain woken result and request matching yield."""
from pathlib import Path
import subprocess
import tempfile

root=Path(__file__).resolve().parents[1]
source=(root/'crates/rp1-freertos/c/bridge.c').read_text()
body='int32_t rp1_freertos_notification_give_from_isr('+source.split('int32_t rp1_freertos_notification_give_from_isr(',1)[1].split('\n}',1)[0]+'\n}\n'
stub=r'''
#include <stdint.h>
#include <stddef.h>
#include <assert.h>
typedef int BaseType_t;
typedef void *TaskHandle_t;
#define pdFALSE 0
#define INVALID -1
static int context, awake, notified, yielded;
static int isr_context(void) { return context; }
static TaskHandle_t task_handle(uint32_t id) { return id==8 ? (void*)8 : NULL; }
static void vTaskNotifyGiveFromISR(TaskHandle_t h,BaseType_t *w) {
    assert(h==(void*)8); notified++; *w=awake;
}
#define portYIELD_FROM_ISR(w) (yielded=(w))
'''
tests=r'''
int main(void) {
    assert(rp1_freertos_notification_give_from_isr(8)==0 && notified==1 && yielded==0);
    awake=1;
    assert(rp1_freertos_notification_give_from_isr(8)==1 && notified==2 && yielded==1);
    assert(rp1_freertos_notification_give_from_isr(0)==INVALID && notified==2);
    context=-5;
    assert(rp1_freertos_notification_give_from_isr(8)==-5 && notified==2);
}
'''
with tempfile.TemporaryDirectory(prefix='rp1-notify-') as directory:
    path=Path(directory)/'check.c';path.write_text(stub+body+tests)
    binary=Path(directory)/'check'
    subprocess.run(['cc','-std=c11','-O2','-Wall','-Wextra','-Werror',str(path),'-o',str(binary)],check=True)
    subprocess.run([str(binary)],check=True)
print('STATIC: production ISR notification/yield wrapper, 4 checks PASS; no hardware')
