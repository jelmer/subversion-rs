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
        with_tmp_pool(|pool| unsafe {
            let uri_cstr = std::ffi::CString::new(self.0.as_str()).unwrap();
            subversion_sys::svn_uri_is_root(uri_cstr.as_ptr(), self.0.len()) != 0
        })
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
