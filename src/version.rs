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
