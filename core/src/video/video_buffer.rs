use std::sync::atomic::{AtomicPtr, AtomicBool, Ordering};
use std::ptr;

use crate::video::{MAX_VIDEO_HEIGHT, MAX_VIDEO_WIDTH};

pub struct VideoBuffer {
    front: AtomicPtr<Vec<u8>>, // UI reads from here
    back: AtomicPtr<Vec<u8>>,  // Emulator writes here
    
    // Flags for synchronization
    pub has_new_data: AtomicBool,
    pub ui_consumed: AtomicBool,

    size: usize,
}

impl VideoBuffer {
    pub fn new() -> Self {
        let size = MAX_VIDEO_WIDTH * MAX_VIDEO_HEIGHT * 4;
        let b1 = Box::into_raw(Box::new(vec![0; size]));
        let b2 = Box::into_raw(Box::new(vec![0; size]));

        Self {
            front: AtomicPtr::new(b1),
            back: AtomicPtr::new(b2),
            has_new_data: AtomicBool::new(false),
            ui_consumed: AtomicBool::new(true), // Start ready to accept
            size,
        }
    }

    /// UI THREAD: Called during the requestAnimationFrame loop
    pub fn get_pixels_for_ui(&self) -> Option<&[u8]> {
        // Only provide data if the emulator says there's something new
        if self.has_new_data.load(Ordering::Acquire) {
            let ptr = self.front.load(Ordering::Acquire);
            return Some(unsafe { &*ptr });
        }
        None
    }

    /// UI THREAD: Call this after pixels.render() is done
    pub fn mark_as_consumed(&self) {
        self.has_new_data.store(false, Ordering::Release);
        self.ui_consumed.store(true, Ordering::Release);
    }

    /// EMULATOR THREAD: Get the buffer to write to
    pub fn get_back_buffer_mut(&self) -> &mut [u8] {
        let ptr = self.back.load(Ordering::Acquire);
        unsafe { &mut *ptr }
    }

    /// EMULATOR THREAD: The "Internal Flip"
    /// Call this whenever the emulator reaches a point where it wants 
    /// the UI to see the current state.
    pub fn try_flip(&self) {
        // Only flip if the UI has finished reading the previous front buffer
        if self.ui_consumed.load(Ordering::Acquire) {
            let back_ptr = self.back.load(Ordering::Relaxed);
            let front_ptr = self.front.load(Ordering::Relaxed);

            // Swap the pointers
            self.back.store(front_ptr, Ordering::Release);
            self.front.store(back_ptr, Ordering::Release);

            // PERSISTENCE: Copy current state to the new back buffer 
            // This is safe because the UI is NOT reading 'front' yet 
            // (has_new_data is still false) and the emulator hasn't 
            // resumed work yet.
            unsafe {
                ptr::copy_nonoverlapping(
                    (*back_ptr).as_ptr(), 
                    (*front_ptr).as_mut_ptr(), 
                    self.size
                );
            }

            // Signal to UI that 'front' is ready
            self.ui_consumed.store(false, Ordering::Release);
            self.has_new_data.store(true, Ordering::Release);
        }
    }
}
