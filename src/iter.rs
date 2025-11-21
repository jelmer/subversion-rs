//! Iterator utilities for APR data structures
//!
//! This module provides safe Rust wrappers around Subversion's iterator functions
//! that work with APR hash tables and arrays.

use crate::{with_tmp_pool, Error};

/// Error type for iterator break - used internally by SVN
pub const SVN_ERR_ITER_BREAK: i32 = 200020;

/// Result type for iterator callbacks - returning IterBreak stops iteration
pub type IterResult<T> = Result<T, IterBreak>;

/// Special error type for breaking out of iteration
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IterBreak;

impl std::fmt::Display for IterBreak {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "iteration break")
    }
}

impl std::error::Error for IterBreak {}

/// Iterate over an APR hash table with a callback function.
///
/// The callback receives the key (as bytes), key length, and value pointer.
/// Return `IterResult::Ok(())` to continue iteration or `Err(IterBreak)` to stop.
///
/// Returns `Ok(true)` if iteration completed normally, `Ok(false)` if broken early.
pub unsafe fn iter_hash<F>(hash: *mut apr_sys::apr_hash_t, mut callback: F) -> Result<bool, Error>
where
    F: FnMut(&[u8], *mut std::ffi::c_void) -> IterResult<()>,
{
    with_tmp_pool(|pool| {
        let mut completed: subversion_sys::svn_boolean_t = 0;
        let mut callback_wrapper =
            |key: &[u8], value: *mut std::ffi::c_void| -> IterResult<()> { callback(key, value) };

        // Create C-compatible callback wrapper
        extern "C" fn c_callback(
            baton: *mut std::ffi::c_void,
            key: *const std::ffi::c_void,
            klen: apr_sys::apr_ssize_t,
            val: *mut std::ffi::c_void,
            _pool: *mut apr_sys::apr_pool_t,
        ) -> *mut subversion_sys::svn_error_t {
            let callback = unsafe {
                &mut *(baton as *mut &mut dyn FnMut(&[u8], *mut std::ffi::c_void) -> IterResult<()>)
            };

            let key_slice = unsafe { std::slice::from_raw_parts(key as *const u8, klen as usize) };

            match callback(key_slice, val) {
                Ok(()) => std::ptr::null_mut(),
                Err(IterBreak) => unsafe { subversion_sys::svn_iter__break() },
            }
        }

        let callback_trait: &mut dyn FnMut(&[u8], *mut std::ffi::c_void) -> IterResult<()> =
            &mut callback_wrapper;
        let baton = &callback_trait as *const _ as *mut std::ffi::c_void;

        unsafe {
            let err = subversion_sys::svn_iter_apr_hash(
                &mut completed,
                hash,
                Some(c_callback),
                baton,
                pool.as_mut_ptr(),
            );
            // Check if it's the special ITER_BREAK error - if so, don't wrap it
            if !err.is_null()
                && (*err).apr_err == subversion_sys::svn_errno_t_SVN_ERR_ITER_BREAK as i32
            {
                // Iteration was interrupted by break - this is normal, not an error
                // The error is a static singleton that must not be cleared
            } else {
                Error::from_raw(err)?;
            }
        }

        Ok(completed != 0)
    })
}

/// Iterate over an APR array with a callback function.
///
/// The callback receives a pointer to each array item.
/// Return `IterResult::Ok(())` to continue iteration or `Err(IterBreak)` to stop.
///
/// Returns `Ok(true)` if iteration completed normally, `Ok(false)` if broken early.
pub unsafe fn iter_array<F>(
    array: *const apr_sys::apr_array_header_t,
    mut callback: F,
) -> Result<bool, Error>
where
    F: FnMut(*mut std::ffi::c_void) -> IterResult<()>,
{
    with_tmp_pool(|pool| {
        let mut completed: subversion_sys::svn_boolean_t = 0;
        let mut callback_wrapper =
            |item: *mut std::ffi::c_void| -> IterResult<()> { callback(item) };

        // Create C-compatible callback wrapper
        extern "C" fn c_callback(
            baton: *mut std::ffi::c_void,
            item: *mut std::ffi::c_void,
            _pool: *mut apr_sys::apr_pool_t,
        ) -> *mut subversion_sys::svn_error_t {
            let callback = unsafe {
                &mut *(baton as *mut &mut dyn FnMut(*mut std::ffi::c_void) -> IterResult<()>)
            };

            match callback(item) {
                Ok(()) => std::ptr::null_mut(),
                Err(IterBreak) => unsafe { subversion_sys::svn_iter__break() },
            }
        }

        let callback_trait: &mut dyn FnMut(*mut std::ffi::c_void) -> IterResult<()> =
            &mut callback_wrapper;
        let baton = &callback_trait as *const _ as *mut std::ffi::c_void;

        unsafe {
            let err = subversion_sys::svn_iter_apr_array(
                &mut completed,
                array,
                Some(c_callback),
                baton,
                pool.as_mut_ptr(),
            );
            // Check if it's the special ITER_BREAK error - if so, don't wrap it
            if !err.is_null()
                && (*err).apr_err == subversion_sys::svn_errno_t_SVN_ERR_ITER_BREAK as i32
            {
                // Iteration was interrupted by break - this is normal, not an error
                // The error is a static singleton that must not be cleared
            } else {
                Error::from_raw(err)?;
            }
        }

        Ok(completed != 0)
    })
}

/// Helper function to break out of iteration - for use in callbacks
pub fn break_iteration() -> IterBreak {
    IterBreak
}

/// Extension trait to add iteration methods to APR hash tables
pub trait HashIterExt {
    /// Iterate over hash entries with a callback
    fn iter_entries<F>(&self, callback: F) -> Result<bool, Error>
    where
        F: FnMut(&[u8], *mut std::ffi::c_void) -> IterResult<()>;
}

impl HashIterExt for apr::hash::Hash<'_> {
    fn iter_entries<F>(&self, callback: F) -> Result<bool, Error>
    where
        F: FnMut(&[u8], *mut std::ffi::c_void) -> IterResult<()>,
    {
        unsafe { iter_hash(self.as_ptr() as *mut _, callback) }
    }
}

/// Extension trait to add iteration methods to APR arrays
pub trait ArrayIterExt {
    /// Iterate over array items with a callback
    fn iter_items<F>(&self, callback: F) -> Result<bool, Error>
    where
        F: FnMut(*mut std::ffi::c_void) -> IterResult<()>;
}

impl<T: Copy> ArrayIterExt for apr::tables::TypedArray<'_, T> {
    fn iter_items<F>(&self, callback: F) -> Result<bool, Error>
    where
        F: FnMut(*mut std::ffi::c_void) -> IterResult<()>,
    {
        unsafe { iter_array(self.as_ptr(), callback) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_basic_hash_iteration() {
        let pool = apr::Pool::new();

        // Create a simple hash with integer values (which work with IntoStoredPointer)
        let mut hash = apr::hash::Hash::new(&pool);
        let val1 = 42i32;
        let val2 = 84i32;
        unsafe {
            hash.insert(b"key1", &val1 as *const _ as *mut std::ffi::c_void);
        }
        unsafe {
            hash.insert(b"key2", &val2 as *const _ as *mut std::ffi::c_void);
        }

        let mut collected = HashMap::new();
        let completed = unsafe {
            iter_hash(hash.as_ptr() as *mut _, |key, value| {
                let key_str = String::from_utf8_lossy(key).into_owned();
                let value_int = *(value as *const i32);
                collected.insert(key_str, value_int);
                Ok(())
            })
        }
        .unwrap();

        assert!(completed);
        assert_eq!(collected.len(), 2);
        assert_eq!(collected.get("key1"), Some(&42));
        assert_eq!(collected.get("key2"), Some(&84));
    }

    #[test]
    fn test_basic_hash_iteration_break() {
        let pool = apr::Pool::new();

        // Create a simple hash with integer values
        let mut hash = apr::hash::Hash::new(&pool);
        let val1 = 42i32;
        let val2 = 84i32;
        unsafe {
            hash.insert(b"key1", &val1 as *const _ as *mut std::ffi::c_void);
        }
        unsafe {
            hash.insert(b"key2", &val2 as *const _ as *mut std::ffi::c_void);
        }

        let mut count = 0;
        let completed = unsafe {
            iter_hash(hash.as_ptr() as *mut _, |_key, _value| {
                count += 1;
                if count >= 1 {
                    Err(break_iteration())
                } else {
                    Ok(())
                }
            })
        }
        .unwrap();

        assert!(!completed); // Should be false because we broke early
        assert_eq!(count, 1); // Should have stopped after first item
    }

    #[test]
    fn test_array_iteration() {
        let pool = apr::Pool::new();

        // Create an array with some test data
        let mut array = apr::tables::TypedArray::<i32>::new(&pool, 10);
        array.push(10);
        array.push(20);
        array.push(30);

        let mut collected = Vec::new();
        let completed = array
            .iter_items(|item| {
                let value = unsafe { *(item as *const i32) };
                collected.push(value);
                Ok(())
            })
            .unwrap();

        assert!(completed);
        assert_eq!(collected, vec![10, 20, 30]);
    }

    #[test]
    fn test_array_iteration_break() {
        let pool = apr::Pool::new();

        // Create an array with some test data
        let mut array = apr::tables::TypedArray::<i32>::new(&pool, 10);
        array.push(10);
        array.push(20);
        array.push(30);

        let mut collected = Vec::new();
        let completed = array
            .iter_items(|item| {
                let value = unsafe { *(item as *const i32) };
                collected.push(value);
                if collected.len() >= 2 {
                    Err(break_iteration())
                } else {
                    Ok(())
                }
            })
            .unwrap();

        assert!(!completed); // Should be false because we broke early
        assert_eq!(collected, vec![10, 20]); // Should have stopped after 2 items
    }
}
