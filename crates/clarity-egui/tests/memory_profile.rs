//! Simple allocation profile for a loaded Session.
//!
//! Run with:
//!   cargo test -p clarity-egui --test memory_profile --release -- --nocapture

#![allow(unsafe_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

struct CountingAlloc(AtomicUsize);

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.0.fetch_add(layout.size(), Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.0.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if !new_ptr.is_null() {
            self.0.fetch_sub(layout.size(), Ordering::Relaxed);
            self.0.fetch_add(new_size, Ordering::Relaxed);
        }
        new_ptr
    }
}

#[global_allocator]
static ALLOC: CountingAlloc = CountingAlloc(AtomicUsize::new(0));

fn current_bytes() -> usize {
    ALLOC.0.load(Ordering::Relaxed)
}

#[test]
fn session_memory_profile() {
    use clarity_egui::session::new_session;
    use clarity_egui::ui::types::{Message, Role, SessionContext};

    let before = current_bytes();
    let mut session = new_session(0, SessionContext::Chat);
    let after_empty = current_bytes();

    for i in 0..1000 {
        let content = format!(
            "This is message number {i} with some text, `inline code`, and **bold** styling.\n\
             It spans a few lines to be representative of a real assistant reply."
        );
        let mut msg = Message {
            role: Role::Agent,
            content,
            blocks: Vec::new(),
            timestamp: Instant::now(),
            parsed: Vec::new(),
            cached_height: None,
            is_error: false,
            lines: Vec::new(),
        };
        msg.prepare();
        session.messages.push(msg);
    }

    let after_populated = current_bytes();
    let _cloned = session.clone();
    let after_clone = current_bytes();

    println!(
        "memory_profile: empty_session={}B populated_1000={}B clone_delta={}B per_msg={:.1}B",
        after_empty - before,
        after_populated - before,
        after_clone - after_populated,
        (after_populated - after_empty) as f64 / 1000.0
    );
}
