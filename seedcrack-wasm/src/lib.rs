use std::cell::RefCell;

use seedcrack::Task;

thread_local! {
    static TASK: RefCell<Option<Task>> = const { RefCell::new(None) };
    static OUT: RefCell<Vec<i64>> = const { RefCell::new(Vec::new()) };
}

#[unsafe(no_mangle)]
pub extern "C" fn seed_alloc(len: u32) -> *mut u8 {
    let mut buffer = Vec::<u8>::with_capacity(len as usize);
    let ptr = buffer.as_mut_ptr();
    std::mem::forget(buffer);
    ptr
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn seed_set_task(ptr: *mut u8, len: u32) -> u32 {
    let bytes = unsafe { Vec::from_raw_parts(ptr, len as usize, len as usize) };
    match Task::from_bytes(&bytes) {
        Some(task) => {
            TASK.with_borrow_mut(|t| *t = Some(task));
            1
        }
        None => 0,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn seed_task_id() -> u64 {
    TASK.with_borrow(|t| t.as_ref().map_or(0, Task::id))
}

#[unsafe(no_mangle)]
pub extern "C" fn seed_scan(start: u64, end: u64) -> u32 {
    let found = TASK.with_borrow(|t| t.as_ref().map(|t| t.scan(start..end.max(start))));
    let Some(found) = found else { return 0 };
    let count = found.len() as u32;
    OUT.with_borrow_mut(|out| *out = found);
    count
}

#[unsafe(no_mangle)]
pub extern "C" fn seed_out() -> *const i64 {
    OUT.with_borrow(|out| out.as_ptr())
}
