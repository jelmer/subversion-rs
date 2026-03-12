//! Path manipulation utilities for Subversion paths and URLs.
//!
//! This module provides utilities for working with filesystem paths, relative paths,
//! and distinguishing between local paths and URLs.

use crate::with_tmp_pool;

/// Check if a path string looks like a valid absolute URL.
///
/// This is a simple check based on the path format, not validation of the URL structure.
/// Returns true for paths that start with a scheme like "http://", "svn://", "file://", etc.
///
/// Wraps `svn_path_is_url`.
///
/// # Examples
///
/// ```
/// use subversion::path;
///
/// assert!(path::is_url("http://example.com/repo"));
/// assert!(path::is_url("svn://svn.example.com/repo"));
/// assert!(path::is_url("file:///var/svn/repo"));
/// assert!(!path::is_url("/usr/local/repo"));
/// assert!(!path::is_url("./relative/path"));
/// ```
pub fn is_url(path: &str) -> bool {
    unsafe {
        let path_cstr = std::ffi::CString::new(path).unwrap();
        subversion_sys::svn_path_is_url(path_cstr.as_ptr()) != 0
    }
}

/// Check if a path string is URI-safe (contains only URI-safe characters).
///
/// Wraps `svn_path_is_uri_safe`.
pub fn is_uri_safe(path: &str) -> bool {
    unsafe {
        let path_cstr = std::ffi::CString::new(path).unwrap();
        subversion_sys::svn_path_is_uri_safe(path_cstr.as_ptr()) != 0
    }
}

/// Join two relative path components.
///
/// This function intelligently joins relative path segments, handling edge cases
/// like empty components and ensuring proper separator placement.
///
/// Wraps `svn_relpath_join`.
///
/// # Examples
///
/// ```
/// use subversion::path;
///
/// assert_eq!(path::relpath_join("a/b", "c/d").unwrap(), "a/b/c/d");
/// assert_eq!(path::relpath_join("a/b", "").unwrap(), "a/b");
/// assert_eq!(path::relpath_join("", "c/d").unwrap(), "c/d");
/// ```
pub fn relpath_join(base: &str, component: &str) -> Result<String, crate::Error<'static>> {
    with_tmp_pool(|pool| unsafe {
        let base_cstr = std::ffi::CString::new(base)?;
        let component_cstr = std::ffi::CString::new(component)?;

        let result = subversion_sys::svn_relpath_join(
            base_cstr.as_ptr(),
            component_cstr.as_ptr(),
            pool.as_mut_ptr(),
        );

        let result_cstr = std::ffi::CStr::from_ptr(result);
        Ok(result_cstr.to_str()?.to_owned())
    })
}

/// Get the basename (final component) of a relative path.
///
/// Wraps `svn_relpath_basename`.
pub fn relpath_basename(relpath: &str) -> Result<String, crate::Error<'static>> {
    with_tmp_pool(|pool| unsafe {
        let relpath_cstr = std::ffi::CString::new(relpath)?;
        let basename =
            subversion_sys::svn_relpath_basename(relpath_cstr.as_ptr(), pool.as_mut_ptr());
        let basename_cstr = std::ffi::CStr::from_ptr(basename);
        Ok(basename_cstr.to_str()?.to_owned())
    })
}

/// Get the dirname (parent directory) of a relative path.
///
/// Wraps `svn_relpath_dirname`.
pub fn relpath_dirname(relpath: &str) -> Result<String, crate::Error<'static>> {
    with_tmp_pool(|pool| unsafe {
        let relpath_cstr = std::ffi::CString::new(relpath)?;
        let dirname = subversion_sys::svn_relpath_dirname(relpath_cstr.as_ptr(), pool.as_mut_ptr());
        let dirname_cstr = std::ffi::CStr::from_ptr(dirname);
        Ok(dirname_cstr.to_str()?.to_owned())
    })
}

/// Join a base directory path with a component.
///
/// This function joins absolute or relative directory paths (not URIs).
/// For joining URLs, use the uri module functions. For joining relative
/// paths, use `relpath_join()`.
///
/// Wraps `svn_dirent_join`.
///
/// # Examples
///
/// ```
/// use subversion::path;
///
/// assert_eq!(path::dirent_join("/usr/local", "bin").unwrap(), "/usr/local/bin");
/// assert_eq!(path::dirent_join("/usr/local", "").unwrap(), "/usr/local");
/// ```
pub fn dirent_join(base: &str, component: &str) -> Result<String, crate::Error<'static>> {
    with_tmp_pool(|pool| unsafe {
        let base_cstr = std::ffi::CString::new(base)?;
        let component_cstr = std::ffi::CString::new(component)?;

        let result = subversion_sys::svn_dirent_join(
            base_cstr.as_ptr(),
            component_cstr.as_ptr(),
            pool.as_mut_ptr(),
        );

        let result_cstr = std::ffi::CStr::from_ptr(result);
        Ok(result_cstr.to_str()?.to_owned())
    })
}

/// Get the basename (final component) of a directory path.
///
/// Wraps `svn_dirent_basename`.
pub fn dirent_basename(dirent: &str) -> Result<String, crate::Error<'static>> {
    with_tmp_pool(|pool| unsafe {
        let dirent_cstr = std::ffi::CString::new(dirent)?;
        let basename = subversion_sys::svn_dirent_basename(dirent_cstr.as_ptr(), pool.as_mut_ptr());
        let basename_cstr = std::ffi::CStr::from_ptr(basename);
        Ok(basename_cstr.to_str()?.to_owned())
    })
}

/// Get the dirname (parent directory) of a directory path.
///
/// Wraps `svn_dirent_dirname`.
pub fn dirent_dirname(dirent: &str) -> Result<String, crate::Error<'static>> {
    with_tmp_pool(|pool| unsafe {
        let dirent_cstr = std::ffi::CString::new(dirent)?;
        let dirname = subversion_sys::svn_dirent_dirname(dirent_cstr.as_ptr(), pool.as_mut_ptr());
        let dirname_cstr = std::ffi::CStr::from_ptr(dirname);
        Ok(dirname_cstr.to_str()?.to_owned())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_url_http() {
        assert!(is_url("http://example.com/repo"));
    }

    #[test]
    fn test_is_url_https() {
        assert!(is_url("https://example.com/repo"));
    }

    #[test]
    fn test_is_url_svn() {
        assert!(is_url("svn://svn.example.com/repo"));
    }

    #[test]
    fn test_is_url_file() {
        assert!(is_url("file:///var/svn/repo"));
    }

    #[test]
    fn test_is_url_absolute_path() {
        assert!(!is_url("/usr/local/repo"));
    }

    #[test]
    fn test_is_url_relative_path() {
        assert!(!is_url("./relative/path"));
        assert!(!is_url("relative/path"));
    }

    #[test]
    fn test_is_uri_safe_basic() {
        // is_uri_safe checks if a URI is already properly formatted and doesn't need escaping
        assert!(is_uri_safe("http://example.com"));
        assert!(is_uri_safe("http://example.com/path/to/file"));
        assert!(!is_uri_safe("http%3A%2F%2Fexample.com")); // already escaped - not "safe"
        assert!(!is_uri_safe("not a uri")); // not a URI at all
    }

    #[test]
    fn test_relpath_join_basic() {
        let result = relpath_join("a/b", "c/d").unwrap();
        assert_eq!(result, "a/b/c/d");
    }

    #[test]
    fn test_relpath_join_empty_base() {
        let result = relpath_join("", "c/d").unwrap();
        assert_eq!(result, "c/d");
    }

    #[test]
    fn test_relpath_join_empty_component() {
        let result = relpath_join("a/b", "").unwrap();
        assert_eq!(result, "a/b");
    }

    #[test]
    fn test_relpath_basename() {
        let result = relpath_basename("a/b/c").unwrap();
        assert_eq!(result, "c");
    }

    #[test]
    fn test_relpath_dirname() {
        let result = relpath_dirname("a/b/c").unwrap();
        assert_eq!(result, "a/b");
    }

    #[test]
    fn test_dirent_join_basic() {
        let result = dirent_join("/usr/local", "bin").unwrap();
        assert_eq!(result, "/usr/local/bin");
    }

    #[test]
    fn test_dirent_join_empty_component() {
        let result = dirent_join("/usr/local", "").unwrap();
        assert_eq!(result, "/usr/local");
    }

    #[test]
    fn test_dirent_basename() {
        let result = dirent_basename("/usr/local/bin").unwrap();
        assert_eq!(result, "bin");
    }

    #[test]
    fn test_dirent_dirname() {
        let result = dirent_dirname("/usr/local/bin").unwrap();
        assert_eq!(result, "/usr/local");
    }
}
