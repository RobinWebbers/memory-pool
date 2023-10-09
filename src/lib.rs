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
//! use typed_pool::TypedPool;
//! 
//! struct Data {
//!     inner: usize,
//! }
//! 
//! let capacity = 2_usize.pow(20);
//! let pool = TypedPool::new(capacity);
//! 
//! let elem = pool.alloc(Data { inner: 0 });
//! 
//! // We can deallocate during the lifetime of pool
//! drop(elem);
//! 
//! // This new element can reuse the memory we freed
//! let elem = pool.alloc(Data { inner: 5 });
//! ```

#![cfg_attr(not(any(feature = "std", test)), no_std)]
#![deny(missing_docs)]

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(test)]
mod test;

use core::ptr::NonNull;
use core::mem::ManuallyDrop;
use core::cell::Cell;
use core::ops::{Deref, DerefMut};
use core::fmt;

use alloc::alloc::Layout;

/// A typed object pool for constant time (de)allocations.
pub struct TypedPool<T> {
    /// The memory region from which we will allocate.
    memory: NonNull<[Item<T>]>,
    /// Pointer to the next free item.
    next: Cell<NonNull<Item<T>>>,
}

impl<T> TypedPool<T> {
    /// Create a typed pool with the specified capacity.
    pub fn new(capacity: usize) -> Self {
        let address = if capacity == 0 {
            NonNull::dangling()
        } else {
            let layout = Layout::array::<Item<T>>(capacity)
                .expect("cannot allocate more than isize::MAX");

            // Zeroed memory will be None for Option<NonNull<T>>
            let memory = unsafe { alloc::alloc::alloc_zeroed(layout) };

            NonNull::new(memory as _)
                .unwrap_or_else(|| alloc::alloc::handle_alloc_error(layout))
        };

        let memory = NonNull::slice_from_raw_parts(address, capacity);

        Self {
            memory,
            next: address.into(),
        }
    }

    /// The maximum number of entries this pool can contain.
    pub fn capacity(&self) -> usize {
        self.memory.len()
    }

    /// Allocates a new object in the pool. Note that the [`TypedPool`] does not
    /// grow its backing memory after the initial allocation.
    ///
    /// # Panics
    ///
    /// Panics if the new allocation exceeds the capacity.
    pub fn alloc(&self, val: T) -> Owned<'_, T> {
        // Check if we have run out of memory
        let mut item = self.next.get();
        if !self.contains(item) { panic!("TypedPool out-of-memory") }

        // Get the next allocation in the chain
        let redirect = unsafe { item.as_ref().next };
        let adjacent = unsafe { NonNull::new_unchecked(item.as_ptr().add(1)) };

        // The next item is either the next on in the chain,
        // or the one adjacent to the fully allocated block.
        self.next.set(redirect.unwrap_or(adjacent));

        // We write first because we do not want to drop the unitialised 
        // value that currently resides in data.
        unsafe { 
            *item.as_mut().data = val;
            Owned::from_raw(item.cast(), self)
        }
    }

    /// Deallocates the item, such that the memory will be used by subsequent
    /// allocations. This does not drop the item that was contained.
    ///
    /// # Safety
    ///
    /// Improper use may lead to memory errors. Additionally, it is not checked
    /// whether the item is contained in the allocator.
    unsafe fn dealloc(&self, mut ptr: NonNull<Item<T>>) {
        debug_assert!(self.contains(ptr));
        let item = unsafe { ptr.as_mut() };

        // Let this entry point to the next free slot
        item.next = Some(self.next.get());

        // Let our next allocation be the one that was just freed
        self.next.set(ptr.into());
    }

    /// Check if the given pointer is in this pools address range.
    /// It does NOT (and cannot) check whether the entry is allocated.
    fn contains(&self, ptr: NonNull<Item<T>>) -> bool {
        // The memory region is owned, so we can create a reference to it.
        let slice = unsafe { self.memory.as_ref() };
        slice.as_ptr_range().contains(&(ptr.as_ptr() as *const _))
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
    /// Data of this allocated item.
    data: ManuallyDrop<T>,
}

/// Ownership over an element allocated in a [`TypedPool`].
pub struct Owned<'pool, T> {
    data: NonNull<T>,
    pool: &'pool TypedPool<T>,
}

impl<'pool, T> Owned<'pool, T> {
    /// Constructs an [`Owned`] from a raw pointer and its [`TypedPool`].
    ///
    /// # Safety
    ///
    /// This function is unsafe because improper use may lead to memory
    /// problems. For example, a double-free may occur if the function is called
    /// twice on the same raw pointer.
    ///
    /// This function does not check whether the pointer was indeed contained
    /// by the [`TypedPool`].
    pub unsafe fn from_raw(raw: NonNull<T>, pool: &'pool TypedPool<T>) -> Self {
        debug_assert!(pool.contains(raw.cast()));

        Self {
            data: raw,
            pool,
        }
    }

    /// Consumes the [`Owned`], returning the raw pointer.
    ///
    /// Contrary to a Box, the underlying allocation is owned by a
    /// [`TypedPool`]. The user is however responsible for dropping the inner
    /// item, as the [`TypedPool`] does not drop remaining elements. The easiest
    /// way to do this is to convert the raw point back into an [`Owned`] with
    /// [`Owned::from_raw`]. It's destructor will then do the clean up.
    pub fn into_raw(owned: Self) -> NonNull<T> {
        let owned = ManuallyDrop::new(owned);
        owned.data
    }

    /// Get a reference to the [`TypedPool`] this object was allocated in.
    pub fn typed_pool(owned: &Self) -> &'pool TypedPool<T> {
        owned.pool
    }
}

impl<T: fmt::Display> fmt::Display for Owned<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}
impl<T: fmt::Debug> fmt::Debug for Owned<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T> fmt::Pointer for Owned<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ptr: *const T = &**self;
        fmt::Pointer::fmt(&ptr, f)
    }
}

impl<T> Deref for Owned<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.data.as_ref() }
    }
}

impl<T> DerefMut for Owned<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.data.as_mut() }
    }
}

impl<T> Drop for Owned<'_, T> {
    fn drop(&mut self) {
        unsafe {
            core::ptr::drop_in_place(self.data.as_ptr());
            self.pool.dealloc(self.data.cast());
        }
    }
}

