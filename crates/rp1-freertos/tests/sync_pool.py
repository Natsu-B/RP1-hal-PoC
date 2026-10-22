#!/usr/bin/env python3
"""Host-only pool/guard regression; no kernel execution or target build."""
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
bridge = (ROOT / 'c/bridge.c').read_text()


def function(signature):
    begin = bridge.index(signature)
    brace = bridge.index('{', begin)
    depth = 0
    for end in range(brace, len(bridge)):
        depth += (bridge[end] == '{') - (bridge[end] == '}')
        if depth == 0:
            return bridge[begin:end + 1]
    raise AssertionError('unterminated function')


program = r'''
#include <assert.h>
#include <stddef.h>
#include <stdint.h>
typedef int StaticQueue_t, StaticSemaphore_t;
typedef int *QueueHandle_t, *SemaphoreHandle_t;
''' + bridge[bridge.index('enum { TASKS'):bridge.index('_Static_assert')] + r'''
_Static_assert(TASKS == 8 && STACK_WORDS == 512, "task limits unchanged");
_Static_assert(QUEUES == EXPECT_QUEUES && QUEUE_WORDS == EXPECT_WORDS &&
               SEMAPHORES == EXPECT_SEMAPHORES, "C pool contract");
''' + bridge[bridge.index('static struct {\n    StaticQueue_t control;'):
             bridge.index('static uint32_t started;')] + r'''
static int32_t context;
static uint32_t capacity_seen, calls, unavailable;
static int32_t create_context(void) { return context; }
static QueueHandle_t xQueueCreateStatic(uint32_t capacity, uint32_t size,
                                        uint8_t *data, StaticQueue_t *control) {
    assert(size == sizeof(uint32_t) && data != NULL);
    capacity_seen = capacity; ++calls;
    return unavailable ? NULL : control;
}
static SemaphoreHandle_t xSemaphoreCreateBinaryStatic(StaticSemaphore_t *control) {
    ++calls; return unavailable ? NULL : control;
}
#define xSemaphoreCreateMutexStatic xSemaphoreCreateBinaryStatic
''' + '\n'.join(function(signature) for signature in (
    'static QueueHandle_t queue_handle(',
    'static SemaphoreHandle_t semaphore_handle(',
    'int32_t rp1_freertos_queue_create(',
    'int32_t rp1_freertos_semaphore_create(',
)) + r'''
int main(void) {
    context = CONTEXT;
    assert(rp1_freertos_queue_create(0, 0) == CONTEXT);
    assert(rp1_freertos_semaphore_create(2, 0) == CONTEXT);
    context = STATE;
    assert(rp1_freertos_queue_create(0, 1) == STATE);
    assert(rp1_freertos_semaphore_create(0, 0) == STATE);
    context = 0;
    assert(rp1_freertos_queue_create(QUEUES, 1) == INVALID);
    assert(rp1_freertos_queue_create(0, 0) == INVALID);
    assert(rp1_freertos_queue_create(0, QUEUE_WORDS + 1) == INVALID);
    assert(rp1_freertos_queue_create(0, UINT32_MAX) == INVALID);
    assert(rp1_freertos_semaphore_create(2, 0) == INVALID);
    assert(rp1_freertos_semaphore_create(BINARY, SEMAPHORES) == INVALID);
    assert(rp1_freertos_semaphore_create(MUTEX, SEMAPHORES) == INVALID);
    assert(calls == 0);
    unavailable = 1;
    assert(rp1_freertos_queue_create(0, 4) == UNAVAILABLE);
    assert(rp1_freertos_semaphore_create(BINARY, 0) == UNAVAILABLE);
    unavailable = 0;
    for (uint32_t slot = 0; slot < QUEUES; ++slot) {
        uint32_t capacity = slot == 1 ? 1 : QUEUE_WORDS;
        assert(queue_handle(slot + 1) == NULL);
        assert(rp1_freertos_queue_create(slot, capacity) == (int32_t)(slot + 1));
        assert(capacity_seen == capacity); /* Actual requested depth is not reduced. */
        assert(queue_handle(slot + 1) != NULL);
        assert(rp1_freertos_queue_create(slot, capacity) == OCCUPIED);
    }
    assert(queue_handle(0) == NULL && queue_handle(QUEUES + 1) == NULL);
    for (uint32_t kind = BINARY; kind <= MUTEX; ++kind) {
        for (uint32_t slot = 0; slot < SEMAPHORES; ++slot) {
            assert(semaphore_handle(kind, slot + 1) == NULL);
            assert(rp1_freertos_semaphore_create(kind, slot) == (int32_t)(slot + 1));
            assert(semaphore_handle(kind, slot + 1) != NULL);
            assert(rp1_freertos_semaphore_create(kind, slot) == OCCUPIED);
        }
        assert(semaphore_handle(kind, 0) == NULL);
        assert(semaphore_handle(kind, SEMAPHORES + 1) == NULL);
    }
    assert(semaphore_handle(2, 1) == NULL);
}
'''

with tempfile.TemporaryDirectory(prefix='rp1-sync-pool-') as tmp:
    source = Path(tmp) / 'sync_pool.c'
    source.write_text(program)
    binary = str(Path(tmp) / 'sync_pool')
    for compact, limits in ((False, (4, 16, 4)), (True, (2, 4, 1))):
        cflags = ['-DRP1_FREERTOS_SYNC_POOL_R1=1'] if compact else []
        cflags += [f'-DEXPECT_{name}={value}' for name, value in
                   zip(('QUEUES', 'WORDS', 'SEMAPHORES'), limits)]
        subprocess.run(['cc', '-std=c11', '-Os', '-Wall', '-Wextra', '-Werror',
                        *cflags, str(source), '-o', binary], check=True)
        subprocess.run([binary], check=True)
        for compact_tasks in (False, True):
            cfg = ['--cfg', 'feature="sync-pool-r1"'] if compact else []
            if compact_tasks:
                cfg += ['--cfg', 'feature="task-pool-2304"']
            subprocess.run(['rustc', '--edition=2024', '--test', *cfg,
                            str(ROOT / 'src/lib.rs'), '-o', binary], check=True, cwd=ROOT)
            subprocess.run([binary], check=True)
        print(f'STATIC PASS: sync-pool-r1={compact}; C/Rust limits={limits}; '
              'actual C create/handle guards, capacity, errors; both Rust task-pool modes', flush=True)
print('No target build, kernel scheduling, SRAM-fit or hardware proof.')
