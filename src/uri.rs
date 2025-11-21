use crate::{with_tmp_pool, Canonical};

/// A URI string - by default returns owned String, borrowed variants behind features
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Uri(String);

impl Uri {
    /// Create a new URI from a string, canonicalizing it
    pub fn new(uri: &str) -> Result<Self, crate::Error> {
        let canonical = canonicalize_uri(uri)?;
        Ok(Uri(canonical))
    }

    /// Get the URI as a string
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Check if this URI is a root URI (has no path component beyond "/")
    pub fn is_root(&self) -> bool {
        unsafe {
            let uri_cstr = std::ffi::CString::new(self.0.as_str()).unwrap();
            subversion_sys::svn_uri_is_root(uri_cstr.as_ptr(), self.0.len()) != 0
        }
    }

    /// Get the canonical form of this URI
    pub fn canonical(&self) -> Canonical<Uri> {
        // Already canonical since we canonicalize on construction
        Canonical(self.clone())
    }
}

impl std::fmt::Display for Uri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::str::FromStr for Uri {
    type Err = crate::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl AsRef<str> for Uri {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Canonicalize a URI string using SVN's canonicalization rules
pub fn canonicalize_uri(uri: &str) -> Result<String, crate::Error> {
    with_tmp_pool(|pool| unsafe {
        let uri_cstr = std::ffi::CString::new(uri)?;
        let canonical = subversion_sys::svn_uri_canonicalize(uri_cstr.as_ptr(), pool.as_mut_ptr());
        let canonical_cstr = std::ffi::CStr::from_ptr(canonical);
        Ok(canonical_cstr.to_str()?.to_owned())
    })
}

/// Get the longest common ancestor of two URIs
pub fn get_longest_ancestor(uri1: &str, uri2: &str) -> Result<String, crate::Error> {
    with_tmp_pool(|pool| unsafe {
        let uri1_cstr = std::ffi::CString::new(uri1)?;
        let uri2_cstr = std::ffi::CString::new(uri2)?;
        let ancestor = subversion_sys::svn_uri_get_longest_ancestor(
            uri1_cstr.as_ptr(),
            uri2_cstr.as_ptr(),
            pool.as_mut_ptr(),
        );
        let ancestor_cstr = std::ffi::CStr::from_ptr(ancestor);
        Ok(ancestor_cstr.to_str()?.to_owned())
    })
}

/// Check if one URI is an ancestor of another
pub fn is_ancestor(ancestor: &str, path: &str) -> bool {
    unsafe {
        let ancestor_cstr = std::ffi::CString::new(ancestor).unwrap();
        let path_cstr = std::ffi::CString::new(path).unwrap();
        subversion_sys::svn_uri__is_ancestor(ancestor_cstr.as_ptr(), path_cstr.as_ptr()) != 0
    }
}

/// Skip the ancestor portion of a URI, returning the remainder
///
/// Returns None if `ancestor` is not an ancestor of `path`.
pub fn skip_ancestor(ancestor: &str, path: &str) -> Option<String> {
    with_tmp_pool(|pool| unsafe {
        let ancestor_cstr = std::ffi::CString::new(ancestor).ok()?;
        let path_cstr = std::ffi::CString::new(path).ok()?;
        let result = subversion_sys::svn_uri_skip_ancestor(
            ancestor_cstr.as_ptr(),
            path_cstr.as_ptr(),
            pool.as_mut_ptr(),
        );
        if result.is_null() {
            None
        } else {
            let result_cstr = std::ffi::CStr::from_ptr(result);
            Some(result_cstr.to_str().ok()?.to_owned())
        }
    })
}

/// Get the basename (final component) of a URI
pub fn basename(uri: &str) -> Result<String, crate::Error> {
    with_tmp_pool(|pool| unsafe {
        let uri_cstr = std::ffi::CString::new(uri)?;
        let basename = subversion_sys::svn_uri_basename(uri_cstr.as_ptr(), pool.as_mut_ptr());
        let basename_cstr = std::ffi::CStr::from_ptr(basename);
        Ok(basename_cstr.to_str()?.to_owned())
    })
}

/// Get the dirname (parent directory) of a URI
pub fn dirname(uri: &str) -> Result<String, crate::Error> {
    with_tmp_pool(|pool| unsafe {
        let uri_cstr = std::ffi::CString::new(uri)?;
        let dirname = subversion_sys::svn_uri_dirname(uri_cstr.as_ptr(), pool.as_mut_ptr());
        let dirname_cstr = std::ffi::CStr::from_ptr(dirname);
        Ok(dirname_cstr.to_str()?.to_owned())
    })
}

/// Split a URI into its dirname and basename components
///
/// Returns (dirname, basename)
pub fn split(uri: &str) -> Result<(String, String), crate::Error> {
    with_tmp_pool(|pool| unsafe {
        let uri_cstr = std::ffi::CString::new(uri)?;
        let mut dirname: *const i8 = std::ptr::null();
        let mut basename: *const i8 = std::ptr::null();

        subversion_sys::svn_uri_split(
            &mut dirname,
            &mut basename,
            uri_cstr.as_ptr(),
            pool.as_mut_ptr(),
        );

        let dirname_cstr = std::ffi::CStr::from_ptr(dirname);
        let basename_cstr = std::ffi::CStr::from_ptr(basename);

        Ok((
            dirname_cstr.to_str()?.to_owned(),
            basename_cstr.to_str()?.to_owned(),
        ))
    })
}

/// Check if a URI is in canonical form
pub fn is_canonical(uri: &str) -> bool {
    with_tmp_pool(|pool| unsafe {
        let uri_cstr = std::ffi::CString::new(uri).unwrap();
        subversion_sys::svn_uri_is_canonical(uri_cstr.as_ptr(), pool.as_mut_ptr()) != 0
    })
}

/// Trait for types that can be converted to canonical URIs
pub trait AsCanonicalUri {
    /// Convert to a canonical URI
    fn as_canonical_uri(&self) -> Result<Canonical<Uri>, crate::Error>;
}

impl AsCanonicalUri for Uri {
    fn as_canonical_uri(&self) -> Result<Canonical<Uri>, crate::Error> {
        Ok(self.canonical())
    }
}

impl AsCanonicalUri for Canonical<Uri> {
    fn as_canonical_uri(&self) -> Result<Canonical<Uri>, crate::Error> {
        Ok(self.clone())
    }
}

impl AsCanonicalUri for &str {
    fn as_canonical_uri(&self) -> Result<Canonical<Uri>, crate::Error> {
        let uri = Uri::new(self)?;
        Ok(uri.canonical())
    }
}

impl AsCanonicalUri for String {
    fn as_canonical_uri(&self) -> Result<Canonical<Uri>, crate::Error> {
        self.as_str().as_canonical_uri()
    }
}

#[cfg(feature = "url")]
impl AsCanonicalUri for url::Url {
    fn as_canonical_uri(&self) -> Result<Canonical<Uri>, crate::Error> {
        self.as_str().as_canonical_uri()
    }
}

#[cfg(feature = "url")]
impl AsCanonicalUri for &url::Url {
    fn as_canonical_uri(&self) -> Result<Canonical<Uri>, crate::Error> {
        self.as_str().as_canonical_uri()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uri_new_valid() {
        let result = Uri::new("http://example.com/path");
        assert!(result.is_ok());
        let uri = result.unwrap();
        assert_eq!(uri.as_str(), "http://example.com/path");
    }

    #[test]
    fn test_uri_canonicalization() {
        let uri = Uri::new("http://example.com//double//slashes").unwrap();
        assert_eq!(uri.as_str(), "http://example.com/double/slashes");
    }

    #[test]
    fn test_uri_is_root_true() {
        let uri = Uri::new("http://example.com/").unwrap();
        assert_eq!(uri.is_root(), true);
    }

    #[test]
    fn test_uri_is_root_false() {
        let uri = Uri::new("http://example.com/path").unwrap();
        assert_eq!(uri.is_root(), false);
    }

    #[test]
    fn test_uri_display() {
        let uri = Uri::new("http://example.com/path").unwrap();
        let displayed = format!("{}", uri);
        assert_eq!(displayed, "http://example.com/path");
    }

    #[test]
    fn test_uri_from_str() {
        let uri: Uri = "http://example.com/path".parse().unwrap();
        assert_eq!(uri.as_str(), "http://example.com/path");
    }

    #[test]
    fn test_uri_as_ref() {
        let uri = Uri::new("http://example.com/path").unwrap();
        let s: &str = uri.as_ref();
        assert_eq!(s, "http://example.com/path");
    }

    #[test]
    fn test_canonicalize_uri() {
        let result = canonicalize_uri("http://example.com//path");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "http://example.com/path");
    }

    #[test]
    fn test_canonical_uri_from_str() {
        let result = "http://example.com/path".as_canonical_uri();
        assert!(result.is_ok());
        let canonical = result.unwrap();
        assert_eq!(canonical.0.as_str(), "http://example.com/path");
    }

    #[test]
    fn test_canonical_uri_from_string() {
        let s = String::from("http://example.com/path");
        let result = s.as_canonical_uri();
        assert!(result.is_ok());
        let canonical = result.unwrap();
        assert_eq!(canonical.0.as_str(), "http://example.com/path");
    }

    #[test]
    fn test_canonical_uri_from_uri() {
        let uri = Uri::new("http://example.com/path").unwrap();
        let result = uri.as_canonical_uri();
        assert!(result.is_ok());
        let canonical = result.unwrap();
        assert_eq!(canonical.0.as_str(), "http://example.com/path");
    }

    #[test]
    #[cfg(feature = "url")]
    fn test_canonical_uri_from_url() {
        let url = url::Url::parse("http://example.com/path").unwrap();
        let result = url.as_canonical_uri();
        assert!(result.is_ok());
        let canonical = result.unwrap();
        assert_eq!(canonical.0.as_str(), "http://example.com/path");
    }

    #[test]
    fn test_get_longest_ancestor() {
        let result =
            get_longest_ancestor("http://example.com/a/b/c", "http://example.com/a/b/d").unwrap();
        assert_eq!(result, "http://example.com/a/b");
    }

    #[test]
    fn test_get_longest_ancestor_no_common() {
        let result = get_longest_ancestor("http://example.com/a", "http://other.com/b").unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_is_ancestor() {
        assert!(is_ancestor(
            "http://example.com/a",
            "http://example.com/a/b/c"
        ));
        assert!(!is_ancestor(
            "http://example.com/a/b",
            "http://example.com/a"
        ));
    }

    #[test]
    fn test_skip_ancestor() {
        let result = skip_ancestor("http://example.com/a", "http://example.com/a/b/c");
        assert_eq!(result, Some("b/c".to_string()));
    }

    #[test]
    fn test_skip_ancestor_not_ancestor() {
        let result = skip_ancestor("http://example.com/x", "http://example.com/a/b");
        assert_eq!(result, None);
    }

    #[test]
    fn test_basename() {
        let result = basename("http://example.com/a/b/c");
        assert_eq!(result.unwrap(), "c");
    }

    #[test]
    fn test_basename_root() {
        // Root URIs don't have a trailing slash in canonical form
        let result = basename("http://example.com");
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    fn test_dirname() {
        let result = dirname("http://example.com/a/b/c");
        assert_eq!(result.unwrap(), "http://example.com/a/b");
    }

    #[test]
    fn test_dirname_root() {
        let result = dirname("http://example.com/a");
        assert_eq!(result.unwrap(), "http://example.com");
    }

    #[test]
    fn test_split() {
        let result = split("http://example.com/a/b/c");
        let (dir, base) = result.unwrap();
        assert_eq!(dir, "http://example.com/a/b");
        assert_eq!(base, "c");
    }

    #[test]
    fn test_is_canonical_true() {
        assert!(is_canonical("http://example.com/path"));
    }

    #[test]
    fn test_is_canonical_false() {
        assert!(!is_canonical("http://example.com//double//slashes"));
    }
}
