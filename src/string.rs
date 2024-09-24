use crate::generated::svn_string_t;
use apr::pool::PooledPtr;

pub struct String(PooledPtr<svn_string_t>);

impl String {
    pub fn as_ptr(&self) -> *const svn_string_t {
        self.0.as_ptr()
    }

    pub fn as_mut_ptr(&mut self) -> *mut svn_string_t {
        self.0.as_mut_ptr()
    }

    pub fn as_slice(&self) -> &[u8] {
        let ptr = unsafe { (*self.as_ptr()).data };
        let len = unsafe { (*self.as_ptr()).len as usize };
        unsafe { std::slice::from_raw_parts(ptr as *const u8, len) }
    }
}

impl From<String> for Vec<u8> {
    fn from(s: String) -> Self {
        s.as_slice().to_vec()
    }
}

impl From<PooledPtr<svn_string_t>> for String {
    fn from(ptr: PooledPtr<svn_string_t>) -> Self {
        String(ptr)
    }
}

impl From<&str> for String {
    fn from(s: &str) -> Self {
        let ptr = apr::pool::PooledPtr::initialize(|pool| {
            let cstr = s.as_ptr();
            let len = s.len();
            let ptr = unsafe {
                crate::generated::svn_string_ncreate(cstr as *const i8, len, pool.as_mut_ptr())
            };
            Ok::<_, crate::Error>(ptr)
        })
        .unwrap();
        String(ptr)
    }
}

impl From<std::string::String> for String {
    fn from(s: std::string::String) -> Self {
        s.as_str().into()
    }
}

impl From<&[u8]> for String {
    fn from(s: &[u8]) -> Self {
        let ptr = apr::pool::PooledPtr::initialize(|pool| {
            let cstr = s.as_ptr();
            let len = s.len();
            let ptr = unsafe {
                crate::generated::svn_string_ncreate(cstr as *const i8, len, pool.as_mut_ptr())
            };
            Ok::<_, crate::Error>(ptr)
        })
        .unwrap();
        String(ptr)
    }
}

impl From<Vec<u8>> for String {
    fn from(s: Vec<u8>) -> Self {
        s.as_slice().into()
    }
}
