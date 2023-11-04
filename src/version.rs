use crate::generated::svn_version_t;
pub struct Version(pub(crate) *const svn_version_t);

impl Version {
    fn equal(&self, other: &Version) -> bool {
        !matches!(
            unsafe { crate::generated::svn_ver_equal(self.0, other.0) },
            0
        )
    }

    pub fn compatible(&self, other: &Version) -> bool {
        !matches!(
            unsafe { crate::generated::svn_ver_compatible(self.0, other.0) },
            0
        )
    }

    pub fn major(&self) -> i32 {
        unsafe { self.0.as_ref().unwrap().major }
    }

    pub fn minor(&self) -> i32 {
        unsafe { self.0.as_ref().unwrap().minor }
    }

    pub fn patch(&self) -> i32 {
        unsafe { self.0.as_ref().unwrap().patch }
    }

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
