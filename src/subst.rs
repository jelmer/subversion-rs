//! Keyword substitution and EOL translation
//!
//! This module provides safe Rust wrappers around Subversion's keyword and
//! end-of-line substitution functionality.

use crate::{with_tmp_pool, Error};
use std::collections::HashMap;

/// EOL style for text files
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EolStyle {
    /// Unrecognized style
    Unknown,
    /// No EOL translation
    None,
    /// Use the native EOL style for the platform
    Native,
    /// Use a fixed EOL style (LF, CR, or CRLF)
    Fixed,
}

impl From<subversion_sys::svn_subst_eol_style_t> for EolStyle {
    fn from(style: subversion_sys::svn_subst_eol_style_t) -> Self {
        match style {
            subversion_sys::svn_subst_eol_style_svn_subst_eol_style_unknown => EolStyle::Unknown,
            subversion_sys::svn_subst_eol_style_svn_subst_eol_style_none => EolStyle::None,
            subversion_sys::svn_subst_eol_style_svn_subst_eol_style_native => EolStyle::Native,
            subversion_sys::svn_subst_eol_style_svn_subst_eol_style_fixed => EolStyle::Fixed,
            _ => EolStyle::Unknown,
        }
    }
}

impl From<EolStyle> for subversion_sys::svn_subst_eol_style_t {
    fn from(style: EolStyle) -> Self {
        match style {
            EolStyle::Unknown => subversion_sys::svn_subst_eol_style_svn_subst_eol_style_unknown,
            EolStyle::None => subversion_sys::svn_subst_eol_style_svn_subst_eol_style_none,
            EolStyle::Native => subversion_sys::svn_subst_eol_style_svn_subst_eol_style_native,
            EolStyle::Fixed => subversion_sys::svn_subst_eol_style_svn_subst_eol_style_fixed,
        }
    }
}

/// Parse an EOL style value from a property string
///
/// Returns the EOL style and the corresponding EOL string.
/// The EOL string is None for EolStyle::None, the platform native
/// EOL for EolStyle::Native, or the specific EOL for EolStyle::Fixed.
pub fn eol_style_from_value(value: &str) -> (EolStyle, Option<String>) {
    let value_cstr = std::ffi::CString::new(value).unwrap();

    unsafe {
        let mut style: subversion_sys::svn_subst_eol_style_t =
            subversion_sys::svn_subst_eol_style_svn_subst_eol_style_unknown;
        let mut eol_ptr: *const std::ffi::c_char = std::ptr::null();

        subversion_sys::svn_subst_eol_style_from_value(
            &mut style,
            &mut eol_ptr,
            value_cstr.as_ptr(),
        );

        let eol_str = if eol_ptr.is_null() {
            None
        } else {
            Some(
                std::ffi::CStr::from_ptr(eol_ptr)
                    .to_str()
                    .unwrap()
                    .to_string(),
            )
        };

        (EolStyle::from(style), eol_str)
    }
}

/// Check if translation is required for the given parameters
///
/// Returns true if the working copy and normalized versions of a file
/// with the given parameters differ.
pub fn translation_required(
    style: EolStyle,
    eol: Option<&str>,
    keywords: Option<&HashMap<String, String>>,
    special: bool,
    force_eol_check: bool,
) -> bool {
    with_tmp_pool(|pool| {
        let eol_ptr = match eol {
            Some(s) => {
                let cstr = std::ffi::CString::new(s).unwrap();
                cstr.as_ptr()
            }
            None => std::ptr::null(),
        };

        let keywords_hash = match keywords {
            Some(kw) => {
                // Create C strings that will live for the duration of the call
                let c_strings: Vec<_> = kw
                    .iter()
                    .map(|(k, v)| {
                        (
                            std::ffi::CString::new(k.as_str()).unwrap(),
                            std::ffi::CString::new(v.as_str()).unwrap(),
                        )
                    })
                    .collect();
                let mut hash = apr::hash::Hash::new(pool);
                for ((k, _), (_, v_cstr)) in kw.iter().zip(c_strings.iter()) {
                    unsafe {
                        hash.insert(k.as_bytes(), v_cstr.as_ptr() as *mut std::ffi::c_void);
                    }
                }
                unsafe { hash.as_mut_ptr() }
            }
            None => std::ptr::null_mut(),
        };

        unsafe {
            subversion_sys::svn_subst_translation_required(
                style.into(),
                eol_ptr,
                keywords_hash,
                if special { 1 } else { 0 },
                if force_eol_check { 1 } else { 0 },
            ) != 0
        }
    })
}

/// Build keyword hash from keyword string and file information
///
/// Creates a keyword hash suitable for use with translation functions.
/// The keywords_string should be the value of the svn:keywords property.
pub fn build_keywords(
    keywords_string: &str,
    rev: Option<&str>,
    url: Option<&str>,
    date: Option<apr::time::Time>,
    author: Option<&str>,
    repos_root_url: Option<&str>,
) -> Result<HashMap<String, String>, Error> {
    with_tmp_pool(|pool| {
        let keywords_cstr = std::ffi::CString::new(keywords_string).unwrap();
        let rev_cstr = rev.map(|s| std::ffi::CString::new(s).unwrap());
        let url_cstr = url.map(|s| std::ffi::CString::new(s).unwrap());
        let author_cstr = author.map(|s| std::ffi::CString::new(s).unwrap());
        let repos_root_cstr = repos_root_url.map(|s| std::ffi::CString::new(s).unwrap());

        let mut keywords_hash: *mut subversion_sys::apr_hash_t = std::ptr::null_mut();

        unsafe {
            let err = subversion_sys::svn_subst_build_keywords3(
                &mut keywords_hash,
                keywords_cstr.as_ptr(),
                rev_cstr
                    .as_ref()
                    .map(|c| c.as_ptr())
                    .unwrap_or(std::ptr::null()),
                url_cstr
                    .as_ref()
                    .map(|c| c.as_ptr())
                    .unwrap_or(std::ptr::null()),
                repos_root_cstr
                    .as_ref()
                    .map(|c| c.as_ptr())
                    .unwrap_or(std::ptr::null()),
                date.map(|d| d.into()).unwrap_or(0),
                author_cstr
                    .as_ref()
                    .map(|c| c.as_ptr())
                    .unwrap_or(std::ptr::null()),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
        }

        if keywords_hash.is_null() {
            return Ok(HashMap::new());
        }

        let prop_hash = unsafe { crate::props::PropHash::from_ptr(keywords_hash) };
        Ok(prop_hash.to_string_hashmap())
    })
}

/// Check if two keyword hashes differ
///
/// If compare_values is true, both the keys and values must match.
/// If false, only the keys need to match (values can differ).
pub fn keywords_differ(
    a: Option<&HashMap<String, String>>,
    b: Option<&HashMap<String, String>>,
    compare_values: bool,
) -> Result<bool, Error> {
    with_tmp_pool(|pool| {
        // Keep c_strings alive until after the FFI call
        let c_strings_a: Vec<_> = match a {
            Some(kw) => kw
                .iter()
                .map(|(k, v)| (k.as_str(), std::ffi::CString::new(v.as_str()).unwrap()))
                .collect(),
            None => Vec::new(),
        };

        let hash_a = match a {
            Some(kw) => {
                let mut hash = apr::hash::Hash::new(pool);
                for ((k, _), (_, v_cstr)) in kw.iter().zip(c_strings_a.iter()) {
                    unsafe {
                        hash.insert(k.as_bytes(), v_cstr.as_ptr() as *mut std::ffi::c_void);
                    }
                }
                unsafe { hash.as_mut_ptr() }
            }
            None => std::ptr::null_mut(),
        };

        // Keep c_strings alive until after the FFI call
        let c_strings_b: Vec<_> = match b {
            Some(kw) => kw
                .iter()
                .map(|(k, v)| (k.as_str(), std::ffi::CString::new(v.as_str()).unwrap()))
                .collect(),
            None => Vec::new(),
        };

        let hash_b = match b {
            Some(kw) => {
                let mut hash = apr::hash::Hash::new(pool);
                for ((k, _), (_, v_cstr)) in kw.iter().zip(c_strings_b.iter()) {
                    unsafe {
                        hash.insert(k.as_bytes(), v_cstr.as_ptr() as *mut std::ffi::c_void);
                    }
                }
                unsafe { hash.as_mut_ptr() }
            }
            None => std::ptr::null_mut(),
        };

        unsafe {
            let result = subversion_sys::svn_subst_keywords_differ2(
                hash_a,
                hash_b,
                if compare_values { 1 } else { 0 },
                pool.as_mut_ptr(),
            );
            Ok(result != 0)
        }
    })
}

/// Create a translated stream for keyword and EOL substitution
///
/// Returns a stream that performs EOL translation and keyword expansion/contraction
/// when data is read from or written to it.
pub fn stream_translated(
    mut source_stream: crate::io::Stream,
    eol_str: Option<&str>,
    repair: bool,
    keywords: Option<&HashMap<String, String>>,
    expand: bool,
) -> Result<crate::io::Stream, Error> {
    with_tmp_pool(|pool| {
        let eol_cstr = eol_str.map(|s| std::ffi::CString::new(s).unwrap());
        let eol_ptr = eol_cstr
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(std::ptr::null());

        let keywords_hash = match keywords {
            Some(kw) => {
                // Create C strings that will live for the duration of the call
                let c_strings: Vec<_> = kw
                    .iter()
                    .map(|(k, v)| {
                        (
                            std::ffi::CString::new(k.as_str()).unwrap(),
                            std::ffi::CString::new(v.as_str()).unwrap(),
                        )
                    })
                    .collect();
                let mut hash = apr::hash::Hash::new(pool);
                for ((k, _), (_, v_cstr)) in kw.iter().zip(c_strings.iter()) {
                    unsafe {
                        hash.insert(k.as_bytes(), v_cstr.as_ptr() as *mut std::ffi::c_void);
                    }
                }
                unsafe { hash.as_mut_ptr() }
            }
            None => std::ptr::null_mut(),
        };

        let result_pool = apr::Pool::new();

        unsafe {
            let translated_stream = subversion_sys::svn_subst_stream_translated(
                source_stream.as_mut_ptr(),
                eol_ptr,
                if repair { 1 } else { 0 },
                keywords_hash,
                if expand { 1 } else { 0 },
                result_pool.as_mut_ptr(),
            );

            Ok(crate::io::Stream::from_ptr(translated_stream, result_pool))
        }
    })
}

/// Create a translated stream for converting to normal form
///
/// This creates a stream that translates content to the "normal form"
/// used internally by Subversion (LF line endings, expanded keywords).
pub fn stream_translated_to_normal_form(
    mut source_stream: crate::io::Stream,
    eol_style: EolStyle,
    eol_str: Option<&str>,
    always_repair_eols: bool,
    keywords: Option<&HashMap<String, String>>,
) -> Result<crate::io::Stream, Error> {
    with_tmp_pool(|pool| {
        let eol_cstr = eol_str.map(|s| std::ffi::CString::new(s).unwrap());
        let eol_ptr = eol_cstr
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(std::ptr::null());

        let keywords_hash = match keywords {
            Some(kw) => {
                // Create C strings that will live for the duration of the call
                let c_strings: Vec<_> = kw
                    .iter()
                    .map(|(k, v)| {
                        (
                            std::ffi::CString::new(k.as_str()).unwrap(),
                            std::ffi::CString::new(v.as_str()).unwrap(),
                        )
                    })
                    .collect();
                let mut hash = apr::hash::Hash::new(pool);
                for ((k, _), (_, v_cstr)) in kw.iter().zip(c_strings.iter()) {
                    unsafe {
                        hash.insert(k.as_bytes(), v_cstr.as_ptr() as *mut std::ffi::c_void);
                    }
                }
                unsafe { hash.as_mut_ptr() }
            }
            None => std::ptr::null_mut(),
        };

        let result_pool = apr::Pool::new();

        unsafe {
            let mut translated_stream: *mut subversion_sys::svn_stream_t = std::ptr::null_mut();
            let err = subversion_sys::svn_subst_stream_translated_to_normal_form(
                &mut translated_stream,
                source_stream.as_mut_ptr(),
                eol_style.into(),
                eol_ptr,
                if always_repair_eols { 1 } else { 0 },
                keywords_hash,
                result_pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;

            Ok(crate::io::Stream::from_ptr(translated_stream, result_pool))
        }
    })
}

/// Copy and translate a file with keyword and EOL substitution
///
/// This is a convenience function that copies from source to destination
/// while performing translation. Either eol_str or keywords (or both) must
/// be provided.
pub fn translate_stream(
    src_stream: &mut crate::io::Stream,
    dst_stream: &mut crate::io::Stream,
    eol_str: Option<&str>,
    repair: bool,
    keywords: Option<&HashMap<String, String>>,
    expand: bool,
) -> Result<(), Error> {
    with_tmp_pool(|pool| {
        let eol_cstr = eol_str.map(|s| std::ffi::CString::new(s).unwrap());
        let eol_ptr = eol_cstr
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(std::ptr::null());

        let keywords_hash = match keywords {
            Some(kw) => {
                // Create C strings that will live for the duration of the call
                let c_strings: Vec<_> = kw
                    .iter()
                    .map(|(k, v)| {
                        (
                            std::ffi::CString::new(k.as_str()).unwrap(),
                            std::ffi::CString::new(v.as_str()).unwrap(),
                        )
                    })
                    .collect();
                let mut hash = apr::hash::Hash::new(pool);
                for ((k, _), (_, v_cstr)) in kw.iter().zip(c_strings.iter()) {
                    unsafe {
                        hash.insert(k.as_bytes(), v_cstr.as_ptr() as *mut std::ffi::c_void);
                    }
                }
                unsafe { hash.as_mut_ptr() }
            }
            None => std::ptr::null_mut(),
        };

        unsafe {
            let err = subversion_sys::svn_subst_translate_stream3(
                src_stream.as_mut_ptr(),
                dst_stream.as_mut_ptr(),
                eol_ptr,
                if repair { 1 } else { 0 },
                keywords_hash,
                if expand { 1 } else { 0 },
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eol_style_from_value() {
        let (style, eol) = eol_style_from_value("native");
        assert_eq!(style, EolStyle::Native);
        assert!(eol.is_some());

        let (style, eol) = eol_style_from_value("LF");
        assert_eq!(style, EolStyle::Fixed);
        assert_eq!(eol.as_deref(), Some("\n"));

        let (style, eol) = eol_style_from_value("CRLF");
        assert_eq!(style, EolStyle::Fixed);
        assert_eq!(eol.as_deref(), Some("\r\n"));

        let (style, eol) = eol_style_from_value("CR");
        assert_eq!(style, EolStyle::Fixed);
        assert_eq!(eol.as_deref(), Some("\r"));

        // Test invalid value returns Unknown
        let (style, eol) = eol_style_from_value("invalid");
        assert_eq!(style, EolStyle::Unknown);
        assert!(eol.is_none());
    }

    #[test]
    fn test_translation_required() {
        // No translation needed
        assert!(!translation_required(
            EolStyle::None,
            None,
            None,
            false,
            false
        ));

        // EOL translation may or may not be needed depending on platform
        // On Unix systems, native is \n, so \n -> native requires no translation
        let _result = translation_required(EolStyle::Native, Some("\n"), None, false, false);
        // Don't assert since this is platform-dependent

        // Keyword translation needed
        let mut keywords = HashMap::new();
        keywords.insert("Id".to_string(), "$Id$".to_string());
        assert!(translation_required(
            EolStyle::None,
            None,
            Some(&keywords),
            false,
            false
        ));
    }

    #[test]
    fn test_build_keywords() {
        let keywords = build_keywords(
            "Id Rev Author Date",
            Some("123"),
            Some("http://example.com/repo/file.txt"),
            Some(apr::time::Time::now()),
            Some("testuser"),
            Some("http://example.com/repo"),
        )
        .unwrap();

        assert!(keywords.contains_key("Id"));
        assert!(keywords.contains_key("Rev"));
        assert!(keywords.contains_key("Author"));
        assert!(keywords.contains_key("Date"));

        // Test with empty keyword string
        let empty_keywords = build_keywords("", None, None, None, None, None).unwrap();
        assert!(empty_keywords.is_empty());
    }

    #[test]
    fn test_keywords_differ() {
        let mut kw1 = HashMap::new();
        kw1.insert("Id".to_string(), "$Id$".to_string());
        kw1.insert("Rev".to_string(), "$Rev$".to_string());

        let mut kw2 = HashMap::new();
        kw2.insert("Id".to_string(), "$Id$".to_string());
        kw2.insert("Rev".to_string(), "$Rev$".to_string());

        // Same keywords
        assert!(!keywords_differ(Some(&kw1), Some(&kw2), true).unwrap());
        assert!(!keywords_differ(Some(&kw1), Some(&kw2), false).unwrap());

        // Different values
        kw2.insert("Rev".to_string(), "$Rev: 456$".to_string());
        assert!(keywords_differ(Some(&kw1), Some(&kw2), true).unwrap());
        assert!(!keywords_differ(Some(&kw1), Some(&kw2), false).unwrap());

        // Different keys
        kw2.insert("Date".to_string(), "$Date$".to_string());
        assert!(keywords_differ(Some(&kw1), Some(&kw2), true).unwrap());
        assert!(keywords_differ(Some(&kw1), Some(&kw2), false).unwrap());

        // One empty, one not
        assert!(keywords_differ(Some(&kw1), None, false).unwrap());
        assert!(keywords_differ(None, Some(&kw2), false).unwrap());

        // Both empty
        assert!(!keywords_differ(None, None, false).unwrap());
    }
}
