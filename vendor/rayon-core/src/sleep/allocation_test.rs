//! Check the cold construction boundary, independently of scheduler timing.
use super::Sleep;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static EVENTS: Cell<Option<usize>> = const { Cell::new(None) };
}
struct CountAllocations;
#[global_allocator]
static ALLOCATOR: CountAllocations = CountAllocations;

fn record() {
    let _ = EVENTS.try_with(|events| {
        if let Some(count) = events.get() { events.set(Some(count + 1)); }
    });
}
unsafe impl GlobalAlloc for CountAllocations {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record();
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record();
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record();
        unsafe { System.realloc(pointer, layout, size) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
}

#[test]
fn cold_sleep_state_has_no_first_use_allocations() {
    for count in [1, 4, 16] {
        let sleep = Sleep::new(count);
        EVENTS.with(|events| events.set(Some(0)));
        // Exercise all independent runtime objects without relying on which
        // worker happens to become idle during a workload or warm-up delay.
        for state in &sleep.worker_sleep_states {
            drop(state.is_blocked.lock().unwrap());
            state.condvar.notify_all();
        }
        let events = EVENTS.with(|events| events.replace(None)).unwrap();
        assert_eq!(events, 0, "cold pool with {count} workers");
    }
}
