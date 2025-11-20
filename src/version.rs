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

        // These might return None depending on build configuration
        let _build_date = ext.build_date();
        let _build_time = ext.build_time();
        let _build_host = ext.build_host();
        let _copyright = ext.copyright();
        let _runtime_host = ext.runtime_host();
        let _runtime_osname = ext.runtime_osname();

        // Just verify they don't crash
    }

    #[test]
    fn test_version_extended_verbose() {
        let ext = VersionExtended::get(true);

        // Verbose mode might provide more information
        let _copyright = ext.copyright();

        // At least copyright should usually be available
        // but we don't assert since it depends on the build
    }
}
