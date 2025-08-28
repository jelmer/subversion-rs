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