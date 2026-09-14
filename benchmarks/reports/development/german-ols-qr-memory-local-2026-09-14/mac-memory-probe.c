#include <mach/mach.h>
#include <stdio.h>
#include <stdint.h>
#include <sys/sysctl.h>
int main(void) {
    vm_statistics64_data_t stats = {0};
    mach_msg_type_number_t count=HOST_VM_INFO64_COUNT;
    mach_port_t host=mach_host_self();
    kern_return_t rc=host_statistics64(host, HOST_VM_INFO64, (host_info64_t)&stats, &count);
    kern_return_t drop=mach_port_deallocate(mach_task_self(), host);
    uint64_t ps=0,free=0, inactive=0,spec=0; size_t len=sizeof(ps);
    int prc=sysctlbyname("hw.pagesize",&ps,&len,0,0);
    len=sizeof(free); int frc=sysctlbyname("vm.page_free_count",&free,&len,0,0);
    len=sizeof(inactive); int irc=sysctlbyname("vm.page_inactive_count",&inactive,&len,0,0);
    len=sizeof(spec); int src=sysctlbyname("vm.page_speculative_count",&spec,&len,0,0);
    printf("mach_rc=%d drop_rc=%d count=%u expected_count=%u page_size_rc=%d page_size=%llu\n",rc,drop,count,HOST_VM_INFO64_COUNT,prc,(unsigned long long)ps);
    printf("mach_free_including_speculative=%u mach_inactive=%u mach_speculative=%u available_bytes=%llu\n",stats.free_count,stats.inactive_count,stats.speculative_count,(unsigned long long)((uint64_t)stats.free_count+stats.inactive_count)*ps);
    printf("sysctl_free_rc=%d free=%llu inactive_rc=%d inactive=%llu speculative_rc=%d speculative=%llu old_available_bytes=%llu\n",frc,(unsigned long long)free,irc,(unsigned long long)inactive,src,(unsigned long long)spec,(unsigned long long)(free+inactive+spec)*ps);
    return rc || drop || prc;
}
