//! Property handling utilities for Subversion.
//!
//! This module provides functions for working with Subversion properties,
//! including validation, classification, and type-safe wrappers for property hashes.
//!
//! # Property Types
//!
//! Subversion has three main types of properties:
//! - **Regular properties**: Normal versioned properties on files and directories
//! - **Revision properties**: Properties attached to specific revisions (like svn:log)
//! - **Working copy properties**: Internal properties used by the working copy
//!
//! # Common Properties
//!
//! Some commonly used SVN properties include:
//! - `svn:mime-type` - MIME type of a file
//! - `svn:eol-style` - End-of-line style (native, LF, CRLF, CR)
//! - `svn:executable` - Mark file as executable
//! - `svn:ignore` - Patterns for ignoring files
//! - `svn:externals` - External repository references
//! - `svn:log` - Commit log message (revision property)
//! - `svn:author` - Author of a revision (revision property)
//! - `svn:date` - Date of a revision (revision property)

/// Kinds of Subversion properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Entry property.
    Entry,
    /// Working copy property.
    Wc,
    /// Regular property.
    Regular,
}

impl From<subversion_sys::svn_prop_kind_t> for Kind {
    fn from(kind: subversion_sys::svn_prop_kind_t) -> Self {
        match kind {
            subversion_sys::svn_prop_kind_svn_prop_entry_kind => Kind::Entry,
            subversion_sys::svn_prop_kind_svn_prop_wc_kind => Kind::Wc,
            subversion_sys::svn_prop_kind_svn_prop_regular_kind => Kind::Regular,
            _ => panic!("Unknown property kind"),
        }
    }
}

/// Gets the kind of a property by its name.
pub fn kind(name: &str) -> Result<Kind, crate::Error<'_>> {
    use crate::ToCString;
    let name = name.to_cstring()?;
    Ok(unsafe { subversion_sys::svn_property_kind2(name.as_ptr()) }.into())
}

/// Checks if a property name is a Subversion property.
pub fn is_svn_prop(name: &str) -> Result<bool, crate::Error<'_>> {
    use crate::ToCString;
    let name = name.to_cstring()?;
    Ok(unsafe { subversion_sys::svn_prop_is_svn_prop(name.as_ptr()) != 0 })
}

/// Checks if a property is a boolean property.
pub fn is_boolean(name: &str) -> Result<bool, crate::Error<'_>> {
    use crate::ToCString;
    let name = name.to_cstring()?;
    Ok(unsafe { subversion_sys::svn_prop_is_boolean(name.as_ptr()) != 0 })
}

/// Checks if a property is a known Subversion revision property.
pub fn is_known_svn_rev_prop(name: &str) -> Result<bool, crate::Error<'_>> {
    use crate::ToCString;
    let name = name.to_cstring()?;
    Ok(unsafe { subversion_sys::svn_prop_is_known_svn_rev_prop(name.as_ptr()) != 0 })
}

/// Checks if a property is a known Subversion node property.
pub fn is_known_svn_node_prop(name: &str) -> Result<bool, crate::Error<'_>> {
    use crate::ToCString;
    let name = name.to_cstring()?;
    Ok(unsafe { subversion_sys::svn_prop_is_known_svn_node_prop(name.as_ptr()) != 0 })
}

/// Checks if a property is a known Subversion file property.
pub fn is_known_svn_file_prop(name: &str) -> Result<bool, crate::Error<'_>> {
    use crate::ToCString;
    let name = name.to_cstring()?;
    Ok(unsafe { subversion_sys::svn_prop_is_known_svn_file_prop(name.as_ptr()) != 0 })
}

/// Checks if a property is a known Subversion directory property.
pub fn is_known_svn_dir_prop(name: &str) -> Result<bool, crate::Error<'_>> {
    use crate::ToCString;
    let name = name.to_cstring()?;
    Ok(unsafe { subversion_sys::svn_prop_is_known_svn_dir_prop(name.as_ptr()) != 0 })
}

/// Checks if a property needs translation.
pub fn needs_translation(name: &str) -> Result<bool, crate::Error<'_>> {
    use crate::ToCString;
    let name = name.to_cstring()?;
    Ok(unsafe { subversion_sys::svn_prop_needs_translation(name.as_ptr()) != 0 })
}

/// Checks if a property name is valid.
pub fn name_is_valid(name: &str) -> bool {
    let name = std::ffi::CString::new(name).unwrap();
    unsafe { subversion_sys::svn_prop_name_is_valid(name.as_ptr()) != 0 }
}

/// A safe wrapper for APR hashes containing property name -> svn_string_t mappings
///
/// This wrapper encapsulates the common pattern of working with property hashes
/// from Subversion's C API, reducing unsafe code and providing convenient
/// conversion methods.
pub struct PropHash<'a> {
    inner: apr::hash::TypedHash<'a, subversion_sys::svn_string_t>,
}

impl<'a> PropHash<'a> {
    /// Create a PropHash from a raw APR hash pointer
    ///
    /// # Safety
    /// The caller must ensure that:
    /// - `ptr` is a valid APR hash containing svn_string_t values
    /// - The hash and its contents remain valid for the lifetime of this wrapper
    pub unsafe fn from_ptr(ptr: *mut apr_sys::apr_hash_t) -> Self {
        Self {
            inner: apr::hash::TypedHash::<subversion_sys::svn_string_t>::from_ptr(ptr),
        }
    }

    /// Create a PropHash from a HashMap of properties.
    ///
    /// Creates a new APR hash in the provided pool and populates it
    /// with the key-value pairs from the HashMap.
    pub fn from_hashmap(
        props: &std::collections::HashMap<String, Vec<u8>>,
        pool: &apr::Pool,
    ) -> Self {
        unsafe {
            let hash = apr_sys::apr_hash_make(pool.as_mut_ptr());

            for (key, value) in props.iter() {
                let key_cstr = std::ffi::CString::new(key.as_str()).unwrap();
                let key_ptr = apr_sys::apr_pstrdup(pool.as_mut_ptr(), key_cstr.as_ptr());

                let svn_string = subversion_sys::svn_string_ncreate(
                    value.as_ptr() as *const i8,
                    value.len(),
                    pool.as_mut_ptr(),
                );

                apr_sys::apr_hash_set(
                    hash,
                    key_ptr as *const std::ffi::c_void,
                    apr_sys::APR_HASH_KEY_STRING as isize,
                    svn_string as *const std::ffi::c_void,
                );
            }

            Self::from_ptr(hash)
        }
    }

    /// Create a deep copy of this property hash in a new pool.
    ///
    /// All keys and values are duplicated in the provided pool.
    ///
    /// Wraps `svn_prop_hash_dup`.
    pub fn duplicate(&self, pool: &apr::Pool) -> Self {
        unsafe {
            let new_hash =
                subversion_sys::svn_prop_hash_dup(self.inner.as_ptr(), pool.as_mut_ptr());
            Self::from_ptr(new_hash)
        }
    }

    /// Convert the properties to a `HashMap<String, Vec<u8>>`
    ///
    /// This is the most common conversion pattern in the codebase.
    pub fn to_hashmap(&self) -> std::collections::HashMap<String, Vec<u8>> {
        self.inner
            .iter()
            .map(|(k, v)| {
                let key = String::from_utf8_lossy(k).into_owned();
                let value = crate::svn_string_helpers::to_vec(v);
                (key, value)
            })
            .collect()
    }

    /// Convert the properties to a HashMap<String, String>
    ///
    /// This is useful when you know the properties contain valid UTF-8 text.
    /// Non-UTF-8 bytes will be replaced with the UTF-8 replacement character.
    pub fn to_string_hashmap(&self) -> std::collections::HashMap<String, String> {
        self.inner
            .iter()
            .map(|(k, v)| {
                let key = String::from_utf8_lossy(k).into_owned();
                let value =
                    String::from_utf8_lossy(crate::svn_string_helpers::as_bytes(v)).into_owned();
                (key, value)
            })
            .collect()
    }

    /// Iterate over the properties as (key: &str, value: &[u8]) pairs
    pub fn iter_bytes(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.inner.iter().map(|(k, v)| {
            let key = std::str::from_utf8(k).unwrap_or("");
            let value = crate::svn_string_helpers::as_bytes(v);
            (key, value)
        })
    }

    /// Iterate over the properties as (key: &str, value: &str) pairs
    ///
    /// Non-UTF-8 bytes in values will be replaced with the UTF-8 replacement character.
    pub fn iter_strings(&self) -> impl Iterator<Item = (&str, std::borrow::Cow<'_, str>)> {
        self.inner.iter().map(|(k, v)| {
            let key = std::str::from_utf8(k).unwrap_or("");
            let value = String::from_utf8_lossy(crate::svn_string_helpers::as_bytes(v));
            (key, value)
        })
    }

    /// Get a property value by name
    pub fn get(&self, name: &str) -> Option<Vec<u8>> {
        // Try to find the key in the hash
        for (k, v) in self.inner.iter() {
            if k == name.as_bytes() {
                return Some(crate::svn_string_helpers::to_vec(v));
            }
        }
        None
    }

    /// Get a property value by name as a string
    pub fn get_string(&self, name: &str) -> Option<String> {
        // Try to find the key in the hash
        for (k, v) in self.inner.iter() {
            if k == name.as_bytes() {
                return Some(
                    String::from_utf8_lossy(crate::svn_string_helpers::as_bytes(v)).into_owned(),
                );
            }
        }
        None
    }

    /// Check if the hash is empty
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Get the number of properties
    pub fn len(&self) -> usize {
        self.inner.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kind_svn_mime_type() {
        let k = kind("svn:mime-type").unwrap();
        assert_eq!(k, Kind::Regular);
    }

    #[test]
    fn test_kind_svn_eol_style() {
        let k = kind("svn:eol-style").unwrap();
        assert_eq!(k, Kind::Regular);
    }

    #[test]
    fn test_kind_custom_property() {
        let k = kind("custom:property").unwrap();
        assert_eq!(k, Kind::Regular);
    }

    #[test]
    fn test_is_svn_prop_true() {
        assert_eq!(is_svn_prop("svn:mime-type").unwrap(), true);
        assert_eq!(is_svn_prop("svn:eol-style").unwrap(), true);
    }

    #[test]
    fn test_is_svn_prop_false() {
        assert_eq!(is_svn_prop("custom:property").unwrap(), false);
        assert_eq!(is_svn_prop("user-property").unwrap(), false);
    }

    #[test]
    fn test_is_boolean_true() {
        assert_eq!(is_boolean("svn:executable").unwrap(), true);
        assert_eq!(is_boolean("svn:special").unwrap(), true);
    }

    #[test]
    fn test_is_boolean_false() {
        assert_eq!(is_boolean("svn:mime-type").unwrap(), false);
        assert_eq!(is_boolean("custom:property").unwrap(), false);
    }

    #[test]
    fn test_is_known_svn_rev_prop_true() {
        assert_eq!(is_known_svn_rev_prop("svn:log").unwrap(), true);
        assert_eq!(is_known_svn_rev_prop("svn:author").unwrap(), true);
        assert_eq!(is_known_svn_rev_prop("svn:date").unwrap(), true);
    }

    #[test]
    fn test_is_known_svn_rev_prop_false() {
        assert_eq!(is_known_svn_rev_prop("svn:mime-type").unwrap(), false);
        assert_eq!(is_known_svn_rev_prop("custom:property").unwrap(), false);
    }

    #[test]
    fn test_is_known_svn_node_prop_true() {
        assert_eq!(is_known_svn_node_prop("svn:mime-type").unwrap(), true);
        assert_eq!(is_known_svn_node_prop("svn:eol-style").unwrap(), true);
    }

    #[test]
    fn test_is_known_svn_node_prop_false() {
        assert_eq!(is_known_svn_node_prop("svn:log").unwrap(), false);
        assert_eq!(is_known_svn_node_prop("custom:property").unwrap(), false);
    }

    #[test]
    fn test_is_known_svn_file_prop_true() {
        assert_eq!(is_known_svn_file_prop("svn:mime-type").unwrap(), true);
        assert_eq!(is_known_svn_file_prop("svn:eol-style").unwrap(), true);
        assert_eq!(is_known_svn_file_prop("svn:executable").unwrap(), true);
    }

    #[test]
    fn test_is_known_svn_file_prop_false() {
        assert_eq!(is_known_svn_file_prop("svn:ignore").unwrap(), false);
        assert_eq!(is_known_svn_file_prop("custom:property").unwrap(), false);
    }

    #[test]
    fn test_is_known_svn_dir_prop_true() {
        assert_eq!(is_known_svn_dir_prop("svn:ignore").unwrap(), true);
        assert_eq!(is_known_svn_dir_prop("svn:externals").unwrap(), true);
    }

    #[test]
    fn test_is_known_svn_dir_prop_false() {
        assert_eq!(is_known_svn_dir_prop("svn:executable").unwrap(), false);
        assert_eq!(is_known_svn_dir_prop("custom:property").unwrap(), false);
    }

    #[test]
    fn test_needs_translation_true() {
        assert_eq!(needs_translation("svn:log").unwrap(), true);
        assert_eq!(needs_translation("svn:externals").unwrap(), true);
        assert_eq!(needs_translation("svn:executable").unwrap(), true);
    }

    #[test]
    fn test_name_is_valid_true() {
        assert_eq!(name_is_valid("svn:mime-type"), true);
        assert_eq!(name_is_valid("custom:property"), true);
        assert_eq!(name_is_valid("user-property"), true);
    }

    #[test]
    fn test_prophash_from_hashmap() {
        let pool = apr::Pool::new();
        let mut props = std::collections::HashMap::new();
        props.insert("svn:mime-type".to_string(), b"text/plain".to_vec());
        props.insert("custom:prop".to_string(), b"value".to_vec());

        let prop_hash = PropHash::from_hashmap(&props, &pool);

        assert_eq!(prop_hash.len(), 2);
        assert_eq!(
            prop_hash.get_string("svn:mime-type"),
            Some("text/plain".to_string())
        );
        assert_eq!(
            prop_hash.get_string("custom:prop"),
            Some("value".to_string())
        );
    }

    #[test]
    fn test_prophash_duplicate() {
        let pool1 = apr::Pool::new();
        let pool2 = apr::Pool::new();

        let mut props = std::collections::HashMap::new();
        props.insert("svn:mime-type".to_string(), b"text/plain".to_vec());
        props.insert("custom:prop".to_string(), b"value".to_vec());

        let prop_hash = PropHash::from_hashmap(&props, &pool1);
        let duplicated = prop_hash.duplicate(&pool2);

        assert_eq!(duplicated.len(), 2);
        assert_eq!(
            duplicated.get_string("svn:mime-type"),
            Some("text/plain".to_string())
        );
        assert_eq!(
            duplicated.get_string("custom:prop"),
            Some("value".to_string())
        );
    }

    #[test]
    fn test_prophash_roundtrip() {
        let pool = apr::Pool::new();
        let mut original = std::collections::HashMap::new();
        original.insert("svn:mime-type".to_string(), b"text/plain".to_vec());
        original.insert("custom:prop".to_string(), b"value".to_vec());

        let prop_hash = PropHash::from_hashmap(&original, &pool);
        let roundtrip = prop_hash.to_hashmap();

        assert_eq!(roundtrip, original);
    }
}
