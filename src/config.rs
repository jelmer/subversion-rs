pub struct Config<'pool>(apr::hash::Hash<'pool>);
use apr::hash::apr_hash_t;

impl<'pool> Config<'pool> {
    pub fn as_ptr(&self) -> *const apr_hash_t {
        unsafe { self.0.as_ptr() }
    }

    pub fn as_mut_ptr(&mut self) -> *mut apr_hash_t {
        unsafe { self.0.as_mut_ptr() }
    }
}
