use subversion_sys::svn_version_t;
/// Represents a Subversion version.
pub struct Version(pub(crate) *const svn_version_t);

impl Version {
    fn equal(&self, other: &Version) -> bool {
        !matches!(unsafe { subversion_sys::svn_ver_equal(self.0, other.0) }, 0)
    }

    /// Checks if this version is compatible with another version.
    pub fn compatible(&self, other: &Version) -> bool {
        !matches!(
            unsafe { subversion_sys::svn_ver_compatible(self.0, other.0) },
            0
        )
    }

    /// Gets the major version number.
    pub fn major(&self) -> i32 {
        unsafe { self.0.as_ref().unwrap().major }
    }

    /// Gets the minor version number.
    pub fn minor(&self) -> i32 {
        unsafe { self.0.as_ref().unwrap().minor }
    }

    /// Gets the patch version number.
    pub fn patch(&self) -> i32 {
        unsafe { self.0.as_ref().unwrap().patch }
    }

    /// Gets the version tag string.
    pub fn tag(&self) -> &str {
        unsafe {
            let tag = self.0.as_ref().unwrap().tag;
            std::ffi::CStr::from_ptr(tag).to_str().unwrap()
        }
    }
}

impl PartialEq for Version {
    fn eq(&self, other: &Version) -> bool {
        self.equal(other)
    }
}

impl Eq for Version {}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tag = self.tag();
        if tag.is_empty() {
            write!(f, "{}.{}.{}", self.major(), self.minor(), self.patch())
        } else {
            write!(
                f,
                "{}.{}.{}-{}",
                self.major(),
                self.minor(),
                self.patch(),
                tag
            )
        }
    }
}

impl std::fmt::Debug for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Version")
            .field("major", &self.major())
            .field("minor", &self.minor())
            .field("patch", &self.patch())
            .field("tag", &self.tag())
            .finish()
    }
}

/// Extended version information
pub struct VersionExtended(
    *const subversion_sys::svn_version_extended_t,
    #[allow(dead_code)] apr::Pool<'static>,
);

impl VersionExtended {
    /// Get the extended version information for the current library
    pub fn get(verbose: bool) -> Self {
        unsafe {
            let pool = apr::Pool::new();
            let ptr = subversion_sys::svn_version_extended(verbose as i32, pool.as_mut_ptr());
            VersionExtended(ptr, pool)
        }
    }

    /// Get the date when the library was compiled
    pub fn build_date(&self) -> Option<&str> {
        unsafe {
            let date_ptr = subversion_sys::svn_version_ext_build_date(self.0);
            if date_ptr.is_null() {
                None
            } else {
                Some(std::ffi::CStr::from_ptr(date_ptr).to_str().unwrap_or(""))
            }
        }
    }

    /// Get the time when the library was compiled
    pub fn build_time(&self) -> Option<&str> {
        unsafe {
            let time_ptr = subversion_sys::svn_version_ext_build_time(self.0);
            if time_ptr.is_null() {
                None
            } else {
                Some(std::ffi::CStr::from_ptr(time_ptr).to_str().unwrap_or(""))
            }
        }
    }

    /// Get the canonical host triplet of the build system
    pub fn build_host(&self) -> Option<&str> {
        unsafe {
            let host_ptr = subversion_sys::svn_version_ext_build_host(self.0);
            if host_ptr.is_null() {
                None
            } else {
                Some(std::ffi::CStr::from_ptr(host_ptr).to_str().unwrap_or(""))
            }
        }
    }

    /// Get the copyright notice
    pub fn copyright(&self) -> Option<&str> {
        unsafe {
            let copyright_ptr = subversion_sys::svn_version_ext_copyright(self.0);
            if copyright_ptr.is_null() {
                None
            } else {
                Some(
                    std::ffi::CStr::from_ptr(copyright_ptr)
                        .to_str()
                        .unwrap_or(""),
                )
            }
        }
    }

    /// Get the canonical host triplet of the runtime system
    pub fn runtime_host(&self) -> Option<&str> {
        unsafe {
            let host_ptr = subversion_sys::svn_version_ext_runtime_host(self.0);
            if host_ptr.is_null() {
                None
            } else {
                Some(std::ffi::CStr::from_ptr(host_ptr).to_str().unwrap_or(""))
            }
        }
    }

    /// Get the commercial OS name of the runtime system
    pub fn runtime_osname(&self) -> Option<&str> {
        unsafe {
            let os_ptr = subversion_sys::svn_version_ext_runtime_osname(self.0);
            if os_ptr.is_null() {
                None
            } else {
                Some(std::ffi::CStr::from_ptr(os_ptr).to_str().unwrap_or(""))
            }
        }
    }
}

// Safety: VersionExtended is just a wrapper around a const pointer
unsafe impl Send for VersionExtended {}
unsafe impl Sync for VersionExtended {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_extended() {
        let ext = VersionExtended::get(false);

        // Verify returned values are valid when present
        if let Some(build_date) = ext.build_date() {
            assert!(!build_date.is_empty(), "Build date should not be empty");
        }
        if let Some(build_time) = ext.build_time() {
            assert!(!build_time.is_empty(), "Build time should not be empty");
        }
        if let Some(build_host) = ext.build_host() {
            assert!(!build_host.is_empty(), "Build host should not be empty");
        }
        if let Some(copyright) = ext.copyright() {
            assert!(!copyright.is_empty(), "Copyright should not be empty");
        }
        if let Some(runtime_host) = ext.runtime_host() {
            assert!(!runtime_host.is_empty(), "Runtime host should not be empty");
        }
        if let Some(runtime_osname) = ext.runtime_osname() {
            assert!(
                !runtime_osname.is_empty(),
                "Runtime OS name should not be empty"
            );
        }
    }

    #[test]
    fn test_version_extended_verbose() {
        let ext = VersionExtended::get(true);

        // In verbose mode, copyright should be available
        let copyright = ext.copyright();
        assert!(copyright.is_some(), "Verbose mode should provide copyright");
        assert!(
            !copyright.unwrap().is_empty(),
            "Copyright should not be empty"
        );
    }

    #[test]
    fn test_version_extended_methods_return_values() {
        let ext = VersionExtended::get(false);

        // Copyright should usually be available and non-empty
        if let Some(copyright) = ext.copyright() {
            assert!(
                !copyright.is_empty(),
                "Copyright should not be empty string"
            );
            assert_ne!(copyright, "xyzzy", "Copyright should not be placeholder");
        }

        // Build date/time might be None, but if Some, should not be empty
        if let Some(build_date) = ext.build_date() {
            assert!(
                !build_date.is_empty(),
                "Build date should not be empty string"
            );
            assert_ne!(build_date, "xyzzy", "Build date should not be placeholder");
        }

        if let Some(build_time) = ext.build_time() {
            assert!(
                !build_time.is_empty(),
                "Build time should not be empty string"
            );
            assert_ne!(build_time, "xyzzy", "Build time should not be placeholder");
        }

        if let Some(build_host) = ext.build_host() {
            assert!(
                !build_host.is_empty(),
                "Build host should not be empty string"
            );
            assert_ne!(build_host, "xyzzy", "Build host should not be placeholder");
        }

        if let Some(runtime_host) = ext.runtime_host() {
            assert!(
                !runtime_host.is_empty(),
                "Runtime host should not be empty string"
            );
            assert_ne!(
                runtime_host, "xyzzy",
                "Runtime host should not be placeholder"
            );
        }

        if let Some(runtime_osname) = ext.runtime_osname() {
            assert!(
                !runtime_osname.is_empty(),
                "Runtime OS should not be empty string"
            );
            assert_ne!(
                runtime_osname, "xyzzy",
                "Runtime OS should not be placeholder"
            );
        }
    }

    #[test]
    fn test_version_extended_verbose_provides_copyright() {
        let ext = VersionExtended::get(true);

        // In verbose mode, copyright should typically be available
        let copyright = ext.copyright();
        if let Some(c) = copyright {
            assert!(!c.is_empty(), "Copyright should not be empty");
            assert_ne!(c, "xyzzy", "Copyright should not be placeholder");
            // Copyright usually contains "Apache" or "Subversion"
            assert!(
                c.contains("Apache") || c.contains("Subversion") || c.contains("Copyright"),
                "Copyright should contain expected keywords, got: {}",
                c
            );
        }
    }

    #[test]
    fn test_version_major_minor_patch() {
        // Get the SVN library version
        let version = unsafe {
            let v = subversion_sys::svn_subr_version();
            Version(v)
        };

        // SVN version should be at least 1.x
        let major = version.major();
        assert!(
            major >= 1,
            "SVN major version should be >= 1, got {}",
            major
        );
        assert!(major > 0, "Major version should be positive");
        assert_ne!(major, -1, "Major version should not be -1");

        // Minor version should be non-negative
        let minor = version.minor();
        assert!(
            minor >= 0,
            "Minor version should be non-negative, got {}",
            minor
        );
        assert_ne!(minor, -1, "Minor version should not be -1");

        // Patch version should be non-negative
        let patch = version.patch();
        assert!(
            patch >= 0,
            "Patch version should be non-negative, got {}",
            patch
        );
        assert_ne!(patch, -1, "Patch version should not be -1");
    }

    #[test]
    fn test_version_tag() {
        let version = unsafe {
            let v = subversion_sys::svn_subr_version();
            Version(v)
        };

        let tag = version.tag();
        // Tag should not be the placeholder string
        assert_ne!(tag, "xyzzy", "Version tag should not be placeholder");
        // Tag is typically empty for release versions, or contains dev/alpha/beta/rc
        // Just verify it's a valid string (not panicking on conversion)
    }

    #[test]
    fn test_version_equal_same_version() {
        let v1 = unsafe {
            let v = subversion_sys::svn_subr_version();
            Version(v)
        };
        let v2 = unsafe {
            let v = subversion_sys::svn_subr_version();
            Version(v)
        };

        // Same version should be equal
        assert!(v1.equal(&v2), "Same version should be equal to itself");
        assert_eq!(v1, v2, "PartialEq should work for same version");
    }

    #[test]
    fn test_version_compatible_with_itself() {
        let v1 = unsafe {
            let v = subversion_sys::svn_subr_version();
            Version(v)
        };
        let v2 = unsafe {
            let v = subversion_sys::svn_subr_version();
            Version(v)
        };

        // A version should be compatible with itself
        assert!(
            v1.compatible(&v2),
            "Version should be compatible with itself"
        );
    }

    #[test]
    fn test_version_partialeq_reflexive() {
        let v = unsafe {
            let v = subversion_sys::svn_subr_version();
            Version(v)
        };

        // Reflexivity: v == v
        assert_eq!(v, v, "Version should equal itself (reflexivity)");
    }
}
