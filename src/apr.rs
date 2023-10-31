use crate::generated;
pub struct Pool(*mut generated::apr_pool_t);

impl Pool {
    pub fn new() -> Self {
        let mut pool: *mut generated::apr_pool_t = std::ptr::null_mut();
        unsafe {
            generated::apr_pool_create_ex(
                &mut pool,
                std::ptr::null_mut(),
                None,
                std::ptr::null_mut() as *mut generated::apr_allocator_t,
            );
        }
        Pool(pool)
    }

    pub fn subpool(&mut self) -> Self {
        let mut subpool: *mut generated::apr_pool_t = std::ptr::null_mut();
        unsafe {
            generated::apr_pool_create_ex(
                &mut subpool,
                self.0,
                None,
                std::ptr::null_mut() as *mut generated::apr_allocator_t,
            );
        }
        Pool(subpool)
    }
}

impl From<&mut Pool> for *mut generated::apr_pool_t {
    fn from(p: &mut Pool) -> Self {
        p.0
    }
}

impl Default for Pool {
    fn default() -> Self {
        Pool::new()
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        unsafe {
            generated::apr_pool_destroy(self.0);
        }
    }
}

pub struct Allocator(*mut generated::apr_allocator_t);

impl From<Allocator> for *mut generated::apr_allocator_t {
    fn from(a: Allocator) -> Self {
        a.0
    }
}

impl Allocator {
    pub fn new() -> Self {
        let mut allocator: *mut generated::apr_allocator_t = std::ptr::null_mut();
        unsafe {
            generated::apr_allocator_create(&mut allocator);
        }
        Allocator(allocator)
    }
}

impl Default for Allocator {
    fn default() -> Self {
        Allocator::new()
    }
}

impl Drop for Allocator {
    fn drop(&mut self) {
        unsafe {
            generated::apr_allocator_destroy(self.0);
        }
    }
}

#[ctor::ctor]
fn initialize_apr() {
    unsafe {
        assert!(generated::apr_initialize() == generated::APR_SUCCESS as i32);
    }
}
