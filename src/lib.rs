pub mod arena {
    use std::alloc::{alloc, dealloc, Layout};
    use std::cell::RefCell;
    use std::marker::PhantomData;
    use std::marker::Sized;
    use std::mem;
    use std::ptr;

    const BLOCK_SIZE: usize = 4096;

    struct Block {
        ptr: *mut u8,
        layout: Layout,
        count_of_elements: usize,
    }

    struct Internal<'a, T: 'a> {
        blocks: Vec<Block>,
        bytes: usize,
        alloc_bytes_remaining: usize,
        alloc_ptr: *mut u8,
        _marker: PhantomData<&'a T>,
    }

    impl<'a, T: Sized> Internal<'a, T> {
        fn new() -> Self {
            Self {
                blocks: Vec::new(),
                bytes: 0,
                alloc_bytes_remaining: 0,
                alloc_ptr: ptr::null_mut(),
                _marker: PhantomData,
            }
        }

        unsafe fn alloc(&mut self, data: T) -> &'a mut T {
            let mut layout = Layout::new::<T>();
            if layout.size() > self.alloc_bytes_remaining {
                self.alloc_bytes_remaining = layout.size();
                if layout.size() <= BLOCK_SIZE {
                    layout = Layout::from_size_align_unchecked(BLOCK_SIZE, 0);
                    self.alloc_bytes_remaining = BLOCK_SIZE;
                }
                self.bytes += layout.size();
                let block_ptr = alloc(layout);
                self.blocks.push(Block {
                    ptr: block_ptr,
                    layout,
                    count_of_elements: 0,
                });
                self.alloc_ptr = block_ptr;
            }

            self.alloc_bytes_remaining -= layout.size();
            let ptr = self.alloc_ptr;
            self.alloc_ptr = self.alloc_ptr.add(layout.size());
            self.blocks.last_mut().map(|b| b.count_of_elements += 1);

            let x = mem::transmute::<*mut u8, &mut T>(ptr);
            ptr::write(x, data);

            x
        }
    }

    impl<'a, T: Sized> Drop for Internal<'a, T> {
        fn drop(&mut self) {
            let layout = Layout::new::<T>();
            unsafe {
                for block in self.blocks.iter() {
                    for i in 0..block.count_of_elements {
                        let offset = layout.size() * i;
                        let ptr = block.ptr.add(offset);
                        let x = mem::transmute::<*mut u8, &mut T>(ptr);
                        std::ptr::drop_in_place(x);
                    }
                    dealloc(block.ptr, block.layout);
                }
            }
        }
    }

    pub struct Arena<'a, T> {
        internal: RefCell<Internal<'a, T>>,
    }

    impl<'a, T: Sized> Arena<'a, T> {
        pub fn new() -> Self {
            Self {
                internal: RefCell::new(Internal::new()),
            }
        }

        pub fn alloc(&self, data: T) -> &'a mut T {
            unsafe { self.internal.borrow_mut().alloc(data) }
        }

        pub fn bytes_allocated(&self) -> usize {
            self.internal.borrow().bytes
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::arena;

    struct X<'a> {
        drop_cnt: &'a RefCell<i32>,
    }

    impl<'a> Drop for X<'a> {
        fn drop(&mut self) {
            *self.drop_cnt.borrow_mut() += 1;
        }
    }

    #[test]
    fn it_works() {
        let drop_cnt = RefCell::new(0);
        {
            let arena = arena::Arena::new();
            for _ in 0..1000 {
                arena.alloc(X {
                    drop_cnt: &drop_cnt,
                });
            }
            assert!(arena.bytes_allocated() > 0);
        }
        assert_eq!(*drop_cnt.borrow(), 1000);
    }
}
