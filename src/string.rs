use std::marker::PhantomData;
use subversion_sys::svn_string_t;

/// Borrowed view of an SVN string tied to pool lifetime
pub struct BStr<'pool> {
    ptr: *const svn_string_t,
    _pool: PhantomData<&'pool apr::Pool>,
}

impl<'pool> BStr<'pool> {
    /// Create from raw SVN string pointer
    pub fn from_raw(ptr: *const svn_string_t) -> Self {
        Self {
            ptr,
            _pool: PhantomData,
        }
    }

    /// Create SVN string in pool from bytes
    pub fn from_bytes(data: &[u8], pool: &'pool apr::Pool) -> Self {
        let ptr = unsafe {
            subversion_sys::svn_string_ncreate(
                data.as_ptr() as *const i8,
                data.len(),
                pool.as_mut_ptr(),
            )
        };
        Self::from_raw(ptr)
    }

    /// Create SVN string in pool from str
    pub fn from_str(s: &str, pool: &'pool apr::Pool) -> Self {
        Self::from_bytes(s.as_bytes(), pool)
    }

    pub fn as_ptr(&self) -> *const svn_string_t {
        self.ptr
    }

    pub fn as_mut_ptr(&mut self) -> *mut svn_string_t {
        self.ptr as *mut svn_string_t
    }

    /// Get bytes as slice
    pub fn as_bytes(&self) -> &[u8] {
        let ptr = unsafe { (*self.ptr).data };
        let len = unsafe { (*self.ptr).len as usize };
        unsafe { std::slice::from_raw_parts(ptr as *const u8, len) }
    }

    /// Convert to owned Vec<u8>
    pub fn to_bytes(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }

    /// Try to interpret as UTF-8 string
    pub fn as_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(self.as_bytes())
    }

    /// Convert to owned String, replacing invalid UTF-8
    pub fn to_string_lossy(&self) -> std::string::String {
        std::string::String::from_utf8_lossy(self.as_bytes()).into_owned()
    }
}

impl<'pool> From<&[u8]> for BStr<'pool> {
    fn from(data: &[u8]) -> Self {
        // This is a bit tricky - we need a pool to create the SVN string
        // For now, create a global pool (not ideal, but works for compatibility)
        let pool = Box::leak(Box::new(apr::Pool::new()));
        Self::from_bytes(data, pool)
    }
}

impl<'pool> From<&str> for BStr<'pool> {
    fn from(s: &str) -> Self {
        Self::from(s.as_bytes())
    }
}

// For backwards compatibility, keep the old String name as an alias
pub type String<'pool> = BStr<'pool>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bstr_from_bytes() {
        let pool = apr::Pool::new();
        let data = b"Hello, world!";
        let bstr = BStr::from_bytes(data, &pool);
        assert_eq!(bstr.as_bytes(), data);
    }

    #[test]
    fn test_bstr_from_str() {
        let pool = apr::Pool::new();
        let text = "Hello, Rust!";
        let bstr = BStr::from_str(text, &pool);
        assert_eq!(bstr.as_bytes(), text.as_bytes());
    }

    #[test]
    fn test_bstr_len() {
        let pool = apr::Pool::new();
        let data = b"Test data";
        let bstr = BStr::from_bytes(data, &pool);
        assert_eq!(bstr.as_bytes().len(), data.len());
    }

    #[test]
    fn test_bstr_is_empty() {
        let pool = apr::Pool::new();
        let empty = BStr::from_bytes(b"", &pool);
        assert!(empty.as_bytes().is_empty());

        let non_empty = BStr::from_bytes(b"data", &pool);
        assert!(!non_empty.as_bytes().is_empty());
    }

    #[test]
    fn test_bstr_display() {
        let pool = apr::Pool::new();
        let text = "Display test";
        let bstr = BStr::from_str(text, &pool);
        assert_eq!(bstr.to_string_lossy(), text);
    }

    #[test]
    fn test_bstr_debug() {
        let pool = apr::Pool::new();
        let text = "Debug test";
        let bstr = BStr::from_str(text, &pool);
        let debug_str = bstr.to_string_lossy();
        assert!(debug_str.contains("Debug test"));
    }
}
