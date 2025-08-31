use crate::{with_tmp_pool, Canonical};

/// A directory path - by default returns owned PathBuf, borrowed variants behind features  
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dirent(std::path::PathBuf);

impl Dirent {
    /// Create a new Dirent from a path, canonicalizing it
    pub fn new(path: impl AsRef<std::path::Path>) -> Result<Self, crate::Error> {
        let canonical = canonicalize_dirent(path.as_ref())?;
        Ok(Dirent(canonical))
    }

    /// Get the path as a PathBuf
    pub fn as_path(&self) -> &std::path::Path {
        &self.0
    }

    /// Get the path as a string
    pub fn as_str(&self) -> &str {
        self.0.to_str().unwrap_or("")
    }

    /// Get the canonical form of this path
    pub fn canonical(&self) -> Canonical<Dirent> {
        // Already canonical since we canonicalize on construction
        Canonical(self.clone())
    }

    /// Join this dirent with another path component using SVN's rules
    pub fn join(&self, component: impl AsRef<std::path::Path>) -> Result<Dirent, crate::Error> {
        join_dirents(self, component)
    }

    /// Get the basename (final component) of this dirent
    pub fn basename(&self) -> Result<Dirent, crate::Error> {
        basename_dirent(self)
    }

    /// Get the dirname (directory component) of this dirent
    pub fn dirname(&self) -> Result<Dirent, crate::Error> {
        dirname_dirent(self)
    }

    /// Split this dirent into dirname and basename components
    pub fn split(&self) -> Result<(Dirent, Dirent), crate::Error> {
        split_dirent(self)
    }

    /// Check if this dirent is absolute
    pub fn is_absolute(&self) -> bool {
        // Since we have a validated Dirent, this should never fail
        is_absolute_dirent(self).unwrap_or(false)
    }

    /// Check if this dirent is a root path
    pub fn is_root(&self) -> bool {
        // Since we have a validated Dirent, this should never fail
        is_root_dirent(self).unwrap_or(false)
    }

    /// Check if this dirent is canonical (should always be true for Dirent)
    pub fn is_canonical(&self) -> bool {
        // Since we canonicalize on construction, this should always be true
        is_canonical_dirent(self).unwrap_or(true)
    }

    /// Get the absolute path from this dirent
    pub fn get_absolute(&self) -> Result<Dirent, crate::Error> {
        get_absolute_dirent(self)
    }
}

impl std::fmt::Display for Dirent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.display().fmt(f)
    }
}

impl AsRef<std::path::Path> for Dirent {
    fn as_ref(&self) -> &std::path::Path {
        &self.0
    }
}

impl From<std::path::PathBuf> for Dirent {
    fn from(path: std::path::PathBuf) -> Self {
        // Note: This doesn't canonicalize - use new() for that
        Dirent(path)
    }
}

/// Validate that a path string is safe for SVN operations
fn validate_path_string(path_str: &str) -> Result<(), crate::Error> {
    // Check for null bytes which would cause C string conversion to fail
    if path_str.contains('\0') {
        return Err(crate::Error::from_str("Path contains null bytes"));
    }

    // Check for extremely long paths that might cause issues
    if path_str.len() > 4096 {
        return Err(crate::Error::from_str("Path too long"));
    }

    // Check for some obviously problematic patterns
    if path_str.contains("\x7f") || path_str.contains("\r") || path_str.contains("\n") {
        return Err(crate::Error::from_str(
            "Path contains invalid control characters",
        ));
    }

    Ok(())
}

/// Canonicalize a directory path using SVN's canonicalization rules
pub fn canonicalize_dirent(path: &std::path::Path) -> Result<std::path::PathBuf, crate::Error> {
    with_tmp_pool(|pool| unsafe {
        let path_str = path
            .to_str()
            .ok_or_else(|| crate::Error::from_str("Invalid path encoding"))?;

        // Validate the path string before passing to SVN C functions
        validate_path_string(path_str)?;

        let path_cstr = std::ffi::CString::new(path_str)?;
        let canonical =
            subversion_sys::svn_dirent_canonicalize(path_cstr.as_ptr(), pool.as_mut_ptr());
        let canonical_cstr = std::ffi::CStr::from_ptr(canonical);
        let canonical_str = canonical_cstr.to_str()?;
        Ok(std::path::PathBuf::from(canonical_str))
    })
}

/// Trait for types that can be converted to canonical directory paths
pub trait AsCanonicalDirent {
    /// Convert to a canonical Dirent
    fn as_canonical_dirent(&self) -> Result<Canonical<Dirent>, crate::Error>;
}

impl AsCanonicalDirent for Dirent {
    fn as_canonical_dirent(&self) -> Result<Canonical<Dirent>, crate::Error> {
        Ok(self.canonical())
    }
}

impl AsCanonicalDirent for Canonical<Dirent> {
    fn as_canonical_dirent(&self) -> Result<Canonical<Dirent>, crate::Error> {
        Ok(self.clone())
    }
}

impl AsCanonicalDirent for &str {
    fn as_canonical_dirent(&self) -> Result<Canonical<Dirent>, crate::Error> {
        let dirent = Dirent::new(std::path::Path::new(self))?;
        Ok(dirent.canonical())
    }
}

impl AsCanonicalDirent for String {
    fn as_canonical_dirent(&self) -> Result<Canonical<Dirent>, crate::Error> {
        self.as_str().as_canonical_dirent()
    }
}

impl AsCanonicalDirent for &std::path::Path {
    fn as_canonical_dirent(&self) -> Result<Canonical<Dirent>, crate::Error> {
        let dirent = Dirent::new(self)?;
        Ok(dirent.canonical())
    }
}

impl AsCanonicalDirent for std::path::PathBuf {
    fn as_canonical_dirent(&self) -> Result<Canonical<Dirent>, crate::Error> {
        self.as_path().as_canonical_dirent()
    }
}

impl AsCanonicalDirent for &std::path::PathBuf {
    fn as_canonical_dirent(&self) -> Result<Canonical<Dirent>, crate::Error> {
        self.as_path().as_canonical_dirent()
    }
}

/// Join two directory path components using SVN's rules (type-safe version)
pub fn join_dirents(
    base: &Dirent,
    component: impl AsRef<std::path::Path>,
) -> Result<Dirent, crate::Error> {
    with_tmp_pool(|pool| unsafe {
        let component_str = component
            .as_ref()
            .to_str()
            .ok_or_else(|| crate::Error::from_str("Invalid component path"))?;
        validate_path_string(component_str)?;

        // base is already canonical since it's a Dirent
        let base_cstr = std::ffi::CString::new(base.as_str())?;
        let component_cstr = std::ffi::CString::new(component_str)?;

        let joined = subversion_sys::svn_dirent_join(
            base_cstr.as_ptr(),
            component_cstr.as_ptr(),
            pool.as_mut_ptr(),
        );

        let joined_cstr = std::ffi::CStr::from_ptr(joined);
        let joined_str = joined_cstr.to_str()?;
        Ok(Dirent(std::path::PathBuf::from(joined_str)))
    })
}

/// Get the basename (final component) of a directory path (type-safe version)
pub fn basename_dirent(dirent: &Dirent) -> Result<Dirent, crate::Error> {
    with_tmp_pool(|pool| unsafe {
        // dirent is already canonical
        let path_cstr = std::ffi::CString::new(dirent.as_str())?;

        let basename = subversion_sys::svn_dirent_basename(path_cstr.as_ptr(), pool.as_mut_ptr());
        let basename_cstr = std::ffi::CStr::from_ptr(basename);
        let basename_str = basename_cstr.to_str()?;
        Ok(Dirent(std::path::PathBuf::from(basename_str)))
    })
}

/// Get the dirname (directory component) of a directory path (type-safe version)
pub fn dirname_dirent(dirent: &Dirent) -> Result<Dirent, crate::Error> {
    with_tmp_pool(|pool| unsafe {
        // dirent is already canonical
        let path_cstr = std::ffi::CString::new(dirent.as_str())?;

        let dirname = subversion_sys::svn_dirent_dirname(path_cstr.as_ptr(), pool.as_mut_ptr());
        let dirname_cstr = std::ffi::CStr::from_ptr(dirname);
        let dirname_str = dirname_cstr.to_str()?;
        Ok(Dirent(std::path::PathBuf::from(dirname_str)))
    })
}

/// Split a directory path into dirname and basename components (type-safe version)
pub fn split_dirent(dirent: &Dirent) -> Result<(Dirent, Dirent), crate::Error> {
    with_tmp_pool(|pool| unsafe {
        // dirent is already canonical
        let path_cstr = std::ffi::CString::new(dirent.as_str())?;

        let mut dirname_ptr: *const std::ffi::c_char = std::ptr::null();
        let mut basename_ptr: *const std::ffi::c_char = std::ptr::null();

        subversion_sys::svn_dirent_split(
            &mut dirname_ptr,
            &mut basename_ptr,
            path_cstr.as_ptr(),
            pool.as_mut_ptr(),
        );

        let dirname_str = std::ffi::CStr::from_ptr(dirname_ptr).to_str()?;
        let basename_str = std::ffi::CStr::from_ptr(basename_ptr).to_str()?;

        Ok((
            Dirent(std::path::PathBuf::from(dirname_str)),
            Dirent(std::path::PathBuf::from(basename_str)),
        ))
    })
}

/// Check if a directory path is absolute (type-safe version)
pub fn is_absolute_dirent(dirent: &Dirent) -> Result<bool, crate::Error> {
    let path_cstr = std::ffi::CString::new(dirent.as_str())?;

    unsafe {
        let result = subversion_sys::svn_dirent_is_absolute(path_cstr.as_ptr());
        Ok(result != 0)
    }
}

/// Check if a directory path is a root path (type-safe version)
pub fn is_root_dirent(dirent: &Dirent) -> Result<bool, crate::Error> {
    let path_str = dirent.as_str();
    let path_cstr = std::ffi::CString::new(path_str)?;

    unsafe {
        let result = subversion_sys::svn_dirent_is_root(path_cstr.as_ptr(), path_str.len());
        Ok(result != 0)
    }
}

/// Check if a directory path is canonical (type-safe version)
pub fn is_canonical_dirent(dirent: &Dirent) -> Result<bool, crate::Error> {
    with_tmp_pool(|pool| unsafe {
        let path_cstr = std::ffi::CString::new(dirent.as_str())?;

        let result = subversion_sys::svn_dirent_is_canonical(path_cstr.as_ptr(), pool.as_mut_ptr());
        Ok(result != 0)
    })
}

/// Get the absolute path from a relative path (type-safe version)
pub fn get_absolute_dirent(dirent: &Dirent) -> Result<Dirent, crate::Error> {
    with_tmp_pool(|pool| unsafe {
        let path_cstr = std::ffi::CString::new(dirent.as_str())?;
        let mut absolute_ptr: *const std::ffi::c_char = std::ptr::null();

        let err = subversion_sys::svn_dirent_get_absolute(
            &mut absolute_ptr,
            path_cstr.as_ptr(),
            pool.as_mut_ptr(),
        );

        crate::Error::from_raw(err)?;

        let absolute_cstr = std::ffi::CStr::from_ptr(absolute_ptr);
        let absolute_str = absolute_cstr.to_str()?;
        Ok(Dirent(std::path::PathBuf::from(absolute_str)))
    })
}

/// Get the longest common ancestor of two directory paths
pub fn get_longest_ancestor(dirent1: &Dirent, dirent2: &Dirent) -> Result<Dirent, crate::Error> {
    with_tmp_pool(|pool| unsafe {
        let path1_cstr = std::ffi::CString::new(dirent1.as_str())?;
        let path2_cstr = std::ffi::CString::new(dirent2.as_str())?;

        let ancestor = subversion_sys::svn_dirent_get_longest_ancestor(
            path1_cstr.as_ptr(),
            path2_cstr.as_ptr(),
            pool.as_mut_ptr(),
        );

        let ancestor_cstr = std::ffi::CStr::from_ptr(ancestor);
        let ancestor_str = ancestor_cstr.to_str()?;
        Ok(Dirent(std::path::PathBuf::from(ancestor_str)))
    })
}

/// Check if the child path is a child of the parent path
pub fn is_child(parent: &Dirent, child: &Dirent) -> Result<Option<Dirent>, crate::Error> {
    with_tmp_pool(|pool| unsafe {
        let parent_cstr = std::ffi::CString::new(parent.as_str())?;
        let child_cstr = std::ffi::CString::new(child.as_str())?;

        let result = subversion_sys::svn_dirent_is_child(
            parent_cstr.as_ptr(),
            child_cstr.as_ptr(),
            pool.as_mut_ptr(),
        );

        if result.is_null() {
            Ok(None)
        } else {
            let result_cstr = std::ffi::CStr::from_ptr(result);
            let result_str = result_cstr.to_str()?;
            Ok(Some(Dirent(std::path::PathBuf::from(result_str))))
        }
    })
}

/// Check if one path is an ancestor of another path
pub fn is_ancestor(ancestor: &Dirent, path: &Dirent) -> Result<bool, crate::Error> {
    let ancestor_cstr = std::ffi::CString::new(ancestor.as_str())?;
    let path_cstr = std::ffi::CString::new(path.as_str())?;

    unsafe {
        let result =
            subversion_sys::svn_dirent_is_ancestor(ancestor_cstr.as_ptr(), path_cstr.as_ptr());
        Ok(result != 0)
    }
}

/// Skip the ancestor part of a path, returning the remaining child portion
pub fn skip_ancestor(ancestor: &Dirent, path: &Dirent) -> Result<Option<Dirent>, crate::Error> {
    let ancestor_cstr = std::ffi::CString::new(ancestor.as_str())?;
    let path_cstr = std::ffi::CString::new(path.as_str())?;

    unsafe {
        let result =
            subversion_sys::svn_dirent_skip_ancestor(ancestor_cstr.as_ptr(), path_cstr.as_ptr());

        if result.is_null() {
            Ok(None)
        } else {
            let result_cstr = std::ffi::CStr::from_ptr(result);
            let result_str = result_cstr.to_str()?;
            Ok(Some(Dirent(std::path::PathBuf::from(result_str))))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_join() {
        let base = Dirent::new("/home/user").unwrap();
        let result = base.join("project").unwrap();
        assert_eq!(result.as_path(), std::path::Path::new("/home/user/project"));

        let base = Dirent::new("/home/user/").unwrap();
        let result = base.join("project").unwrap();
        assert_eq!(result.as_path(), std::path::Path::new("/home/user/project"));

        let base = Dirent::new("").unwrap();
        let result = base.join("project").unwrap();
        assert_eq!(result.as_path(), std::path::Path::new("project"));
    }

    #[test]
    fn test_basename() {
        let dirent = Dirent::new("/home/user/project").unwrap();
        let result = dirent.basename().unwrap();
        assert_eq!(result.as_path(), std::path::Path::new("project"));

        let dirent = Dirent::new("/home/user/project/").unwrap();
        let result = dirent.basename().unwrap();
        assert_eq!(result.as_path(), std::path::Path::new("project"));

        let dirent = Dirent::new("project").unwrap();
        let result = dirent.basename().unwrap();
        assert_eq!(result.as_path(), std::path::Path::new("project"));
    }

    #[test]
    fn test_dirname() {
        let dirent = Dirent::new("/home/user/project").unwrap();
        let result = dirent.dirname().unwrap();
        assert_eq!(result.as_path(), std::path::Path::new("/home/user"));

        let dirent = Dirent::new("/home/user/project/").unwrap();
        let result = dirent.dirname().unwrap();
        assert_eq!(result.as_path(), std::path::Path::new("/home/user"));

        let dirent = Dirent::new("project").unwrap();
        let result = dirent.dirname().unwrap();
        assert_eq!(result.as_path(), std::path::Path::new(""));
    }

    #[test]
    fn test_split() {
        let dirent = Dirent::new("/home/user/project").unwrap();
        let (dirname, basename) = dirent.split().unwrap();
        assert_eq!(dirname.as_path(), std::path::Path::new("/home/user"));
        assert_eq!(basename.as_path(), std::path::Path::new("project"));

        let dirent = Dirent::new("project").unwrap();
        let (dirname, basename) = dirent.split().unwrap();
        assert_eq!(dirname.as_path(), std::path::Path::new(""));
        assert_eq!(basename.as_path(), std::path::Path::new("project"));
    }

    #[test]
    fn test_is_absolute() {
        let dirent = Dirent::new("/home/user").unwrap();
        assert!(dirent.is_absolute());

        let dirent = Dirent::new("home/user").unwrap();
        assert!(!dirent.is_absolute());

        let dirent = Dirent::new("./home/user").unwrap();
        assert!(!dirent.is_absolute());

        let dirent = Dirent::new("../home/user").unwrap();
        assert!(!dirent.is_absolute());
    }

    #[test]
    fn test_is_root() {
        let dirent = Dirent::new("/").unwrap();
        assert!(dirent.is_root());

        let dirent = Dirent::new("/home").unwrap();
        assert!(!dirent.is_root());

        let dirent = Dirent::new("home").unwrap();
        assert!(!dirent.is_root());

        let dirent = Dirent::new("").unwrap();
        assert!(!dirent.is_root());
    }

    #[test]
    fn test_is_canonical() {
        let dirent = Dirent::new("/home/user").unwrap();
        assert!(dirent.is_canonical());

        // These paths will be canonicalized when creating Dirent, so they should all be canonical
        let dirent = Dirent::new("/home/user/").unwrap();
        assert!(dirent.is_canonical());

        let dirent = Dirent::new("/home//user").unwrap();
        assert!(dirent.is_canonical());

        let dirent = Dirent::new("/home/./user").unwrap();
        assert!(dirent.is_canonical());
    }

    #[test]
    fn test_get_longest_ancestor() {
        let dirent1 = Dirent::new("/home/user/project1").unwrap();
        let dirent2 = Dirent::new("/home/user/project2").unwrap();
        let result = get_longest_ancestor(&dirent1, &dirent2).unwrap();
        assert_eq!(result.as_path(), std::path::Path::new("/home/user"));

        let dirent1 = Dirent::new("/home/user").unwrap();
        let dirent2 = Dirent::new("/var/log").unwrap();
        let result = get_longest_ancestor(&dirent1, &dirent2).unwrap();
        assert_eq!(result.as_path(), std::path::Path::new("/"));

        let dirent1 = Dirent::new("project1").unwrap();
        let dirent2 = Dirent::new("project2").unwrap();
        let result = get_longest_ancestor(&dirent1, &dirent2).unwrap();
        assert_eq!(result.as_path(), std::path::Path::new(""));
    }

    #[test]
    fn test_is_child() {
        let parent = Dirent::new("/home/user").unwrap();
        let child = Dirent::new("/home/user/project").unwrap();
        let result = is_child(&parent, &child).unwrap();
        assert_eq!(
            result.as_ref().map(|d| d.as_path()),
            Some(std::path::Path::new("project"))
        );

        let parent = Dirent::new("/home/user").unwrap();
        let child = Dirent::new("/var/log").unwrap();
        let result = is_child(&parent, &child).unwrap();
        assert_eq!(result, None);

        let parent = Dirent::new("/home/user").unwrap();
        let child = Dirent::new("/home/user").unwrap();
        let result = is_child(&parent, &child).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_is_ancestor() {
        let ancestor = Dirent::new("/home").unwrap();
        let path = Dirent::new("/home/user/project").unwrap();
        assert!(is_ancestor(&ancestor, &path).unwrap());

        let ancestor = Dirent::new("/home/user").unwrap();
        let path = Dirent::new("/home/user/project").unwrap();
        assert!(is_ancestor(&ancestor, &path).unwrap());

        let ancestor = Dirent::new("/var").unwrap();
        let path = Dirent::new("/home/user/project").unwrap();
        assert!(!is_ancestor(&ancestor, &path).unwrap());

        let ancestor = Dirent::new("/home/user/project").unwrap();
        let path = Dirent::new("/home/user").unwrap();
        assert!(!is_ancestor(&ancestor, &path).unwrap());
    }

    #[test]
    fn test_skip_ancestor() {
        let ancestor = Dirent::new("/home/user").unwrap();
        let path = Dirent::new("/home/user/project/file.txt").unwrap();
        let result = skip_ancestor(&ancestor, &path).unwrap();
        assert_eq!(
            result.as_ref().map(|d| d.as_path()),
            Some(std::path::Path::new("project/file.txt"))
        );

        let ancestor = Dirent::new("/var/log").unwrap();
        let path = Dirent::new("/home/user/project").unwrap();
        let result = skip_ancestor(&ancestor, &path).unwrap();
        assert_eq!(result, None);

        let ancestor = Dirent::new("/home/user").unwrap();
        let path = Dirent::new("/home/user").unwrap();
        let result = skip_ancestor(&ancestor, &path).unwrap();
        assert_eq!(
            result.as_ref().map(|d| d.as_path()),
            Some(std::path::Path::new(""))
        );
    }

    #[test]
    fn test_canonicalize_dirent() {
        // SVN canonicalization removes trailing slashes and double slashes
        let result = canonicalize_dirent(std::path::Path::new("/home/user//project/")).unwrap();
        assert_eq!(result, std::path::PathBuf::from("/home/user/project"));

        // SVN canonicalization handles empty paths
        let result = canonicalize_dirent(std::path::Path::new("")).unwrap();
        assert_eq!(result, std::path::PathBuf::from(""));
    }

    #[test]
    fn test_dirent_new_and_methods() {
        // Test with a path that needs canonicalization (double slashes)
        let dirent = Dirent::new("/home/user//project/").unwrap();
        assert_eq!(dirent.as_path(), std::path::Path::new("/home/user/project"));
        assert_eq!(dirent.as_str(), "/home/user/project");
        assert_eq!(dirent.to_string(), "/home/user/project");
    }

    #[test]
    fn test_as_canonical_dirent_trait() {
        let path_str = "/home/user/project";
        let canonical = path_str.as_canonical_dirent().unwrap();
        assert_eq!(
            canonical.as_path(),
            std::path::Path::new("/home/user/project")
        );

        let path_buf = std::path::PathBuf::from("/home/user/project");
        let canonical = path_buf.as_canonical_dirent().unwrap();
        assert_eq!(
            canonical.as_path(),
            std::path::Path::new("/home/user/project")
        );

        let path_ref = std::path::Path::new("/home/user/project");
        let canonical = path_ref.as_canonical_dirent().unwrap();
        assert_eq!(
            canonical.as_path(),
            std::path::Path::new("/home/user/project")
        );
    }

    #[test]
    fn test_crash_resistance_problematic_inputs() {
        // Test null bytes - should be rejected, not crash
        let result = Dirent::new("path\0with\0nulls");
        assert!(result.is_err(), "Should reject null bytes");

        // Test control characters - should be rejected, not crash
        let result = Dirent::new("path\x7fwith\rcontrol\nchars");
        assert!(result.is_err(), "Should reject control characters");

        // Test extremely long path - should be rejected, not crash
        let long_path = "a".repeat(5000);
        let result = Dirent::new(&long_path);
        assert!(result.is_err(), "Should reject overly long paths");

        // Test empty path - should work
        let dirent = Dirent::new("").unwrap();
        assert_eq!(dirent.as_str(), "");

        // Test single character paths
        let dirent = Dirent::new("a").unwrap();
        assert_eq!(dirent.as_str(), "a");
    }

    #[test]
    fn test_relative_absolute_mixed_paths() {
        // Absolute paths
        let abs_dirent = Dirent::new("/home/user/project").unwrap();
        assert!(abs_dirent.is_absolute());
        assert!(!abs_dirent.is_root());

        // Relative paths
        let rel_dirent = Dirent::new("project/subdir").unwrap();
        assert!(!rel_dirent.is_absolute());
        assert!(!rel_dirent.is_root());

        // Root path
        let root_dirent = Dirent::new("/").unwrap();
        assert!(root_dirent.is_absolute());
        assert!(root_dirent.is_root());

        // Test operations mixing relative and absolute
        let abs_base = Dirent::new("/home/user").unwrap();
        let joined = abs_base.join("project").unwrap();
        assert!(joined.is_absolute());
        assert_eq!(joined.as_str(), "/home/user/project");

        let rel_base = Dirent::new("home/user").unwrap();
        let joined = rel_base.join("project").unwrap();
        assert!(!joined.is_absolute());
        assert_eq!(joined.as_str(), "home/user/project");
    }

    #[test]
    fn test_get_absolute() {
        // Test with relative path
        let rel_dirent = Dirent::new("project/subdir").unwrap();
        assert!(!rel_dirent.is_absolute());

        let abs_dirent = rel_dirent.get_absolute().unwrap();
        assert!(abs_dirent.is_absolute());

        // Should contain current working directory
        assert!(abs_dirent.as_str().contains("project/subdir"));

        // Test with already absolute path
        let already_abs = Dirent::new("/home/user/project").unwrap();
        assert!(already_abs.is_absolute());

        let still_abs = already_abs.get_absolute().unwrap();
        assert!(still_abs.is_absolute());
        // Should remain the same
        assert_eq!(already_abs.as_str(), still_abs.as_str());
    }

    #[test]
    fn test_dirent_path_conversions() {
        let original_path = std::path::Path::new("/home/user//project/");

        // Path -> Dirent
        let dirent = Dirent::new(original_path).unwrap();
        assert_eq!(dirent.as_str(), "/home/user/project"); // Canonicalized

        // Dirent -> &Path
        let path_ref: &std::path::Path = dirent.as_path();
        assert_eq!(path_ref, std::path::Path::new("/home/user/project"));

        // Dirent -> PathBuf
        let path_buf: std::path::PathBuf = dirent.as_path().to_path_buf();
        assert_eq!(path_buf, std::path::PathBuf::from("/home/user/project"));

        // Test AsRef<Path> trait
        let path_ref: &std::path::Path = dirent.as_ref();
        assert_eq!(path_ref, std::path::Path::new("/home/user/project"));

        // Test that we can use Dirent anywhere Path is expected
        fn takes_path(p: &std::path::Path) -> String {
            p.to_string_lossy().to_string()
        }
        let result = takes_path(dirent.as_ref());
        assert_eq!(result, "/home/user/project");

        // Test Display trait
        let displayed = format!("{}", dirent);
        assert_eq!(displayed, "/home/user/project");
    }

    #[test]
    fn test_edge_cases_all_functions() {
        // Test with various path types
        let test_paths = vec![
            "/",          // Root
            "",           // Empty
            "single",     // Single component
            "/single",    // Absolute single
            "a/b/c/d/e",  // Deep relative
            "/a/b/c/d/e", // Deep absolute
        ];

        for path in test_paths {
            let dirent = Dirent::new(path).unwrap();

            // All these should not crash, even if they return errors
            let _ = dirent.basename();
            let _ = dirent.dirname();
            let _ = dirent.split();
            let _ = dirent.join("test");

            // These should never crash since dirent is validated
            let _ = dirent.is_absolute();
            let _ = dirent.is_root();
            let _ = dirent.is_canonical();
        }
    }

    #[test]
    fn test_function_pairs_consistency() {
        let test_cases = vec!["/home/user/project", "relative/path", "/", "", "single"];

        for path in test_cases {
            let dirent = Dirent::new(path).unwrap();

            // Test that basename + dirname operations are consistent with split
            if let (Ok(basename), Ok(dirname)) = (dirent.basename(), dirent.dirname()) {
                if let Ok((split_dirname, split_basename)) = dirent.split() {
                    assert_eq!(
                        basename.as_str(),
                        split_basename.as_str(),
                        "basename() and split() basename should match for path: {}",
                        path
                    );
                    assert_eq!(
                        dirname.as_str(),
                        split_dirname.as_str(),
                        "dirname() and split() dirname should match for path: {}",
                        path
                    );
                }
            }

            // Test that join operations are safe
            let joined = dirent.join("test").unwrap();
            assert!(
                joined.as_str().contains("test"),
                "Join should include component"
            );
        }
    }
}
