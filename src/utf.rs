//! UTF-8 conversion utilities
//!
//! This module provides functionality for converting between UTF-8 and native/locale encodings
//! using Subversion's UTF-8 handling routines.

use crate::{with_tmp_pool, Error};

/// Initialize the UTF-8 encoding/decoding subsystem
///
/// # Arguments
/// * `assume_native_utf8` - If true, assume the native encoding is UTF-8
///
/// This should be called early in the application lifecycle.
pub fn initialize(assume_native_utf8: bool) {
    with_tmp_pool(|pool| unsafe {
        subversion_sys::svn_utf_initialize2(
            if assume_native_utf8 { 1 } else { 0 },
            pool.as_mut_ptr(),
        );
    })
}

/// Convert a C string from native encoding to UTF-8
///
/// # Arguments
/// * `src` - Source string in native encoding
///
/// # Returns
/// The string converted to UTF-8, or an error if conversion fails.
pub fn cstring_to_utf8(src: &str) -> Result<String, Error<'static>> {
    with_tmp_pool(|pool| unsafe {
        let src_cstr = std::ffi::CString::new(src)?;
        let mut dest_ptr: *const std::ffi::c_char = std::ptr::null();

        let err = subversion_sys::svn_utf_cstring_to_utf8(
            &mut dest_ptr,
            src_cstr.as_ptr(),
            pool.as_mut_ptr(),
        );
        Error::from_raw(err)?;

        if dest_ptr.is_null() {
            return Err(Error::from_message("UTF-8 conversion returned null"));
        }

        let dest_cstr = std::ffi::CStr::from_ptr(dest_ptr);
        Ok(dest_cstr.to_str()?.to_string())
    })
}

/// Convert a C string from native encoding to UTF-8 with explicit source encoding
///
/// # Arguments
/// * `src` - Source string in specified encoding
/// * `frompage` - Source encoding name (e.g., "ISO-8859-1")
///
/// # Returns
/// The string converted to UTF-8, or an error if conversion fails.
pub fn cstring_to_utf8_ex(src: &str, frompage: &str) -> Result<String, Error<'static>> {
    with_tmp_pool(|pool| unsafe {
        let src_cstr = std::ffi::CString::new(src)?;
        let frompage_cstr = std::ffi::CString::new(frompage)?;
        let mut dest_ptr: *const std::ffi::c_char = std::ptr::null();

        let err = subversion_sys::svn_utf_cstring_to_utf8_ex2(
            &mut dest_ptr,
            src_cstr.as_ptr(),
            frompage_cstr.as_ptr(),
            pool.as_mut_ptr(),
        );
        Error::from_raw(err)?;

        if dest_ptr.is_null() {
            return Err(Error::from_message("UTF-8 conversion returned null"));
        }

        let dest_cstr = std::ffi::CStr::from_ptr(dest_ptr);
        Ok(dest_cstr.to_str()?.to_string())
    })
}

/// Convert a UTF-8 string to native encoding
///
/// # Arguments
/// * `src` - Source string in UTF-8
///
/// # Returns
/// The string converted to native encoding, or an error if conversion fails.
pub fn cstring_from_utf8(src: &str) -> Result<String, Error<'static>> {
    with_tmp_pool(|pool| unsafe {
        let src_cstr = std::ffi::CString::new(src)?;
        let mut dest_ptr: *const std::ffi::c_char = std::ptr::null();

        let err = subversion_sys::svn_utf_cstring_from_utf8(
            &mut dest_ptr,
            src_cstr.as_ptr(),
            pool.as_mut_ptr(),
        );
        Error::from_raw(err)?;

        if dest_ptr.is_null() {
            return Err(Error::from_message(
                "Native encoding conversion returned null",
            ));
        }

        let dest_cstr = std::ffi::CStr::from_ptr(dest_ptr);
        Ok(dest_cstr.to_str()?.to_string())
    })
}

/// Convert a UTF-8 string to specified target encoding
///
/// # Arguments
/// * `src` - Source string in UTF-8
/// * `topage` - Target encoding name (e.g., "ISO-8859-1")
///
/// # Returns
/// The string converted to target encoding, or an error if conversion fails.
pub fn cstring_from_utf8_ex(src: &str, topage: &str) -> Result<String, Error<'static>> {
    with_tmp_pool(|pool| unsafe {
        let src_cstr = std::ffi::CString::new(src)?;
        let topage_cstr = std::ffi::CString::new(topage)?;
        let mut dest_ptr: *const std::ffi::c_char = std::ptr::null();

        let err = subversion_sys::svn_utf_cstring_from_utf8_ex2(
            &mut dest_ptr,
            src_cstr.as_ptr(),
            topage_cstr.as_ptr(),
            pool.as_mut_ptr(),
        );
        Error::from_raw(err)?;

        if dest_ptr.is_null() {
            return Err(Error::from_message(
                "Target encoding conversion returned null",
            ));
        }

        let dest_cstr = std::ffi::CStr::from_ptr(dest_ptr);
        Ok(dest_cstr.to_str()?.to_string())
    })
}

/// Convert UTF-8 to native encoding with fuzzy conversion
///
/// This function converts a UTF-8 string to the native encoding, but unlike
/// `cstring_from_utf8`, it doesn't fail on unconvertible characters. Instead,
/// it replaces them with escape sequences of the form "?\\XXX".
///
/// # Arguments
/// * `src` - Source string in UTF-8
///
/// # Returns
/// The string converted to native encoding with escape sequences for unconvertible characters.
pub fn cstring_from_utf8_fuzzy(src: &str) -> Result<String, Error<'static>> {
    with_tmp_pool(|pool| unsafe {
        let src_cstr = std::ffi::CString::new(src)?;

        let dest_ptr =
            subversion_sys::svn_utf_cstring_from_utf8_fuzzy(src_cstr.as_ptr(), pool.as_mut_ptr());

        if dest_ptr.is_null() {
            return Err(Error::from_message("Fuzzy UTF-8 conversion returned null"));
        }

        let dest_cstr = std::ffi::CStr::from_ptr(dest_ptr);
        Ok(dest_cstr.to_str()?.to_string())
    })
}

/// Get the display width of a UTF-8 string
///
/// This function returns the number of display columns occupied by the UTF-8 string,
/// taking into account that some characters (like combining marks) have zero width
/// and some characters (like CJK) have double width.
///
/// # Arguments
/// * `cstr` - UTF-8 string to measure
///
/// # Returns
/// The display width in columns, or `None` if the string contains invalid UTF-8.
pub fn cstring_utf8_width(cstr: &str) -> Option<isize> {
    let cstr_c = std::ffi::CString::new(cstr).ok()?;

    unsafe {
        let width = subversion_sys::svn_utf_cstring_utf8_width(cstr_c.as_ptr());
        if width == -1 {
            None
        } else {
            Some(width as isize)
        }
    }
}

/// Trait for converting strings to UTF-8
pub trait ToUtf8 {
    /// Convert to UTF-8 from native encoding
    fn to_utf8(&self) -> Result<String, Error<'static>>;

    /// Convert to UTF-8 from specified encoding
    fn to_utf8_from(&self, frompage: &str) -> Result<String, Error<'static>>;
}

impl ToUtf8 for str {
    fn to_utf8(&self) -> Result<String, Error<'static>> {
        cstring_to_utf8(self)
    }

    fn to_utf8_from(&self, frompage: &str) -> Result<String, Error<'static>> {
        cstring_to_utf8_ex(self, frompage)
    }
}

impl ToUtf8 for String {
    fn to_utf8(&self) -> Result<String, Error<'static>> {
        cstring_to_utf8(self)
    }

    fn to_utf8_from(&self, frompage: &str) -> Result<String, Error<'static>> {
        cstring_to_utf8_ex(self, frompage)
    }
}

/// Trait for converting UTF-8 strings to native encoding
pub trait FromUtf8 {
    /// Convert from UTF-8 to native encoding
    fn from_utf8(&self) -> Result<String, Error<'static>>;

    /// Convert from UTF-8 to specified encoding
    fn from_utf8_to(&self, topage: &str) -> Result<String, Error<'static>>;

    /// Convert from UTF-8 to native encoding with fuzzy conversion
    fn from_utf8_fuzzy(&self) -> Result<String, Error<'static>>;
}

impl FromUtf8 for str {
    fn from_utf8(&self) -> Result<String, Error<'static>> {
        cstring_from_utf8(self)
    }

    fn from_utf8_to(&self, topage: &str) -> Result<String, Error<'static>> {
        cstring_from_utf8_ex(self, topage)
    }

    fn from_utf8_fuzzy(&self) -> Result<String, Error<'static>> {
        cstring_from_utf8_fuzzy(self)
    }
}

impl FromUtf8 for String {
    fn from_utf8(&self) -> Result<String, Error<'static>> {
        cstring_from_utf8(self)
    }

    fn from_utf8_to(&self, topage: &str) -> Result<String, Error<'static>> {
        cstring_from_utf8_ex(self, topage)
    }

    fn from_utf8_fuzzy(&self) -> Result<String, Error<'static>> {
        cstring_from_utf8_fuzzy(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialize() {
        // Test initialization with UTF-8 assumption
        initialize(true);

        // Test initialization without UTF-8 assumption
        initialize(false);
    }

    #[test]
    fn test_utf8_width() {
        // Basic ASCII
        assert_eq!(cstring_utf8_width("hello"), Some(5));

        // Empty string
        assert_eq!(cstring_utf8_width(""), Some(0));

        // Single character
        assert_eq!(cstring_utf8_width("a"), Some(1));

        // Multi-byte UTF-8 characters (may vary by system)
        let width = cstring_utf8_width("café");
        assert!(width.is_some());
        assert!(width.unwrap() >= 4); // At least 4 columns
    }

    #[test]
    fn test_utf8_round_trip() {
        // Initialize UTF system first
        initialize(true);

        let original = "Hello, UTF-8 world! 🌍";

        // Since we're likely on a UTF-8 system, this should be a no-op
        let utf8_string = cstring_to_utf8(original).unwrap();
        assert_eq!(utf8_string, original);

        let final_string = cstring_from_utf8(&utf8_string).unwrap();
        assert_eq!(final_string, original);
    }

    #[test]
    fn test_fuzzy_conversion() {
        // Initialize UTF system first
        initialize(true);

        let utf8_string = "Hello, world!";

        // Fuzzy conversion should always succeed
        let converted = cstring_from_utf8_fuzzy(utf8_string).unwrap();
        assert_eq!(converted, "Hello, world!");
    }

    #[test]
    fn test_traits() {
        let test_str = "Hello, world!";

        // Test ToUtf8 trait on &str
        assert_eq!(test_str.to_utf8().unwrap(), "Hello, world!");

        // Test ToUtf8 trait on String
        let test_string = test_str.to_string();
        assert_eq!(test_string.to_utf8().unwrap(), "Hello, world!");

        // Test FromUtf8 trait
        assert_eq!(test_str.from_utf8().unwrap(), "Hello, world!");
        assert_eq!(test_str.from_utf8_fuzzy().unwrap(), "Hello, world!");
    }

    #[test]
    fn test_explicit_encoding() {
        let test_str = "Hello";

        // Converting UTF-8 to/from UTF-8 should return the same string
        assert_eq!(test_str.to_utf8_from("UTF-8").unwrap(), "Hello");
        assert_eq!(test_str.from_utf8_to("UTF-8").unwrap(), "Hello");
    }

    #[test]
    fn test_empty_strings() {
        let empty = "";

        // All operations on empty strings should return empty strings
        assert_eq!(empty.to_utf8().unwrap(), "");
        assert_eq!(empty.from_utf8().unwrap(), "");
        assert_eq!(empty.from_utf8_fuzzy().unwrap(), "");
        assert_eq!(cstring_utf8_width(empty), Some(0));
    }

    #[test]
    fn test_null_handling() {
        // Test that we handle null bytes correctly
        let result = cstring_to_utf8("test\0ing");
        // Should fail because we can't create a CString with null bytes
        assert!(result.is_err());
    }
}
