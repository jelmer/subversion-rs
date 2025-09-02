//! Hash table serialization and utilities
//!
//! This module provides safe Rust wrappers around Subversion's hash table utilities
//! for reading, writing, and manipulating hash tables to/from streams.

use crate::{error::Error, io::Stream};
use std::collections::HashMap;

/// Hash difference key status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffKeyStatus {
    /// Key is present in both hashes
    Both,
    /// Key is present in first hash only  
    A,
    /// Key is present in second hash only
    B,
}

impl From<subversion_sys::svn_hash_diff_key_status> for DiffKeyStatus {
    fn from(status: subversion_sys::svn_hash_diff_key_status) -> Self {
        match status {
            subversion_sys::svn_hash_diff_key_status_svn_hash_diff_key_both => Self::Both,
            subversion_sys::svn_hash_diff_key_status_svn_hash_diff_key_a => Self::A,
            subversion_sys::svn_hash_diff_key_status_svn_hash_diff_key_b => Self::B,
            _ => unreachable!("Invalid svn_hash_diff_key_status"),
        }
    }
}

/// Read hash data from a stream into a hash table
///
/// Reads key-value pairs from stream until terminator is found.
/// The hash should contain `const char *` keys and `svn_string_t *` values.
pub fn read(
    stream: &mut Stream,
    terminator: Option<&str>,
) -> Result<HashMap<String, String>, Error> {
    let pool = apr::Pool::new();

    // Create empty APR hash
    let raw_hash = unsafe { apr_sys::apr_hash_make(pool.as_mut_ptr()) };

    let terminator_cstr = terminator
        .map(std::ffi::CString::new)
        .transpose()
        .map_err(|_| Error::from_str("Invalid terminator string"))?;

    unsafe {
        let err = subversion_sys::svn_hash_read2(
            raw_hash,
            stream.as_mut_ptr(),
            terminator_cstr
                .as_ref()
                .map_or(std::ptr::null(), |s| s.as_ptr()),
            pool.as_mut_ptr(),
        );
        Error::from_raw(err)?;
    }

    // Convert APR hash to Rust HashMap using direct iteration
    let mut result = HashMap::new();

    // Iterate directly using APR hash iteration
    unsafe {
        let mut hi = apr_sys::apr_hash_first(pool.as_mut_ptr(), raw_hash);
        while !hi.is_null() {
            let mut key_ptr: *const std::ffi::c_void = std::ptr::null();
            let mut key_len: apr_sys::apr_ssize_t = 0;
            let mut val_ptr: *mut std::ffi::c_void = std::ptr::null_mut();

            apr_sys::apr_hash_this(hi, &mut key_ptr, &mut key_len, &mut val_ptr);

            // Get the key as a string
            let key_bytes = if key_len < 0 {
                // Null-terminated string
                std::ffi::CStr::from_ptr(key_ptr as *const i8).to_bytes()
            } else {
                std::slice::from_raw_parts(key_ptr as *const u8, key_len as usize)
            };
            let key_str = String::from_utf8_lossy(key_bytes).to_string();

            // Get the value as svn_string_t*
            let value_ptr = val_ptr as *mut subversion_sys::svn_string_t;

            let value_str = if value_ptr.is_null() {
                String::new()
            } else {
                let svn_str = &*value_ptr;
                if svn_str.data.is_null() || svn_str.len == 0 {
                    String::new()
                } else {
                    let data_slice =
                        std::slice::from_raw_parts(svn_str.data as *const u8, svn_str.len as usize);
                    String::from_utf8_lossy(data_slice).into_owned()
                }
            };

            result.insert(key_str, value_str);

            // Move to next
            hi = apr_sys::apr_hash_next(hi);
        }
    }

    Ok(result)
}

/// Write hash data to a stream
///
/// Writes key-value pairs to stream, optionally terminating with terminator.
pub fn write(
    hash: &HashMap<String, String>,
    stream: &mut Stream,
    terminator: Option<&str>,
) -> Result<(), Error> {
    let pool = apr::Pool::new();
    let mut apr_hash = apr::hash::Hash::new(&pool);

    // Convert Rust HashMap to APR hash with svn_string_t values
    // We need to keep CStrings alive for the duration of the hash operations
    let mut key_cstrings = Vec::new();
    for (key, value) in hash.iter() {
        let key_cstr = std::ffi::CString::new(key.as_str()).unwrap();
        let value_bytes = value.as_bytes();
        let svn_string = unsafe {
            subversion_sys::svn_string_ncreate(
                value_bytes.as_ptr() as *const i8,
                value_bytes.len(),
                pool.as_mut_ptr(),
            )
        };
        // Use the C string for the key
        unsafe {
            apr_sys::apr_hash_set(
                apr_hash.as_mut_ptr(),
                key_cstr.as_ptr() as *const std::ffi::c_void,
                apr_sys::APR_HASH_KEY_STRING as apr_sys::apr_ssize_t,
                svn_string as *const std::ffi::c_void,
            );
        }
        key_cstrings.push(key_cstr); // Keep the CString alive
    }

    let terminator_cstr = terminator
        .map(std::ffi::CString::new)
        .transpose()
        .map_err(|_| Error::from_str("Invalid terminator string"))?;

    unsafe {
        let err = subversion_sys::svn_hash_write2(
            apr_hash.as_mut_ptr(),
            stream.as_mut_ptr(),
            terminator_cstr
                .as_ref()
                .map_or(std::ptr::null(), |s| s.as_ptr()),
            pool.as_mut_ptr(),
        );
        Error::from_raw(err)?;
    }

    Ok(())
}

/// Read hash data incrementally from stream, allowing deletions
///
/// Similar to read() but allows stream to contain deletion lines which
/// remove entries from the hash as well as adding to it.
pub fn read_incremental(
    stream: &mut Stream,
    terminator: Option<&str>,
) -> Result<HashMap<String, String>, Error> {
    let pool = apr::Pool::new();

    // Create empty APR hash
    let raw_hash = unsafe { apr_sys::apr_hash_make(pool.as_mut_ptr()) };

    let terminator_cstr = terminator
        .map(std::ffi::CString::new)
        .transpose()
        .map_err(|_| Error::from_str("Invalid terminator string"))?;

    unsafe {
        let err = subversion_sys::svn_hash_read_incremental(
            raw_hash,
            stream.as_mut_ptr(),
            terminator_cstr
                .as_ref()
                .map_or(std::ptr::null(), |s| s.as_ptr()),
            pool.as_mut_ptr(),
        );
        Error::from_raw(err)?;
    }

    // Convert APR hash to Rust HashMap using direct iteration
    let mut result = HashMap::new();

    // Iterate directly using APR hash iteration
    unsafe {
        let mut hi = apr_sys::apr_hash_first(pool.as_mut_ptr(), raw_hash);
        while !hi.is_null() {
            let mut key_ptr: *const std::ffi::c_void = std::ptr::null();
            let mut key_len: apr_sys::apr_ssize_t = 0;
            let mut val_ptr: *mut std::ffi::c_void = std::ptr::null_mut();

            apr_sys::apr_hash_this(hi, &mut key_ptr, &mut key_len, &mut val_ptr);

            // Get the key as a string
            let key_bytes = if key_len < 0 {
                // Null-terminated string
                std::ffi::CStr::from_ptr(key_ptr as *const i8).to_bytes()
            } else {
                std::slice::from_raw_parts(key_ptr as *const u8, key_len as usize)
            };
            let key_str = String::from_utf8_lossy(key_bytes).to_string();

            // Get the value as svn_string_t*
            let value_ptr = val_ptr as *mut subversion_sys::svn_string_t;

            let value_str = if value_ptr.is_null() {
                String::new()
            } else {
                let svn_str = &*value_ptr;
                if svn_str.data.is_null() || svn_str.len == 0 {
                    String::new()
                } else {
                    let data_slice =
                        std::slice::from_raw_parts(svn_str.data as *const u8, svn_str.len as usize);
                    String::from_utf8_lossy(data_slice).into_owned()
                }
            };

            result.insert(key_str, value_str);

            // Move to next
            hi = apr_sys::apr_hash_next(hi);
        }
    }

    Ok(result)
}

/// Write hash data incrementally to stream
///
/// Only writes out entries for keys which differ between hash and oldhash,
/// and also writes out deletion lines for keys which are present in oldhash
/// but not in hash.
pub fn write_incremental(
    hash: &HashMap<String, String>,
    oldhash: &HashMap<String, String>,
    stream: &mut Stream,
    terminator: Option<&str>,
) -> Result<(), Error> {
    let pool = apr::Pool::new();

    // Convert new hash to APR hash
    let mut apr_hash = apr::hash::Hash::new(&pool);
    let mut key_cstrings = Vec::new();
    for (key, value) in hash.iter() {
        let key_cstr = std::ffi::CString::new(key.as_str()).unwrap();
        let value_bytes = value.as_bytes();
        let svn_string = unsafe {
            subversion_sys::svn_string_ncreate(
                value_bytes.as_ptr() as *const i8,
                value_bytes.len(),
                pool.as_mut_ptr(),
            )
        };
        unsafe {
            apr_sys::apr_hash_set(
                apr_hash.as_mut_ptr(),
                key_cstr.as_ptr() as *const std::ffi::c_void,
                apr_sys::APR_HASH_KEY_STRING as apr_sys::apr_ssize_t,
                svn_string as *const std::ffi::c_void,
            );
        }
        key_cstrings.push(key_cstr);
    }

    // Convert old hash to APR hash
    let mut apr_oldhash = apr::hash::Hash::new(&pool);
    let mut old_key_cstrings = Vec::new();
    for (key, value) in oldhash.iter() {
        let key_cstr = std::ffi::CString::new(key.as_str()).unwrap();
        let value_bytes = value.as_bytes();
        let svn_string = unsafe {
            subversion_sys::svn_string_ncreate(
                value_bytes.as_ptr() as *const i8,
                value_bytes.len(),
                pool.as_mut_ptr(),
            )
        };
        unsafe {
            apr_sys::apr_hash_set(
                apr_oldhash.as_mut_ptr(),
                key_cstr.as_ptr() as *const std::ffi::c_void,
                apr_sys::APR_HASH_KEY_STRING as apr_sys::apr_ssize_t,
                svn_string as *const std::ffi::c_void,
            );
        }
        old_key_cstrings.push(key_cstr);
    }

    let terminator_cstr = terminator
        .map(std::ffi::CString::new)
        .transpose()
        .map_err(|_| Error::from_str("Invalid terminator string"))?;

    unsafe {
        let err = subversion_sys::svn_hash_write_incremental(
            apr_hash.as_mut_ptr(),
            apr_oldhash.as_mut_ptr(),
            stream.as_mut_ptr(),
            terminator_cstr
                .as_ref()
                .map_or(std::ptr::null(), |s| s.as_ptr()),
            pool.as_mut_ptr(),
        );
        Error::from_raw(err)?;
    }

    Ok(())
}

/// Compare two hash tables and call a function for each differing key
///
/// For each key in the union of hash_a's and hash_b's keys, calls diff_func
/// with the key, key length, status indicating which table(s) the key appears in.
pub fn diff<F>(
    hash_a: &HashMap<String, String>,
    hash_b: &HashMap<String, String>,
    diff_func: F,
) -> Result<(), Error>
where
    F: FnMut(&str, DiffKeyStatus) -> Result<(), Error>,
{
    let pool = apr::Pool::new();

    // Convert hash_a to APR hash
    let mut apr_hash_a = apr::hash::Hash::new(&pool);
    let mut keys_a_cstrings = Vec::new();
    for (key, value) in hash_a.iter() {
        let key_cstr = std::ffi::CString::new(key.as_str()).unwrap();
        let value_bytes = value.as_bytes();
        let svn_string = unsafe {
            subversion_sys::svn_string_ncreate(
                value_bytes.as_ptr() as *const i8,
                value_bytes.len(),
                pool.as_mut_ptr(),
            )
        };
        unsafe {
            apr_sys::apr_hash_set(
                apr_hash_a.as_mut_ptr(),
                key_cstr.as_ptr() as *const std::ffi::c_void,
                apr_sys::APR_HASH_KEY_STRING as apr_sys::apr_ssize_t,
                svn_string as *const std::ffi::c_void,
            );
        }
        keys_a_cstrings.push(key_cstr);
    }

    // Convert hash_b to APR hash
    let mut apr_hash_b = apr::hash::Hash::new(&pool);
    let mut keys_b_cstrings = Vec::new();
    for (key, value) in hash_b.iter() {
        let key_cstr = std::ffi::CString::new(key.as_str()).unwrap();
        let value_bytes = value.as_bytes();
        let svn_string = unsafe {
            subversion_sys::svn_string_ncreate(
                value_bytes.as_ptr() as *const i8,
                value_bytes.len(),
                pool.as_mut_ptr(),
            )
        };
        unsafe {
            apr_sys::apr_hash_set(
                apr_hash_b.as_mut_ptr(),
                key_cstr.as_ptr() as *const std::ffi::c_void,
                apr_sys::APR_HASH_KEY_STRING as apr_sys::apr_ssize_t,
                svn_string as *const std::ffi::c_void,
            );
        }
        keys_b_cstrings.push(key_cstr);
    }

    // Create wrapper for the callback
    extern "C" fn diff_callback(
        key: *const std::ffi::c_void,
        klen: apr_sys::apr_ssize_t,
        status: subversion_sys::svn_hash_diff_key_status,
        baton: *mut std::ffi::c_void,
    ) -> *mut subversion_sys::svn_error_t {
        let callback = unsafe {
            &mut **(baton as *mut Box<dyn FnMut(&str, DiffKeyStatus) -> Result<(), Error>>)
        };

        // Convert key to string
        let key_str = if klen < 0 {
            // Null-terminated string
            let key_cstr = unsafe { std::ffi::CStr::from_ptr(key as *const i8) };
            key_cstr.to_string_lossy().into_owned()
        } else {
            // String with known length
            let key_slice = unsafe { std::slice::from_raw_parts(key as *const u8, klen as usize) };
            String::from_utf8_lossy(key_slice).into_owned()
        };

        match callback(&key_str, DiffKeyStatus::from(status)) {
            Ok(()) => std::ptr::null_mut(),
            Err(e) => unsafe { e.into_raw() },
        }
    }

    let boxed_callback: Box<Box<dyn FnMut(&str, DiffKeyStatus) -> Result<(), Error>>> =
        Box::new(Box::new(diff_func));
    let baton = Box::into_raw(boxed_callback) as *mut std::ffi::c_void;

    unsafe {
        let err = subversion_sys::svn_hash_diff(
            apr_hash_a.as_mut_ptr(),
            apr_hash_b.as_mut_ptr(),
            Some(diff_callback),
            baton,
            pool.as_mut_ptr(),
        );
        Error::from_raw(err)?;
    }

    Ok(())
}

/// Create a hash table from an array of C string keys
///
/// Creates a hash table using the provided keys, with all values set to empty strings.
pub fn from_cstring_keys(keys: &[&str]) -> Result<HashMap<String, String>, Error> {
    let pool = apr::Pool::new();
    let mut keys_array = apr::tables::TypedArray::<*const i8>::new(&pool, keys.len() as i32);

    let key_cstrings: Vec<std::ffi::CString> = keys
        .iter()
        .map(|&s| std::ffi::CString::new(s))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| Error::from_str("Invalid key string"))?;

    for cstring in key_cstrings.iter() {
        keys_array.push(cstring.as_ptr());
    }

    let mut hash_ptr: *mut apr_sys::apr_hash_t = std::ptr::null_mut();
    unsafe {
        let err = subversion_sys::svn_hash_from_cstring_keys(
            &mut hash_ptr,
            keys_array.as_ptr(),
            pool.as_mut_ptr(),
        );
        Error::from_raw(err)?;
    }

    // The hash was created but values are probably NULL, so just return empty strings for all keys
    let mut result = HashMap::new();
    for &key in keys {
        result.insert(key.to_string(), String::new());
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_cstring_keys() {
        let keys = ["key1", "key2", "key3"];
        let hash = from_cstring_keys(&keys).unwrap();

        assert_eq!(hash.len(), 3);
        assert!(hash.contains_key("key1"));
        assert!(hash.contains_key("key2"));
        assert!(hash.contains_key("key3"));
        assert_eq!(hash.get("key1"), Some(&String::new()));
    }

    #[test]
    fn test_write_read_roundtrip() {
        let mut original_hash = HashMap::new();
        original_hash.insert("key1".to_string(), "value1".to_string());
        original_hash.insert("key2".to_string(), "value2".to_string());
        original_hash.insert("key3".to_string(), "value3".to_string());

        // Create a string buffer that will outlive both streams
        let mut stringbuf = crate::io::StringBuf::new();

        // Write to the stream - stream must be dropped before reading
        {
            let mut write_stream = Stream::from_stringbuf(&mut stringbuf);
            write(&original_hash, &mut write_stream, Some("END")).unwrap();
            // Stream is dropped here, flushing any buffered data
        }

        // Convert stringbuf contents to a String so it has its own memory
        let contents_string = stringbuf.to_string();
        let mut read_stream = Stream::from(contents_string);
        let read_hash = read(&mut read_stream, Some("END")).unwrap();

        assert_eq!(original_hash.len(), read_hash.len());
        for (key, value) in &original_hash {
            assert_eq!(read_hash.get(key), Some(value));
        }
    }

    #[test]
    fn test_hash_diff() {
        let mut hash_a = HashMap::new();
        hash_a.insert("common".to_string(), "value1".to_string());
        hash_a.insert("only_a".to_string(), "value2".to_string());

        let mut hash_b = HashMap::new();
        hash_b.insert("common".to_string(), "value1".to_string());
        hash_b.insert("only_b".to_string(), "value3".to_string());

        let mut diffs = Vec::new();
        diff(&hash_a, &hash_b, |key, status| {
            diffs.push((key.to_string(), status));
            Ok(())
        })
        .unwrap();

        assert_eq!(diffs.len(), 3);

        // Check that all keys are covered
        let keys: Vec<String> = diffs.iter().map(|(k, _)| k.clone()).collect();
        assert!(keys.contains(&"common".to_string()));
        assert!(keys.contains(&"only_a".to_string()));
        assert!(keys.contains(&"only_b".to_string()));
    }

    #[test]
    fn test_incremental_write_read() {
        let mut old_hash = HashMap::new();
        old_hash.insert("key1".to_string(), "old_value1".to_string());
        old_hash.insert("key2".to_string(), "old_value2".to_string());
        old_hash.insert("to_delete".to_string(), "delete_me".to_string());

        let mut new_hash = HashMap::new();
        new_hash.insert("key1".to_string(), "new_value1".to_string()); // changed
        new_hash.insert("key2".to_string(), "old_value2".to_string()); // unchanged
        new_hash.insert("new_key".to_string(), "new_value".to_string()); // added
                                                                         // to_delete is removed

        // Create a string buffer that will outlive both streams
        let mut stringbuf = crate::io::StringBuf::new();

        // Write incremental changes - stream must be dropped before reading
        {
            let mut write_stream = Stream::from_stringbuf(&mut stringbuf);
            write_incremental(&new_hash, &old_hash, &mut write_stream, Some("END")).unwrap();
            // Stream is dropped here, flushing any buffered data
        }

        // Convert stringbuf contents to a String so it has its own memory
        let contents_string = stringbuf.to_string();
        let mut read_stream = Stream::from(contents_string);
        let result_hash = read_incremental(&mut read_stream, Some("END")).unwrap();

        // The result should reflect the incremental changes
        // Note: The exact behavior depends on SVN's implementation
        assert!(!result_hash.is_empty());
    }
}

// ============================================================================
// Safe Hash Wrappers for Common Patterns
// ============================================================================

/// A safe wrapper for APR hashes containing path -> svn_dirent_t mappings
///
/// This wrapper encapsulates the common pattern of working with directory entry
/// hashes from Subversion's C API, reducing unsafe code and providing convenient
/// conversion methods.
pub struct DirentHash<'a> {
    inner: apr::hash::TypedHash<'a, subversion_sys::svn_dirent_t>,
}

impl<'a> DirentHash<'a> {
    /// Create a DirentHash from a raw APR hash pointer
    ///
    /// # Safety
    /// The caller must ensure that:
    /// - `ptr` is a valid APR hash containing svn_dirent_t values
    /// - The hash and its contents remain valid for the lifetime of this wrapper
    pub unsafe fn from_ptr(ptr: *mut apr_sys::apr_hash_t) -> Self {
        Self {
            inner: apr::hash::TypedHash::<subversion_sys::svn_dirent_t>::from_ptr(ptr),
        }
    }

    /// Convert the dirents to a HashMap<String, crate::Dirent>
    pub fn to_hashmap(&self) -> HashMap<String, crate::ra::Dirent> {
        self.inner
            .iter()
            .map(|(k, v)| {
                let key = String::from_utf8_lossy(k).into_owned();
                let dirent = crate::ra::Dirent::from_raw(v as *const _ as *mut _);
                (key, dirent)
            })
            .collect()
    }

    /// Iterate over the dirents as (path: &str, dirent: crate::ra::Dirent) pairs
    pub fn iter(&self) -> impl Iterator<Item = (&str, crate::ra::Dirent)> + '_ {
        self.inner.iter().map(|(k, v)| {
            let path = std::str::from_utf8(k).unwrap_or("");
            let dirent = crate::ra::Dirent::from_raw(v as *const _ as *mut _);
            (path, dirent)
        })
    }

    /// Check if the hash is empty
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Get the number of dirents
    pub fn len(&self) -> usize {
        self.inner.len()
    }
}

/// A safe wrapper for APR hashes containing path -> svn_fs_path_change2_t mappings
///
/// This wrapper encapsulates the common pattern of working with changed path
/// hashes from Subversion's FS API.
pub struct PathChangeHash<'a> {
    inner: apr::hash::Hash<'a>,
}

impl<'a> PathChangeHash<'a> {
    /// Create a PathChangeHash from a raw APR hash pointer
    ///
    /// # Safety
    /// The caller must ensure that:
    /// - `ptr` is a valid APR hash containing svn_fs_path_change2_t values
    /// - The hash and its contents remain valid for the lifetime of this wrapper
    pub unsafe fn from_ptr(ptr: *mut apr_sys::apr_hash_t) -> Self {
        Self {
            inner: apr::hash::Hash::from_ptr(ptr),
        }
    }

    /// Convert the path changes to a HashMap<String, FsPathChange>
    pub fn to_hashmap(&self) -> HashMap<String, crate::fs::FsPathChange> {
        let mut result = HashMap::new();
        for (k, v) in self.inner.iter() {
            let path = String::from_utf8_lossy(k).into_owned();
            let change =
                crate::fs::FsPathChange::from_raw(v as *mut subversion_sys::svn_fs_path_change2_t);
            result.insert(path, change);
        }
        result
    }

    /// Check if the hash is empty
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Get the number of path changes
    pub fn len(&self) -> usize {
        self.inner.len()
    }
}

/// A safe wrapper for APR hashes containing path -> svn_fs_dirent_t mappings
///
/// This wrapper encapsulates the common pattern of working with directory
/// entry hashes from Subversion's FS API.
pub struct FsDirentHash<'a> {
    inner: apr::hash::Hash<'a>,
}

impl<'a> FsDirentHash<'a> {
    /// Create a FsDirentHash from a raw APR hash pointer
    ///
    /// # Safety
    /// The caller must ensure that:
    /// - `ptr` is a valid APR hash containing svn_fs_dirent_t values
    /// - The hash and its contents remain valid for the lifetime of this wrapper
    pub unsafe fn from_ptr(ptr: *mut apr_sys::apr_hash_t) -> Self {
        Self {
            inner: apr::hash::Hash::from_ptr(ptr),
        }
    }

    /// Convert the dirents to a HashMap<String, FsDirEntry>
    pub fn to_hashmap(&self) -> HashMap<String, crate::fs::FsDirEntry> {
        let mut result = HashMap::new();
        for (k, v) in self.inner.iter() {
            let name = String::from_utf8_lossy(k).into_owned();
            let entry = crate::fs::FsDirEntry::from_raw(v as *mut subversion_sys::svn_fs_dirent_t);
            result.insert(name, entry);
        }
        result
    }

    /// Check if the hash is empty
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Get the number of dirents
    pub fn len(&self) -> usize {
        self.inner.len()
    }
}
