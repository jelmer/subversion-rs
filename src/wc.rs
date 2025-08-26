use crate::generated::{svn_wc_context_t, svn_wc_version};
use crate::Error;
pub fn version() -> crate::Version {
    unsafe { crate::Version(svn_wc_version()) }
}

pub struct Context(apr::pool::PooledPtr<svn_wc_context_t>);

impl Context {
    pub fn new() -> Result<Self, crate::Error> {
        let pool = apr::pool::Pool::new();
        let mut ctx = std::ptr::null_mut();
        let err = unsafe {
            crate::generated::svn_wc_context_create(
                &mut ctx,
                std::ptr::null_mut(),
                pool.as_mut_ptr(),
                apr::pool::Pool::new().as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(Self(unsafe {
            apr::pool::PooledPtr::in_pool(std::rc::Rc::new(pool), ctx)
        }))
    }

    pub fn check_wc(&mut self, path: &str) -> Result<i32, crate::Error> {
        let path = std::ffi::CString::new(path).unwrap();
        let mut wc_format = 0;
        let err = unsafe {
            crate::generated::svn_wc_check_wc2(
                &mut wc_format,
                self.0.as_mut_ptr(),
                path.as_ptr(),
                apr::pool::Pool::new().as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(wc_format)
    }

    pub fn text_modified(&mut self, path: &str) -> Result<bool, crate::Error> {
        let path = std::ffi::CString::new(path).unwrap();
        let mut modified = 0;
        let err = unsafe {
            crate::generated::svn_wc_text_modified_p2(
                &mut modified,
                self.0.as_mut_ptr(),
                path.as_ptr(),
                0,
                apr::pool::Pool::new().as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(modified != 0)
    }

    pub fn props_modified(&mut self, path: &str) -> Result<bool, crate::Error> {
        let path = std::ffi::CString::new(path).unwrap();
        let mut modified = 0;
        let err = unsafe {
            crate::generated::svn_wc_props_modified_p2(
                &mut modified,
                self.0.as_mut_ptr(),
                path.as_ptr(),
                apr::pool::Pool::new().as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(modified != 0)
    }

    pub fn conflicted(&mut self, path: &str) -> Result<(bool, bool, bool), crate::Error> {
        let path = std::ffi::CString::new(path).unwrap();
        let mut text_conflicted = 0;
        let mut prop_conflicted = 0;
        let mut tree_conflicted = 0;
        let err = unsafe {
            crate::generated::svn_wc_conflicted_p3(
                &mut text_conflicted,
                &mut prop_conflicted,
                &mut tree_conflicted,
                self.0.as_mut_ptr(),
                path.as_ptr(),
                apr::pool::Pool::new().as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok((
            text_conflicted != 0,
            prop_conflicted != 0,
            tree_conflicted != 0,
        ))
    }

    pub fn ensure_adm(
        &mut self,
        local_abspath: &str,
        url: &str,
        repos_root_url: &str,
        repos_uuid: &str,
        revision: crate::Revnum,
        depth: crate::Depth,
    ) -> Result<(), crate::Error> {
        let local_abspath = std::ffi::CString::new(local_abspath).unwrap();
        let url = std::ffi::CString::new(url).unwrap();
        let repos_root_url = std::ffi::CString::new(repos_root_url).unwrap();
        let repos_uuid = std::ffi::CString::new(repos_uuid).unwrap();
        let err = unsafe {
            crate::generated::svn_wc_ensure_adm4(
                self.0.as_mut_ptr(),
                local_abspath.as_ptr(),
                url.as_ptr(),
                repos_root_url.as_ptr(),
                repos_uuid.as_ptr(),
                revision.0,
                depth.into(),
                apr::pool::Pool::new().as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(())
    }

    pub fn locked(&mut self, path: &str) -> Result<(bool, bool), crate::Error> {
        let path = std::ffi::CString::new(path).unwrap();
        let mut locked = 0;
        let mut locked_here = 0;
        let scratch_pool = apr::pool::Pool::new();
        let err = unsafe {
            crate::generated::svn_wc_locked2(
                &mut locked_here,
                &mut locked,
                self.0.as_mut_ptr(),
                path.as_ptr(),
                scratch_pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok((locked != 0, locked_here != 0))
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe {
            crate::generated::svn_wc_context_destroy(self.0.as_mut_ptr());
        }
    }
}

pub fn set_adm_dir(name: &str) -> Result<(), crate::Error> {
    let name = std::ffi::CString::new(name).unwrap();
    let err = unsafe {
        crate::generated::svn_wc_set_adm_dir(name.as_ptr(), apr::pool::Pool::new().as_mut_ptr())
    };
    Error::from_raw(err)?;
    Ok(())
}

pub fn get_adm_dir() -> String {
    let pool = apr::pool::Pool::new();
    let name = unsafe { crate::generated::svn_wc_get_adm_dir(pool.as_mut_ptr()) };
    unsafe { std::ffi::CStr::from_ptr(name) }
        .to_string_lossy()
        .into_owned()
}
