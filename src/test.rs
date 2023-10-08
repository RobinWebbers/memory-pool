use super::TypedPool;
use std::vec::Vec;

#[test]
fn capacity_small() {
    let capacity = 2_usize.pow(4);
    let allocator = TypedPool::<u32>::new(capacity);
    assert!(allocator.capacity() == capacity);
}

#[test]
fn capacity_large() {
    let capacity = 2_usize.pow(30);
    let allocator = TypedPool::<u32>::new(capacity);
    assert!(allocator.capacity() == capacity);
}

#[test]
fn just_allocations() {
    let capacity = 2_usize.pow(8);
    let allocator = TypedPool::new(capacity);

    let vec: Vec<_> = (0..capacity).map(|i| allocator.alloc(i)).collect();

    for i in 0..capacity {
        assert!(i == *vec[i])
    }
}

#[test]
#[should_panic(expected="TypedPool out-of-memory")]
fn out_of_memory() {
    let capacity = 2_usize.pow(8);
    let allocator = TypedPool::new(capacity);

    let _vec: Vec<_> = (0..capacity).map(|i| allocator.alloc(i)).collect();

    // Should panic here!
    allocator.alloc(capacity + 1);
}

#[test]
fn reuse_freed_memory() {
    let capacity = 2_usize.pow(8);
    let allocator = TypedPool::new(capacity);

    let mut vec: Vec<_> = (0..capacity).map(|i| allocator.alloc(i)).collect();

    // Remove one fourth of the allocated entries
    for i in (0..capacity/2).step_by(2) {
        vec.swap_remove(i);
    }

    let _vec: Vec<_> = (0..capacity/4).map(|i| allocator.alloc(i)).collect();
}
