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

/// Canonicalize a directory path using SVN's canonicalization rules
pub fn canonicalize_dirent(path: &std::path::Path) -> Result<std::path::PathBuf, crate::Error> {
    with_tmp_pool(|pool| unsafe {
        let path_str = path.to_str().ok_or_else(|| crate::Error::from_str("Invalid path"))?;
        let path_cstr = std::ffi::CString::new(path_str)?;
        let canonical = subversion_sys::svn_dirent_canonicalize(path_cstr.as_ptr(), pool.as_mut_ptr());
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

/// Join two directory path components using SVN's rules
pub fn join<P1: AsRef<std::path::Path>, P2: AsRef<std::path::Path>>(
    base: P1, 
    component: P2
) -> Result<std::path::PathBuf, crate::Error> {
    with_tmp_pool(|pool| unsafe {
        let base_str = base.as_ref().to_str().ok_or_else(|| crate::Error::from_str("Invalid base path"))?;
        let component_str = component.as_ref().to_str().ok_or_else(|| crate::Error::from_str("Invalid component path"))?;
        
        // Canonicalize the base path first
        let base_cstr = std::ffi::CString::new(base_str)?;
        let canonical_base = subversion_sys::svn_dirent_canonicalize(base_cstr.as_ptr(), pool.as_mut_ptr());
        
        let component_cstr = std::ffi::CString::new(component_str)?;
        
        let joined = subversion_sys::svn_dirent_join(
            canonical_base,
            component_cstr.as_ptr(),
            pool.as_mut_ptr()
        );
        
        let joined_cstr = std::ffi::CStr::from_ptr(joined);
        let joined_str = joined_cstr.to_str()?;
        Ok(std::path::PathBuf::from(joined_str))
    })
}

/// Get the basename (final component) of a directory path
pub fn basename<P: AsRef<std::path::Path>>(path: P) -> Result<std::path::PathBuf, crate::Error> {
    with_tmp_pool(|pool| unsafe {
        let path_str = path.as_ref().to_str().ok_or_else(|| crate::Error::from_str("Invalid path"))?;
        let path_cstr = std::ffi::CString::new(path_str)?;
        
        // Canonicalize the path first  
        let canonical_path = subversion_sys::svn_dirent_canonicalize(path_cstr.as_ptr(), pool.as_mut_ptr());
        
        let basename = subversion_sys::svn_dirent_basename(canonical_path, pool.as_mut_ptr());
        let basename_cstr = std::ffi::CStr::from_ptr(basename);
        let basename_str = basename_cstr.to_str()?;
        Ok(std::path::PathBuf::from(basename_str))
    })
}

/// Get the dirname (directory component) of a directory path  
pub fn dirname<P: AsRef<std::path::Path>>(path: P) -> Result<std::path::PathBuf, crate::Error> {
    with_tmp_pool(|pool| unsafe {
        let path_str = path.as_ref().to_str().ok_or_else(|| crate::Error::from_str("Invalid path"))?;
        let path_cstr = std::ffi::CString::new(path_str)?;
        
        // Canonicalize the path first
        let canonical_path = subversion_sys::svn_dirent_canonicalize(path_cstr.as_ptr(), pool.as_mut_ptr());
        
        let dirname = subversion_sys::svn_dirent_dirname(canonical_path, pool.as_mut_ptr());
        let dirname_cstr = std::ffi::CStr::from_ptr(dirname);
        let dirname_str = dirname_cstr.to_str()?;
        Ok(std::path::PathBuf::from(dirname_str))
    })
}

/// Split a directory path into dirname and basename components
pub fn split<P: AsRef<std::path::Path>>(path: P) -> Result<(std::path::PathBuf, std::path::PathBuf), crate::Error> {
    with_tmp_pool(|pool| unsafe {
        let path_str = path.as_ref().to_str().ok_or_else(|| crate::Error::from_str("Invalid path"))?;
        let path_cstr = std::ffi::CString::new(path_str)?;
        
        // Canonicalize the path first
        let canonical_path = subversion_sys::svn_dirent_canonicalize(path_cstr.as_ptr(), pool.as_mut_ptr());
        
        let mut dirname_ptr: *const std::ffi::c_char = std::ptr::null();
        let mut basename_ptr: *const std::ffi::c_char = std::ptr::null();
        
        subversion_sys::svn_dirent_split(
            &mut dirname_ptr,
            &mut basename_ptr,
            canonical_path,
            pool.as_mut_ptr()
        );
        
        let dirname_str = std::ffi::CStr::from_ptr(dirname_ptr).to_str()?;
        let basename_str = std::ffi::CStr::from_ptr(basename_ptr).to_str()?;
        
        Ok((std::path::PathBuf::from(dirname_str), std::path::PathBuf::from(basename_str)))
    })
}

/// Check if a directory path is absolute
pub fn is_absolute<P: AsRef<std::path::Path>>(path: P) -> Result<bool, crate::Error> {
    let path_str = path.as_ref().to_str().ok_or_else(|| crate::Error::from_str("Invalid path"))?;
    let path_cstr = std::ffi::CString::new(path_str)?;
    
    unsafe {
        let result = subversion_sys::svn_dirent_is_absolute(path_cstr.as_ptr());
        Ok(result != 0)
    }
}

/// Check if a directory path is a root path
pub fn is_root<P: AsRef<std::path::Path>>(path: P) -> Result<bool, crate::Error> {
    let path_str = path.as_ref().to_str().ok_or_else(|| crate::Error::from_str("Invalid path"))?;
    let path_cstr = std::ffi::CString::new(path_str)?;
    
    unsafe {
        let result = subversion_sys::svn_dirent_is_root(path_cstr.as_ptr(), path_str.len());
        Ok(result != 0)
    }
}

/// Check if a directory path is canonical
pub fn is_canonical<P: AsRef<std::path::Path>>(path: P) -> Result<bool, crate::Error> {
    with_tmp_pool(|pool| unsafe {
        let path_str = path.as_ref().to_str().ok_or_else(|| crate::Error::from_str("Invalid path"))?;
        let path_cstr = std::ffi::CString::new(path_str)?;
        
        let result = subversion_sys::svn_dirent_is_canonical(path_cstr.as_ptr(), pool.as_mut_ptr());
        Ok(result != 0)
    })
}

/// Get the longest common ancestor of two directory paths
pub fn get_longest_ancestor<P1: AsRef<std::path::Path>, P2: AsRef<std::path::Path>>(
    path1: P1,
    path2: P2
) -> Result<std::path::PathBuf, crate::Error> {
    with_tmp_pool(|pool| unsafe {
        let path1_str = path1.as_ref().to_str().ok_or_else(|| crate::Error::from_str("Invalid path1"))?;
        let path2_str = path2.as_ref().to_str().ok_or_else(|| crate::Error::from_str("Invalid path2"))?;
        
        let path1_cstr = std::ffi::CString::new(path1_str)?;
        let path2_cstr = std::ffi::CString::new(path2_str)?;
        
        let ancestor = subversion_sys::svn_dirent_get_longest_ancestor(
            path1_cstr.as_ptr(),
            path2_cstr.as_ptr(),
            pool.as_mut_ptr()
        );
        
        let ancestor_cstr = std::ffi::CStr::from_ptr(ancestor);
        let ancestor_str = ancestor_cstr.to_str()?;
        Ok(std::path::PathBuf::from(ancestor_str))
    })
}

/// Check if the child path is a child of the parent path
pub fn is_child<P1: AsRef<std::path::Path>, P2: AsRef<std::path::Path>>(
    parent: P1,
    child: P2
) -> Result<Option<std::path::PathBuf>, crate::Error> {
    with_tmp_pool(|pool| unsafe {
        let parent_str = parent.as_ref().to_str().ok_or_else(|| crate::Error::from_str("Invalid parent path"))?;
        let child_str = child.as_ref().to_str().ok_or_else(|| crate::Error::from_str("Invalid child path"))?;
        
        let parent_cstr = std::ffi::CString::new(parent_str)?;
        let child_cstr = std::ffi::CString::new(child_str)?;
        
        let result = subversion_sys::svn_dirent_is_child(
            parent_cstr.as_ptr(),
            child_cstr.as_ptr(),
            pool.as_mut_ptr()
        );
        
        if result.is_null() {
            Ok(None)
        } else {
            let result_cstr = std::ffi::CStr::from_ptr(result);
            let result_str = result_cstr.to_str()?;
            Ok(Some(std::path::PathBuf::from(result_str)))
        }
    })
}

/// Check if the child path is an ancestor of the parent path
pub fn is_ancestor<P1: AsRef<std::path::Path>, P2: AsRef<std::path::Path>>(
    ancestor: P1,
    path: P2
) -> Result<bool, crate::Error> {
    let ancestor_str = ancestor.as_ref().to_str().ok_or_else(|| crate::Error::from_str("Invalid ancestor path"))?;
    let path_str = path.as_ref().to_str().ok_or_else(|| crate::Error::from_str("Invalid path"))?;
    
    let ancestor_cstr = std::ffi::CString::new(ancestor_str)?;
    let path_cstr = std::ffi::CString::new(path_str)?;
    
    unsafe {
        let result = subversion_sys::svn_dirent_is_ancestor(
            ancestor_cstr.as_ptr(),
            path_cstr.as_ptr()
        );
        Ok(result != 0)
    }
}

/// Skip the ancestor part of a path, returning the remaining child portion
pub fn skip_ancestor<P1: AsRef<std::path::Path>, P2: AsRef<std::path::Path>>(
    ancestor: P1,
    path: P2
) -> Result<Option<std::path::PathBuf>, crate::Error> {
    let ancestor_str = ancestor.as_ref().to_str().ok_or_else(|| crate::Error::from_str("Invalid ancestor path"))?;
    let path_str = path.as_ref().to_str().ok_or_else(|| crate::Error::from_str("Invalid path"))?;
    
    let ancestor_cstr = std::ffi::CString::new(ancestor_str)?;
    let path_cstr = std::ffi::CString::new(path_str)?;
    
    unsafe {
        let result = subversion_sys::svn_dirent_skip_ancestor(
            ancestor_cstr.as_ptr(),
            path_cstr.as_ptr()
        );
        
        if result.is_null() {
            Ok(None)
        } else {
            let result_cstr = std::ffi::CStr::from_ptr(result);
            let result_str = result_cstr.to_str()?;
            Ok(Some(std::path::PathBuf::from(result_str)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_join() {
        let result = join("/home/user", "project").unwrap();
        assert_eq!(result, std::path::PathBuf::from("/home/user/project"));
        
        let result = join("/home/user/", "project").unwrap();
        assert_eq!(result, std::path::PathBuf::from("/home/user/project"));
        
        let result = join("", "project").unwrap();
        assert_eq!(result, std::path::PathBuf::from("project"));
    }
    
    #[test]
    fn test_basename() {
        let result = basename("/home/user/project").unwrap();
        assert_eq!(result, std::path::PathBuf::from("project"));
        
        let result = basename("/home/user/project/").unwrap();
        assert_eq!(result, std::path::PathBuf::from("project"));
        
        let result = basename("project").unwrap();
        assert_eq!(result, std::path::PathBuf::from("project"));
    }
    
    #[test]
    fn test_dirname() {
        let result = dirname("/home/user/project").unwrap();
        assert_eq!(result, std::path::PathBuf::from("/home/user"));
        
        let result = dirname("/home/user/project/").unwrap();
        assert_eq!(result, std::path::PathBuf::from("/home/user"));
        
        let result = dirname("project").unwrap();
        assert_eq!(result, std::path::PathBuf::from(""));
    }
    
    #[test]
    fn test_split() {
        let (dirname, basename) = split("/home/user/project").unwrap();
        assert_eq!(dirname, std::path::PathBuf::from("/home/user"));
        assert_eq!(basename, std::path::PathBuf::from("project"));
        
        let (dirname, basename) = split("project").unwrap();
        assert_eq!(dirname, std::path::PathBuf::from(""));
        assert_eq!(basename, std::path::PathBuf::from("project"));
    }
    
    #[test]
    fn test_is_absolute() {
        assert!(is_absolute("/home/user").unwrap());
        assert!(!is_absolute("home/user").unwrap());
        assert!(!is_absolute("./home/user").unwrap());
        assert!(!is_absolute("../home/user").unwrap());
    }
    
    #[test]
    fn test_is_root() {
        assert!(is_root("/").unwrap());
        assert!(!is_root("/home").unwrap());
        assert!(!is_root("home").unwrap());
        assert!(!is_root("").unwrap());
    }
    
    #[test]
    fn test_is_canonical() {
        assert!(is_canonical("/home/user").unwrap());
        assert!(!is_canonical("/home/user/").unwrap());
        assert!(!is_canonical("/home//user").unwrap());
        assert!(!is_canonical("/home/./user").unwrap());
    }
    
    #[test]
    fn test_get_longest_ancestor() {
        let result = get_longest_ancestor("/home/user/project1", "/home/user/project2").unwrap();
        assert_eq!(result, std::path::PathBuf::from("/home/user"));
        
        let result = get_longest_ancestor("/home/user", "/var/log").unwrap();
        assert_eq!(result, std::path::PathBuf::from("/"));
        
        let result = get_longest_ancestor("project1", "project2").unwrap();
        assert_eq!(result, std::path::PathBuf::from(""));
    }
    
    #[test]
    fn test_is_child() {
        let result = is_child("/home/user", "/home/user/project").unwrap();
        assert_eq!(result, Some(std::path::PathBuf::from("project")));
        
        let result = is_child("/home/user", "/var/log").unwrap();
        assert_eq!(result, None);
        
        let result = is_child("/home/user", "/home/user").unwrap();
        assert_eq!(result, None);
    }
    
    #[test]
    fn test_is_ancestor() {
        assert!(is_ancestor("/home", "/home/user/project").unwrap());
        assert!(is_ancestor("/home/user", "/home/user/project").unwrap());
        assert!(!is_ancestor("/var", "/home/user/project").unwrap());
        assert!(!is_ancestor("/home/user/project", "/home/user").unwrap());
    }
    
    #[test]
    fn test_skip_ancestor() {
        let result = skip_ancestor("/home/user", "/home/user/project/file.txt").unwrap();
        assert_eq!(result, Some(std::path::PathBuf::from("project/file.txt")));
        
        let result = skip_ancestor("/var/log", "/home/user/project").unwrap();
        assert_eq!(result, None);
        
        let result = skip_ancestor("/home/user", "/home/user").unwrap();
        assert_eq!(result, Some(std::path::PathBuf::from("")));
    }
    
    #[test]
    fn test_canonicalize_dirent() {
        // SVN canonicalization removes trailing slashes and double slashes
        let result = canonicalize_dirent(&std::path::Path::new("/home/user//project/")).unwrap();
        assert_eq!(result, std::path::PathBuf::from("/home/user/project"));
        
        // SVN canonicalization handles empty paths
        let result = canonicalize_dirent(&std::path::Path::new("")).unwrap();
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
        assert_eq!(canonical.as_path(), std::path::Path::new("/home/user/project"));
        
        let path_buf = std::path::PathBuf::from("/home/user/project");
        let canonical = path_buf.as_canonical_dirent().unwrap();
        assert_eq!(canonical.as_path(), std::path::Path::new("/home/user/project"));
        
        let path_ref = std::path::Path::new("/home/user/project");
        let canonical = path_ref.as_canonical_dirent().unwrap();
        assert_eq!(canonical.as_path(), std::path::Path::new("/home/user/project"));
    }
}