#ifndef RP1_FREERTOS_CONFIG_H
#define RP1_FREERTOS_CONFIG_H

#include <stdint.h>

extern uint32_t rp1_freertos_cpu_hz;
void rp1_freertos_assert(uint32_t line) __attribute__((noreturn));
void rp1_freertos_trace_switch(void *task);

#define configNUMBER_OF_CORES                    1
#define configUSE_PREEMPTION                     1
#define configUSE_TIME_SLICING                    1
#define configUSE_TICKLESS_IDLE                   0
#define configCPU_CLOCK_HZ                       (rp1_freertos_cpu_hz)
#define configTICK_RATE_HZ                       1000U
#define configTICK_TYPE_WIDTH_IN_BITS             TICK_TYPE_WIDTH_32_BITS
#define configMAX_PRIORITIES                     8
#define configMINIMAL_STACK_SIZE                 128U
#define configMAX_TASK_NAME_LEN                  16
#define configSUPPORT_STATIC_ALLOCATION          1
#define configSUPPORT_DYNAMIC_ALLOCATION         0
#define configKERNEL_PROVIDED_STATIC_MEMORY       0
#define configUSE_IDLE_HOOK                       0
#define configUSE_TICK_HOOK                       1
#define configUSE_TIMERS                         0
#define configUSE_MUTEXES                        1
#define configUSE_RECURSIVE_MUTEXES               0
#define configUSE_COUNTING_SEMAPHORES             0
#define configUSE_TASK_NOTIFICATIONS              1
#define configTASK_NOTIFICATION_ARRAY_ENTRIES     1
#define configQUEUE_REGISTRY_SIZE                0
#define configUSE_TRACE_FACILITY                  0
#define configCHECK_FOR_STACK_OVERFLOW            2
#define configCHECK_HANDLER_INSTALLATION          1
#define configPRIO_BITS                          3
#define configLIBRARY_LOWEST_INTERRUPT_PRIORITY   7
#define configLIBRARY_MAX_SYSCALL_INTERRUPT_PRIORITY 5
#define configKERNEL_INTERRUPT_PRIORITY           0xe0U
#define configMAX_SYSCALL_INTERRUPT_PRIORITY       0xa0U
#define configASSERT(condition) do { if (!(condition)) rp1_freertos_assert(__LINE__); } while (0)
#define traceTASK_SWITCHED_IN() rp1_freertos_trace_switch((void *)pxCurrentTCB)

#define INCLUDE_vTaskDelay                       1
#define INCLUDE_vTaskSuspend                     1
#define INCLUDE_uxTaskPriorityGet                 1
#define INCLUDE_uxTaskGetStackHighWaterMark        1
#define INCLUDE_xTaskGetCurrentTaskHandle         1
#define INCLUDE_xTaskGetIdleTaskHandle            1
#define INCLUDE_xTaskGetSchedulerState            1
#define INCLUDE_xSemaphoreGetMutexHolder          1
#define INCLUDE_vTaskDelete                      0
#ifdef RP1_FREERTOS_ASSERT_PROBE
#define INCLUDE_vTaskPrioritySet                 1
#else
#define INCLUDE_vTaskPrioritySet                 0
#endif

#endif
