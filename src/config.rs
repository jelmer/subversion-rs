pub struct Config<'pool>(apr::hash::Hash<'pool, &'pool str, *mut subversion_sys::svn_config_t>);
use apr::hash::apr_hash_t;

impl<'pool> Config<'pool> {
    pub fn as_ptr(&self) -> *const apr_hash_t {
        unsafe { self.0.as_ptr() }
    }

    pub fn as_mut_ptr(&mut self) -> *mut apr_hash_t {
        unsafe { self.0.as_mut_ptr() }
    }
}
