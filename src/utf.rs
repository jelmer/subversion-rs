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
pub fn cstring_to_utf8(src: &str) -> Result<String, Error> {
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
            return Err(Error::from_str("UTF-8 conversion returned null"));
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
pub fn cstring_to_utf8_ex(src: &str, frompage: &str) -> Result<String, Error> {
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
            return Err(Error::from_str("UTF-8 conversion returned null"));
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
pub fn cstring_from_utf8(src: &str) -> Result<String, Error> {
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
            return Err(Error::from_str("Native encoding conversion returned null"));
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
pub fn cstring_from_utf8_ex(src: &str, topage: &str) -> Result<String, Error> {
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
            return Err(Error::from_str("Target encoding conversion returned null"));
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
pub fn cstring_from_utf8_fuzzy(src: &str) -> Result<String, Error> {
    with_tmp_pool(|pool| unsafe {
        let src_cstr = std::ffi::CString::new(src)?;

        let dest_ptr =
            subversion_sys::svn_utf_cstring_from_utf8_fuzzy(src_cstr.as_ptr(), pool.as_mut_ptr());

        if dest_ptr.is_null() {
            return Err(Error::from_str("Fuzzy UTF-8 conversion returned null"));
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
    fn to_utf8(&self) -> Result<String, Error>;

    /// Convert to UTF-8 from specified encoding
    fn to_utf8_from(&self, frompage: &str) -> Result<String, Error>;
}

impl ToUtf8 for str {
    fn to_utf8(&self) -> Result<String, Error> {
        cstring_to_utf8(self)
    }

    fn to_utf8_from(&self, frompage: &str) -> Result<String, Error> {
        cstring_to_utf8_ex(self, frompage)
    }
}

impl ToUtf8 for String {
    fn to_utf8(&self) -> Result<String, Error> {
        cstring_to_utf8(self)
    }

    fn to_utf8_from(&self, frompage: &str) -> Result<String, Error> {
        cstring_to_utf8_ex(self, frompage)
    }
}

/// Trait for converting UTF-8 strings to native encoding
pub trait FromUtf8 {
    /// Convert from UTF-8 to native encoding
    fn from_utf8(&self) -> Result<String, Error>;

    /// Convert from UTF-8 to specified encoding
    fn from_utf8_to(&self, topage: &str) -> Result<String, Error>;

    /// Convert from UTF-8 to native encoding with fuzzy conversion
    fn from_utf8_fuzzy(&self) -> Result<String, Error>;
}

impl FromUtf8 for str {
    fn from_utf8(&self) -> Result<String, Error> {
        cstring_from_utf8(self)
    }

    fn from_utf8_to(&self, topage: &str) -> Result<String, Error> {
        cstring_from_utf8_ex(self, topage)
    }

    fn from_utf8_fuzzy(&self) -> Result<String, Error> {
        cstring_from_utf8_fuzzy(self)
    }
}

impl FromUtf8 for String {
    fn from_utf8(&self) -> Result<String, Error> {
        cstring_from_utf8(self)
    }

    fn from_utf8_to(&self, topage: &str) -> Result<String, Error> {
        cstring_from_utf8_ex(self, topage)
    }

    fn from_utf8_fuzzy(&self) -> Result<String, Error> {
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
        let to_utf8_result = cstring_to_utf8(original);
        assert!(to_utf8_result.is_ok());

        let utf8_string = to_utf8_result.unwrap();
        let from_utf8_result = cstring_from_utf8(&utf8_string);
        assert!(from_utf8_result.is_ok());

        // On UTF-8 systems, this should be identical
        let final_string = from_utf8_result.unwrap();
        assert_eq!(original, final_string);
    }

    #[test]
    fn test_fuzzy_conversion() {
        // Initialize UTF system first
        initialize(true);

        let utf8_string = "Hello, world! 🌍";

        // Fuzzy conversion should always succeed
        let result = cstring_from_utf8_fuzzy(utf8_string);
        assert!(result.is_ok());

        let converted = result.unwrap();
        // Should at least contain the ASCII parts
        assert!(converted.contains("Hello, world!"));
    }

    #[test]
    fn test_traits() {
        let test_str = "Hello, world!";

        // Test ToUtf8 trait on &str
        let utf8_result = test_str.to_utf8();
        assert!(utf8_result.is_ok());

        // Test ToUtf8 trait on String
        let test_string = test_str.to_string();
        let utf8_result2 = test_string.to_utf8();
        assert!(utf8_result2.is_ok());

        // Test FromUtf8 trait
        let utf8_str = utf8_result.unwrap();
        let native_result = utf8_str.from_utf8();
        assert!(native_result.is_ok());

        let fuzzy_result = utf8_str.from_utf8_fuzzy();
        assert!(fuzzy_result.is_ok());
    }

    #[test]
    fn test_explicit_encoding() {
        let test_str = "Hello";

        // Test with explicit encodings (these may or may not work depending on system)
        // We mainly test that the functions don't crash
        let _ = test_str.to_utf8_from("UTF-8");
        let _ = test_str.from_utf8_to("UTF-8");
    }

    #[test]
    fn test_empty_strings() {
        let empty = "";

        // All operations should work with empty strings
        assert!(empty.to_utf8().is_ok());
        assert!(empty.from_utf8().is_ok());
        assert!(empty.from_utf8_fuzzy().is_ok());
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
