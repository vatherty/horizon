/* Public SDK calls only. The SDK owns the Mach struct layout and counts. */
#include <mach/mach.h>
#include <os/proc.h>
#include <stdint.h>

uint32_t horizon_process_memory_ios(uint64_t *resident, uint64_t *footprint,
                                    uint64_t *available) {
    uint32_t known = 0;
    mach_task_basic_info_data_t basic = {0};
    mach_msg_type_number_t count = MACH_TASK_BASIC_INFO_COUNT;
    if (task_info(mach_task_self(), MACH_TASK_BASIC_INFO,
                  (task_info_t)&basic, &count) == KERN_SUCCESS &&
        count >= MACH_TASK_BASIC_INFO_COUNT) {
        *resident = basic.resident_size;
        known |= 1;
    }
    task_vm_info_data_t vm = {0};
    count = TASK_VM_INFO_COUNT;
    if (task_info(mach_task_self(), TASK_VM_INFO, (task_info_t)&vm, &count) ==
            KERN_SUCCESS && count >= TASK_VM_INFO_REV1_COUNT) {
        *footprint = vm.phys_footprint;
        known |= 2;
    }
    if (__builtin_available(iOS 13.0, *)) {
        *available = os_proc_available_memory();
        known |= 4;
    }
    return known;
}
