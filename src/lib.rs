//! **A fixed-size block allocator for constant time (de)allocations.**
//! 
//! The memory pool reserves a fixed size of (virtual) memory and does not grow
//! with new allocations. We store a pointer in free entries, so types smaller
//! than a pointer have additional space overhead. The flipside is that we can
//! rapidly allocate and free entries, no matter the access pattern.
//! 
//! The primary use case of this crate is as a performance optimisation for
//! (de)allocation heavy code.
//! 
//! # Example
//! 
//! ```rust
//! #![feature(allocator_api)]
//!
//! use memory_pool::MemoryPool;
//! use std::alloc::Layout;
//! 
//! struct Data {
//!     inner: usize,
//! }
//! 
//! let capacity = 2_usize.pow(20);
//! let pool = MemoryPool::new(capacity, Layout::new::<Data>());
//! 
//! let elem = Box::new_in(Data { inner: 0 }, &pool);
//! 
//! // We can deallocate during the lifetime of pool
//! drop(elem);
//! 
//! // This new element can reuse the memory we freed
//! let elem = Box::new_in(Data { inner: 5 }, &pool);
//! ```
#![feature(allocator_api)]
#![feature(slice_ptr_get)]
#![feature(alloc_layout_extra)]

#![cfg_attr(not(any(feature = "std", test)), no_std)]
#![deny(missing_docs)]

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(test)]
mod test;

use core::ptr::NonNull;
use core::cell::Cell;

use alloc::alloc::{Allocator, AllocError, Global, Layout};

/// A memory pool for (de)allocation fixed-size blocks in constant time. It is
/// not thread safe and incurs space overhead for types smaller than a pointer.
pub struct MemoryPool {
    /// The layout requirement of the blocks in our allocator.
    layout: Layout,
    /// The memory region from which we will allocate.
    memory: NonNull<[u8]>,
    /// Pointer to the next free item. We store this as a u8 pointer because
    /// the free list nodes are stored based on the layout of the blocks, not
    /// their own.
    next: Cell<NonNull<u8>>,
}

impl MemoryPool {
    /// Create a memory pool with the a maximum capacity where each block
    /// adheres the given layout requirements.
    ///
    /// Note that the minimum size for each allocation is a pointer. This means
    /// that even zero sized types actually consume memory in this structure.
    ///
    /// # Panics
    ///
    /// This will panic on incorrect layouts and if the global allocator is out
    /// of memory.
    pub fn new(capacity: usize, layout: Layout) -> Self {
        let layout = union_layout(layout, Layout::new::<Free>())
            // Pad the layout to be multiples of the alignment. We use this
            // property when calculating the next free entry.
            .pad_to_align();

        // Get the layout for the array.
        let (array, _) = layout.repeat(capacity)
            .expect("layout did not satisfy its constraints");

        // Zeroed memory will be None for Option<NonNull<_>>
        let memory = Global.allocate_zeroed(array)
            .unwrap_or_else(|_| alloc::alloc::handle_alloc_error(layout));

        // The next free element is the first entry in the allocated block.
        let base = memory.as_non_null_ptr();

        Self {
            layout,
            memory,
            next: base.into(),
        }
    }

    /// The maximum number of entries this pool can contain.
    pub fn capacity(&self) -> usize {
        self.memory.len() / self.layout.size()
    }

    /// Check if the given pointer is in this pools address range.
    /// It does NOT (and cannot) check whether the entry is allocated.
    fn contains(&self, ptr: NonNull<u8>) -> bool {
        // The memory region is owned, so we can create a reference to it.
        let slice = unsafe { self.memory.as_ref() };
        slice.as_ptr_range().contains(&(ptr.as_ptr() as *const _))
    }
}

unsafe impl Allocator for MemoryPool {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        // Check if we are allocating the correct object (we cannot do more
        // than to check layout requirements sadly).

        // Check if given layout fits the layout requirements.
        if self.layout != union_layout(self.layout, layout) { return Err(AllocError) }

        // Check if we have run out of memory
        let block = self.next.get();
        if !self.contains(block) { return Err(AllocError) }

        // Get the next allocation in the chain
        let redirect = unsafe { *block.cast::<Free>().as_ref() };

        // Get the element adjecent to the current free one.
        let adjacent = unsafe {
            let adjacent = block.as_ptr().add(self.layout.size());
            NonNull::new_unchecked(adjacent)
        };

        // The next item is either the next on in the chain,
        // or the one adjacent if there was none.
        self.next.set(redirect.unwrap_or(adjacent));

        // Construct the slice to the allocated block.
        let slice = NonNull::slice_from_raw_parts(block, self.layout.size());
        Ok(slice)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        // Check if given layout fits the layout requirements.
        debug_assert_eq!(self.layout, union_layout(self.layout, layout));
        // Check if the given pointer is contained in the allocator.
        debug_assert!(self.contains(ptr));

        // Let this entry point to the next free slot
        *ptr.cast::<Free>().as_mut() = Some(self.next.get());

        // Let our next allocation be the one that was just freed
        self.next.set(ptr.into());
    }
}

impl Drop for MemoryPool {
    fn drop(&mut self) {
        // This exact layout was already created, so this cannot fail.
        let (layout, _) = self.layout.repeat(self.capacity()).unwrap();
        unsafe { alloc::alloc::dealloc(self.memory.cast().as_ptr(), layout) }
    }
}

/// A pointer to the next free entry in our pool. This will essentially form a
/// chain of pointers in memory.
type Free = Option<NonNull<u8>>;

/// Returns a new layout as if the given two layouts were put into a union.
fn union_layout(first: Layout, second: Layout) -> Layout {
    let size = core::cmp::max(first.size(), second.size());
    let align = core::cmp::max(first.align(), second.align());
    Layout::from_size_align(size, align)
        .expect("layout did not satisfy its constraints")
}
