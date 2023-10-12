use super::MemoryPool;
use std::vec::Vec;
use std::alloc::Layout;

#[test]
fn capacity_small() {
    let capacity = 2_usize.pow(4);
    let pool = MemoryPool::new(capacity, Layout::new::<usize>());
    assert!(pool.capacity() == capacity);
}

#[test]
fn capacity_large() {
    let capacity = 2_usize.pow(30);
    let pool = MemoryPool::new(capacity, Layout::new::<u32>());
    assert!(pool.capacity() == capacity);
}

#[test]
fn just_allocations() {
    let capacity = 2_usize.pow(8);
    let pool = MemoryPool::new(capacity, Layout::new::<usize>());

    let vec: Vec<_> = (0..capacity).map(|i| Box::new_in(i, &pool)).collect();

    for i in 0..capacity {
        assert!(i == *vec[i])
    }
}

#[test]
fn out_of_memory() {
    let capacity = 2_usize.pow(8);
    let pool = MemoryPool::new(capacity, Layout::new::<usize>());

    let _vec: Vec<_> = (0..capacity).map(|i| Box::new_in(i, &pool)).collect();

    use std::alloc::{Allocator, AllocError, Layout};

    // We are out of memory here
    let result = pool.allocate(Layout::new::<usize>());
    assert_eq!(Err(AllocError), result);
}

#[test]
fn reuse_freed_memory() {
    let capacity = 2_usize.pow(8);
    let pool = MemoryPool::new(capacity, Layout::new::<usize>());

    let mut vec: Vec<_> = (0..capacity).map(|i| Box::new_in(i, &pool)).collect();

    // Drop one fourth of the allocated entries
    for i in (0..capacity/2).step_by(2) {
        vec.swap_remove(i);
    }

    // Allocate on fourth again.
    let _vec: Vec<_> = (0..capacity/4).map(|i| Box::new_in(i, &pool)).collect();
}

#[test]
fn zero_sized_types() {
    let capacity = 2_usize.pow(8);
    let pool = MemoryPool::new(capacity, Layout::new::<()>());

    let vec: Vec<_> = (0..capacity).map(|_| Box::new_in((), &pool)).collect();

    for unit in vec {
        assert_eq!((), *unit);
    }
}

#[test]
fn allocate_smaller_block() {
    let capacity = 2_usize.pow(8);
    let pool = MemoryPool::new(capacity, Layout::new::<usize>());

    let _ = Box::new_in(5_u8, &pool);
}
