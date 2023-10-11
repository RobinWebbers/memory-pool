//! **A fixed size allocator for single typed, constant time (de)allocations.**
//! 
//! The typed pool reserves a fixed size of (virtual) memory and does not grow
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
//! use typed_pool::TypedPool;
//! 
//! struct Data {
//!     inner: usize,
//! }
//! 
//! let capacity = 2_usize.pow(20);
//! let pool = TypedPool::<Data>::new(capacity);
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

#![cfg_attr(not(any(feature = "std", test)), no_std)]
#![deny(missing_docs)]

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(test)]
mod test;

use core::ptr::NonNull;
use core::mem::ManuallyDrop;
use core::cell::Cell;

use alloc::alloc::{Allocator, AllocError, Global, Layout};

/// A typed object pool for constant time (de)allocations. It is not thread safe
/// and incurs space overhead for types smaller than a pointer.
pub struct TypedPool<T> {
    /// The memory region from which we will allocate.
    memory: NonNull<[Item<T>]>,
    /// Pointer to the next free item.
    next: Cell<NonNull<Item<T>>>,
}

impl<T> TypedPool<T> {
    /// Create a typed pool with the specified capacity.
    pub fn new(capacity: usize) -> Self {
        let layout = Layout::array::<Item<T>>(capacity)
            .expect("cannot allocate more than isize::MAX");

        // Zeroed memory will be None for Option<NonNull<T>>
        let memory = Global.allocate_zeroed(layout)
            .unwrap_or_else(|_| alloc::alloc::handle_alloc_error(layout));

        // Cast the byte slice to an Item slice.
        let base = memory.as_non_null_ptr().cast();
        let memory = NonNull::slice_from_raw_parts(base, capacity);

        Self {
            memory,
            next: base.into(),
        }
    }

    /// The maximum number of entries this pool can contain.
    pub fn capacity(&self) -> usize {
        self.memory.len()
    }

    /// Check if the given pointer is in this pools address range.
    /// It does NOT (and cannot) check whether the entry is allocated.
    fn contains(&self, ptr: NonNull<Item<T>>) -> bool {
        // The memory region is owned, so we can create a reference to it.
        let slice = unsafe { self.memory.as_ref() };
        slice.as_ptr_range().contains(&(ptr.as_ptr() as *const _))
    }
}

unsafe impl<T> Allocator for TypedPool<T> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        // Check if we are allocating the correct object (we cannot do more
        // than to check layout requirements sadly).
        if layout != Layout::new::<T>() { return Err(AllocError) }

        // Check if we have run out of memory
        let item = self.next.get();
        if !self.contains(item) { return Err(AllocError) }

        // Get the next allocation in the chain
        let redirect = unsafe { item.as_ref().next };
        let adjacent = unsafe { NonNull::new_unchecked(item.as_ptr().add(1)) };

        // The next item is either the next on in the chain,
        // or the one adjacent to the fully allocated block.
        self.next.set(redirect.unwrap_or(adjacent));

        // Cast the item pointer to a u8 slice pointer.
        let base: NonNull<u8> = item.cast();
        let length = layout.size();
        let slice = NonNull::slice_from_raw_parts(base, length);
        Ok(slice)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        let mut ptr: NonNull<Item<T>> = ptr.cast();

        assert_eq!(layout, Layout::new::<T>());
        assert!(self.contains(ptr));

        // Let this entry point to the next free slot
        ptr.as_mut().next = Some(self.next.get());

        // Let our next allocation be the one that was just freed
        self.next.set(ptr.into());
    }
}

impl<T> Drop for TypedPool<T> {
    fn drop(&mut self) {
        // This exact layout was already created, so this cannot fail.
        let layout = Layout::array::<Item<T>>(self.capacity()).unwrap();
        unsafe { alloc::alloc::dealloc(self.memory.cast().as_ptr(), layout) }
    }
}

/// An item in the allocator. Unallocated items should be intepreted
/// as a pointer to the next free entry. Allocated entries are data.
#[derive(Copy, Clone)]
union Item<T> {
    /// Pointer to the next free item, if this item is unalloceted.
    next: Option<NonNull<Item<T>>>,
    /// Data of this allocated item. This is here to get correct layout for the
    /// allocations.
    _data: ManuallyDrop<T>,
}
