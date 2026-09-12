#include "FreeRTOS.h"
#include "task.h"
#include "queue.h"
#include "semphr.h"
#include <stddef.h>
#include <stdint.h>
#include "static_stack.h"

enum { TASKS = 8, STACK_WORDS = 512, QUEUES = 4, QUEUE_WORDS = 16, SEMAPHORES = 4 };
#ifdef RP1_FREERTOS_TASK_POOL_2304
/* AY: seven unchanged R1 stacks1792 + unchanged owner512; no spare task slots. */
enum { TASK_STACK_POOL_WORDS = 2304 };
#else
enum { TASK_STACK_POOL_WORDS = 2560 };
#endif
enum { INVALID = -1, STATE = -2, OCCUPIED = -3, UNAVAILABLE = -4, CONTEXT = -5 };
enum { BINARY = 0, MUTEX = 1 };

_Static_assert(sizeof(TickType_t) == 4, "32-bit ticks required");
_Static_assert(sizeof(StackType_t) == 4, "32-bit stack words required");
_Static_assert(sizeof(void *) == 4, "32-bit target required");

/* ponytail: finite lifetime pools; add deletion/reuse only with a real lifecycle requirement. */
static struct {
    StaticTask_t tcb;
    TaskHandle_t handle;
} tasks[TASKS];
static StackType_t task_stacks[TASK_STACK_POOL_WORDS] __attribute__((aligned(8)));
static uint32_t task_stack_used;
static StaticTask_t idle_tcb;
static StackType_t idle_stack[configMINIMAL_STACK_SIZE] __attribute__((aligned(8)));
static struct {
    StaticQueue_t control;
    uint32_t data[QUEUE_WORDS];
    QueueHandle_t handle;
} queues[QUEUES];
static struct {
    StaticSemaphore_t control;
    SemaphoreHandle_t handle;
} semaphores[2][SEMAPHORES];
static uint32_t started;
static uint32_t task_count;
uint32_t rp1_freertos_cpu_hz;

__attribute__((weak)) void rp1_freertos_tick_hook(void) {}
__attribute__((weak)) void rp1_freertos_switch_hook(uint32_t task_id) { (void)task_id; }
__attribute__((weak, noreturn)) void rp1_freertos_fault_hook(uint32_t reason, uint32_t detail)
{
    (void)reason;
    (void)detail;
    __asm volatile("cpsid i" ::: "memory");
    for (;;) { __asm volatile("wfi"); }
}

static uint32_t task_id(TaskHandle_t handle)
{
    for (uint32_t i = 0; i < TASKS; ++i) {
        if (tasks[i].handle != NULL && tasks[i].handle == handle) return i + 1;
    }
    if (handle != NULL && handle == xTaskGetIdleTaskHandle()) return UINT32_MAX;
    return 0;
}

void rp1_freertos_trace_switch(void *task) { rp1_freertos_switch_hook(task_id(task)); }
void vApplicationTickHook(void) { rp1_freertos_tick_hook(); }
void rp1_freertos_assert(uint32_t line) { rp1_freertos_fault_hook(1, line); }
#ifdef RP1_FREERTOS_ASSERT_PROBE
/* Deliberate official-kernel assertion, not the Rust API argument guard.
 * If configASSERT were disabled/returned, record a distinct failure instead. */
__attribute__((noreturn)) void rp1_freertos_config_assert_probe(void)
{
    vTaskPrioritySet(NULL, configMAX_PRIORITIES);
    rp1_freertos_fault_hook(5, 0xa551);
}
#endif
void vApplicationStackOverflowHook(TaskHandle_t task, char *name)
{
    (void)name;
    rp1_freertos_fault_hook(2, task_id(task));
}
void vApplicationGetIdleTaskMemory(StaticTask_t **tcb, StackType_t **stack,
                                 configSTACK_DEPTH_TYPE *words)
{
    *tcb = &idle_tcb;
    *stack = idle_stack;
    *words = configMINIMAL_STACK_SIZE;
}

static uint32_t exception_number(void)
{
    uint32_t ipsr;
    __asm volatile("mrs %0, ipsr" : "=r"(ipsr));
    return ipsr;
}

static int32_t thread_context(uint32_t must_run)
{
    if (exception_number() != 0) return CONTEXT;
    if (must_run && !started) return STATE;
    return 0;
}

static int32_t create_context(void)
{
    int32_t result = thread_context(0);
    if (result != 0) return result;
    return started ? STATE : 0;
}

static int32_t isr_context(void)
{
    /* SysTick is reserved to the official port; only external IRQs call this API. */
    if (exception_number() < 16) return CONTEXT;
    if (!started) return STATE;
    portASSERT_IF_INTERRUPT_PRIORITY_INVALID();
    return 0;
}

static TaskHandle_t task_handle(uint32_t id)
{
    return id >= 1 && id <= TASKS ? tasks[id - 1].handle : NULL;
}

static QueueHandle_t queue_handle(uint32_t id)
{
    return id >= 1 && id <= QUEUES ? queues[id - 1].handle : NULL;
}

static SemaphoreHandle_t semaphore_handle(uint32_t kind, uint32_t id)
{
    return kind <= MUTEX && id >= 1 && id <= SEMAPHORES ? semaphores[kind][id - 1].handle : NULL;
}

int32_t rp1_freertos_task_create(uint32_t slot, const char *name, TaskFunction_t entry,
                               void *argument, uint32_t priority, uint32_t stack_words)
{
    int32_t result = create_context();
    if (result != 0) return result;
    if (slot >= TASKS || name == NULL || entry == NULL || priority >= configMAX_PRIORITIES ||
        stack_words < configMINIMAL_STACK_SIZE || stack_words > STACK_WORDS) return INVALID;
    uint32_t length = 0;
    while (length < configMAX_TASK_NAME_LEN && name[length] != '\0') ++length;
    if (length == 0 || length == configMAX_TASK_NAME_LEN) return INVALID;
    if (tasks[slot].handle != NULL) return OCCUPIED;
    uint32_t next_stack = rp1_stack_next(task_stack_used, stack_words, TASK_STACK_POOL_WORDS);
    if (next_stack == UINT32_MAX) return UNAVAILABLE;
    TaskHandle_t handle = xTaskCreateStatic(entry, name, stack_words, argument, priority,
                                          &task_stacks[task_stack_used], &tasks[slot].tcb);
    if (handle == NULL) return UNAVAILABLE;
    task_stack_used = next_stack;
    tasks[slot].handle = handle;
    ++task_count;
    return (int32_t)(slot + 1);
}

int32_t rp1_freertos_start(uint32_t cpu_hz)
{
    int32_t result = create_context();
    if (result != 0) return result;
    /* Measured core clock, integer 1 ms SysTick reload; never infer it from RP1 defaults. */
    if (cpu_hz < 1000000U || cpu_hz > 1000000000U || cpu_hz % configTICK_RATE_HZ != 0) return INVALID;
    if (task_count == 0) return STATE;
    rp1_freertos_cpu_hz = cpu_hz;
    started = 1;
    vTaskStartScheduler();
    rp1_freertos_fault_hook(3, 0);
}

int32_t rp1_freertos_delay(uint32_t ticks)
{
    int32_t result = thread_context(1);
    if (result != 0) return result;
    vTaskDelay(ticks);
    return 0;
}

int32_t rp1_freertos_delay_until(uint32_t *previous, uint32_t increment)
{
    int32_t result = thread_context(1);
    if (result != 0) return result;
    if (previous == NULL || increment == 0 || increment >= 0x80000000U) return INVALID;
    TickType_t wake = *previous;
    BaseType_t delayed = xTaskDelayUntil(&wake, increment);
    *previous = (uint32_t)wake;
    return delayed != pdFALSE;
}

int32_t rp1_freertos_tick(uint32_t *ticks)
{
    int32_t result = thread_context(1);
    if (result != 0) return result;
    if (ticks == NULL) return INVALID;
    *ticks = (uint32_t)xTaskGetTickCount();
    return 0;
}

int32_t rp1_freertos_current_task(uint32_t *id)
{
    int32_t result = thread_context(1);
    if (result != 0) return result;
    if (id == NULL) return INVALID;
    *id = task_id(xTaskGetCurrentTaskHandle());
    return 0;
}

int32_t rp1_freertos_priority_get(uint32_t id)
{
    int32_t result = thread_context(1);
    if (result != 0) return result;
    TaskHandle_t handle = task_handle(id);
    if (handle == NULL) return INVALID;
    return (int32_t)uxTaskPriorityGet(handle);
}

int32_t rp1_freertos_stack_high_water(uint32_t id)
{
    int32_t result = thread_context(1);
    if (result != 0) return result;
    TaskHandle_t handle = task_handle(id);
    if (handle == NULL) return INVALID;
    return (int32_t)uxTaskGetStackHighWaterMark(handle);
}

int32_t rp1_freertos_notification_take(uint32_t clear, uint32_t ticks, uint32_t *count)
{
    int32_t result = thread_context(1);
    if (result != 0) return result;
    if (clear > 1 || count == NULL) return INVALID;
    *count = ulTaskNotifyTake((BaseType_t)clear, ticks);
    return 0;
}

int32_t rp1_freertos_notification_give(uint32_t id)
{
    int32_t result = thread_context(1);
    if (result != 0) return result;
    TaskHandle_t handle = task_handle(id);
    if (handle == NULL) return INVALID;
    return xTaskNotifyGive(handle) == pdPASS ? 0 : UNAVAILABLE;
}

/* One proc0 task transaction: an old canceller must not resume across rearm.
 * Notify is nonblocking; its PendSV runs only after the outer critical exit. */
int32_t rp1_freertos_cancel_notification(uint32_t generation,
        const volatile uint32_t *active, volatile uint32_t *cancelled,
        const volatile uint32_t *waiter)
{
    int32_t result = thread_context(1);
    if (result != 0) return result;
    if (active == NULL || cancelled == NULL || waiter == NULL) return INVALID;
    if (generation == 0) return 0;
    taskENTER_CRITICAL();
    result = 0;
    if (*active == generation) {
        TaskHandle_t handle = task_handle(*waiter);
        if (handle == NULL) result = INVALID;
        else {
            *cancelled = generation;
            result = xTaskNotifyGive(handle) == pdPASS ? 1 : UNAVAILABLE;
        }
    }
    taskEXIT_CRITICAL();
    return result;
}

int32_t rp1_freertos_notification_give_from_isr(uint32_t id)
{
    int32_t result = isr_context();
    if (result != 0) return result;
    TaskHandle_t handle = task_handle(id);
    if (handle == NULL) return INVALID;
    BaseType_t woken = pdFALSE;
    vTaskNotifyGiveFromISR(handle, &woken);
    portYIELD_FROM_ISR(woken);
    return woken != pdFALSE;
}

int32_t rp1_freertos_queue_create(uint32_t slot, uint32_t capacity)
{
    int32_t result = create_context();
    if (result != 0) return result;
    if (slot >= QUEUES || capacity == 0 || capacity > QUEUE_WORDS) return INVALID;
    if (queues[slot].handle != NULL) return OCCUPIED;
    queues[slot].handle = xQueueCreateStatic(capacity, sizeof(uint32_t),
                                           (uint8_t *)queues[slot].data, &queues[slot].control);
    return queues[slot].handle != NULL ? (int32_t)(slot + 1) : UNAVAILABLE;
}

int32_t rp1_freertos_queue_send(uint32_t id, uint32_t value, uint32_t ticks)
{
    int32_t result = thread_context(1);
    if (result != 0) return result;
    QueueHandle_t handle = queue_handle(id);
    if (handle == NULL) return INVALID;
    return xQueueSend(handle, &value, ticks) == pdPASS;
}

int32_t rp1_freertos_queue_receive(uint32_t id, uint32_t ticks, uint32_t *value)
{
    int32_t result = thread_context(1);
    if (result != 0) return result;
    QueueHandle_t handle = queue_handle(id);
    if (handle == NULL || value == NULL) return INVALID;
    return xQueueReceive(handle, value, ticks) == pdPASS;
}

int32_t rp1_freertos_semaphore_create(uint32_t kind, uint32_t slot)
{
    int32_t result = create_context();
    if (result != 0) return result;
    if (kind > MUTEX || slot >= SEMAPHORES) return INVALID;
    if (semaphores[kind][slot].handle != NULL) return OCCUPIED;
    semaphores[kind][slot].handle = kind == MUTEX ? xSemaphoreCreateMutexStatic(&semaphores[kind][slot].control)
                                               : xSemaphoreCreateBinaryStatic(&semaphores[kind][slot].control);
    return semaphores[kind][slot].handle != NULL ? (int32_t)(slot + 1) : UNAVAILABLE;
}

int32_t rp1_freertos_semaphore_take(uint32_t kind, uint32_t id, uint32_t ticks)
{
    int32_t result = thread_context(1);
    if (result != 0) return result;
    SemaphoreHandle_t handle = semaphore_handle(kind, id);
    if (handle == NULL) return INVALID;
    if (kind == MUTEX && xSemaphoreGetMutexHolder(handle) == xTaskGetCurrentTaskHandle()) return STATE;
    return xSemaphoreTake(handle, ticks) == pdPASS;
}

int32_t rp1_freertos_semaphore_give(uint32_t kind, uint32_t id)
{
    int32_t result = thread_context(1);
    if (result != 0) return result;
    SemaphoreHandle_t handle = semaphore_handle(kind, id);
    if (handle == NULL) return INVALID;
    if (kind == MUTEX && xSemaphoreGetMutexHolder(handle) != xTaskGetCurrentTaskHandle()) return STATE;
    return xSemaphoreGive(handle) == pdPASS;
}

int32_t rp1_freertos_binary_give_from_isr(uint32_t id)
{
    int32_t result = isr_context();
    if (result != 0) return result;
    SemaphoreHandle_t handle = semaphore_handle(BINARY, id);
    if (handle == NULL) return INVALID;
    BaseType_t woken = pdFALSE;
    result = xSemaphoreGiveFromISR(handle, &woken) == pdPASS;
    portYIELD_FROM_ISR(woken);
    return result;
}

int32_t rp1_freertos_binary_take_from_isr(uint32_t id)
{
    int32_t result = isr_context();
    if (result != 0) return result;
    SemaphoreHandle_t handle = semaphore_handle(BINARY, id);
    if (handle == NULL) return INVALID;
    BaseType_t woken = pdFALSE;
    result = xSemaphoreTakeFromISR(handle, &woken) == pdPASS;
    portYIELD_FROM_ISR(woken);
    return result;
}
