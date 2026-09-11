#!/usr/bin/env python3
"""Compile the actual bridge transaction with a tiny deterministic kernel stub.

STATIC only: no claim about RP1 interrupt latency or hardware cancellation.
"""
from pathlib import Path
import subprocess
import tempfile
import re

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

# Execute the actual owner-side settlement blocks, not a second model of them.
# Structural checks bind them to the generation withdrawal in all three drivers.
drivers = {name: (repo/f'crates/rp1-freertos/src/{name}.rs').read_text()
           for name in ['spi0', 'i2c1', 'uart0']}
settlements = []
for name, text in drivers.items():
    closed = text.index('ptr::addr_of_mut!(GENERATION).write_volatile(0);')
    pattern = r'if result\.is_ok\(\) && unsafe \{ ptr::addr_of!\(CANCEL\)\.read_volatile\(\) \} == generation \{\s*result = Err\(Error::Cancelled\);\s*\}'
    blocks = list(re.finditer(pattern, text))
    assert len(blocks) == 1, name
    block = blocks[0]
    assert closed < block.start() < text.index('barrier();', closed), name
    settlements.append(f'fn {name}(mut result: Result<(), Error>, generation: u32) -> Result<(), Error> {{\n'
                       + block.group() + '\nresult\n}')
spi = drivers['spi0']
generation = re.search(r'let generation = self\.generation\.checked_add\(1\)\.expect\("[^"]*"\);', spi).group()
assert spi.index(generation) < spi.index('let deadline =') < spi.index('self.host.prepare_irq_transfer(')
assert 'wrapping_add(1).max(1)' not in spi
generation = generation.replace('self.generation', 'previous')
i2c = drivers['i2c1']
fatal = i2c[i2c.index('if matches!(result, Ok(()) | Err(Error::Cancelled))'):i2c.index('let bytes=c.engine.bytes();')]
rust = r'''
use std::ptr;
static mut CANCEL: u32 = 0;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RxError { Fatal { causes: u32, abort_source: u32 } }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Error { Cancelled, Timeout, Receive(RxError) }
struct Receipt { first_fatal_causes: u32, first_abort_source: u32 }
struct Context { receipt: Receipt }
''' + '\n'.join(settlements) + '\nfn next(previous: u32) -> u32 {\n' + generation + '\ngeneration\n}\n' + \
    'fn fatal(mut result: Result<(), Error>, c: Context) -> Result<(), Error> {\n' + fatal + '\nresult\n}\n' + r'''
fn main() {
    let prior = Error::Receive(RxError::Fatal { causes: 8, abort_source: 9 });
    let states = [Ok(()), Err(Error::Cancelled), Err(Error::Timeout), Err(prior)];
    let mut checks = 0;
    for settle in [spi0, i2c1, uart0] {
        for state in states {
            // Cancel committed before withdrawal versus rejected/stale ticket.
            for cancel in [0, 10, 11] {
                unsafe { ptr::addr_of_mut!(CANCEL).write_volatile(cancel); }
                let expected = if state.is_ok() && cancel == 11 { Err(Error::Cancelled) } else { state };
                assert_eq!(settle(state, 11), expected); checks += 1;
            }
        }
    }
    for state in states {
        for (causes, abort_source) in [(0,0), (0x40,0), (0,0x800001)] {
            let expected = if matches!(state, Ok(()) | Err(Error::Cancelled)) && (causes != 0 || abort_source != 0) {
                Err(Error::Receive(RxError::Fatal { causes, abort_source }))
            } else { state };
            assert_eq!(fatal(state, Context { receipt: Receipt {
                first_fatal_causes: causes, first_abort_source: abort_source } }), expected);
            checks += 1;
        }
    }
    assert_eq!(next(0), 1); assert_eq!(next(u32::MAX-1), u32::MAX);
    std::panic::set_hook(Box::new(|_| {}));
    assert!(std::panic::catch_unwind(|| next(u32::MAX)).is_err()); checks += 3;
    println!("STATIC: production Rust settlement/fatal/generation blocks, {checks} checks PASS; no hardware");
}
'''
with tempfile.TemporaryDirectory(prefix='rp1-cancel-settlement-') as directory:
    path = Path(directory)/'check.rs'
    path.write_text(rust)
    binary = Path(directory)/'check'
    subprocess.run(['rustc','--edition=2024','-O','-Dwarnings',str(path),'-o',str(binary)],check=True)
    subprocess.run([str(binary)],check=True)
