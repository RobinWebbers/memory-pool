use super::TypedPool;
use std::vec::Vec;

#[test]
fn capacity_small() {
    let capacity = 2_usize.pow(4);
    let pool = TypedPool::<u32>::new(capacity);
    assert!(pool.capacity() == capacity);
}

#[test]
fn capacity_large() {
    let capacity = 2_usize.pow(30);
    let pool = TypedPool::<u32>::new(capacity);
    assert!(pool.capacity() == capacity);
}

#[test]
fn just_allocations() {
    let capacity = 2_usize.pow(8);
    let pool = TypedPool::<usize>::new(capacity);

    let vec: Vec<_> = (0..capacity).map(|i| Box::new_in(i, &pool)).collect();

    for i in 0..capacity {
        assert!(i == *vec[i])
    }
}

#[test]
fn out_of_memory() {
    let capacity = 2_usize.pow(8);
    let pool = TypedPool::<usize>::new(capacity);

    let _vec: Vec<_> = (0..capacity).map(|i| Box::new_in(i, &pool)).collect();

    use std::alloc::{Allocator, AllocError, Layout};

    // We are out of memory here
    let result = pool.allocate(Layout::new::<usize>());
    assert_eq!(Err(AllocError), result);
}

#[test]
fn reuse_freed_memory() {
    let capacity = 2_usize.pow(8);
    let pool = TypedPool::<usize>::new(capacity);

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
    let pool = TypedPool::<()>::new(capacity);

    let vec: Vec<_> = (0..capacity).map(|_| Box::new_in((), &pool)).collect();

    for unit in vec {
        assert_eq!((), *unit);
    }
}
