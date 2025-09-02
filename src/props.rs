#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Entry,
    Wc,
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

pub fn kind(name: &str) -> Kind {
    let name = std::ffi::CString::new(name).unwrap();
    unsafe { subversion_sys::svn_property_kind2(name.as_ptr()) }.into()
}

pub fn is_svn_prop(name: &str) -> bool {
    let name = std::ffi::CString::new(name).unwrap();
    unsafe { subversion_sys::svn_prop_is_svn_prop(name.as_ptr()) != 0 }
}

pub fn is_boolean(name: &str) -> bool {
    let name = std::ffi::CString::new(name).unwrap();
    unsafe { subversion_sys::svn_prop_is_boolean(name.as_ptr()) != 0 }
}

pub fn is_known_svn_rev_prop(name: &str) -> bool {
    let name = std::ffi::CString::new(name).unwrap();
    unsafe { subversion_sys::svn_prop_is_known_svn_rev_prop(name.as_ptr()) != 0 }
}

pub fn is_known_svn_node_prop(name: &str) -> bool {
    let name = std::ffi::CString::new(name).unwrap();
    unsafe { subversion_sys::svn_prop_is_known_svn_node_prop(name.as_ptr()) != 0 }
}

pub fn is_known_svn_file_prop(name: &str) -> bool {
    let name = std::ffi::CString::new(name).unwrap();
    unsafe { subversion_sys::svn_prop_is_known_svn_file_prop(name.as_ptr()) != 0 }
}

pub fn is_known_svn_dir_prop(name: &str) -> bool {
    let name = std::ffi::CString::new(name).unwrap();
    unsafe { subversion_sys::svn_prop_is_known_svn_dir_prop(name.as_ptr()) != 0 }
}

pub fn needs_translation(name: &str) -> bool {
    let name = std::ffi::CString::new(name).unwrap();
    unsafe { subversion_sys::svn_prop_needs_translation(name.as_ptr()) != 0 }
}

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

    /// Convert the properties to a HashMap<String, Vec<u8>>
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
    pub fn iter_strings(&self) -> impl Iterator<Item = (&str, std::borrow::Cow<str>)> {
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
