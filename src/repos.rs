use crate::generated::{svn_repos_create, svn_repos_find_root_path, svn_repos_t};
use apr::pool::PooledPtr;

pub fn find_root_path(path: &std::path::Path) -> Option<std::path::PathBuf> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let mut pool = apr::Pool::new();
    let ret = unsafe { svn_repos_find_root_path(path.as_ptr(), pool.as_mut_ptr()) };
    if ret.is_null() {
        let root_path = unsafe { std::ffi::CStr::from_ptr(ret) };
        Some(std::path::PathBuf::from(root_path.to_str().unwrap()))
    } else {
        None
    }
}

pub struct Repos<'pool>(PooledPtr<'pool, svn_repos_t>);

impl<'pool> Repos<'pool> {
    pub fn create(path: &std::path::Path) -> Result<Repos<'pool>, crate::Error> {
        // TODO: Support config, fs_config
        let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
        let config = std::ptr::null_mut();
        let fs_config = std::ptr::null_mut();
        Ok(Self(PooledPtr::initialize(|pool| unsafe {
            let mut repos: *mut svn_repos_t = std::ptr::null_mut();
            let ret = svn_repos_create(
                &mut repos,
                path.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                config,
                fs_config,
                pool.as_mut_ptr(),
            );
            if ret.is_null() {
                Ok(repos)
            } else {
                Err(crate::Error(ret))
            }
        })?))
    }

    pub fn open(path: &std::path::Path) -> Result<Repos<'pool>, crate::Error> {
        let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
        Ok(Self(PooledPtr::initialize(|pool| {
            let mut repos: *mut svn_repos_t = std::ptr::null_mut();
            let ret = unsafe {
                crate::generated::svn_repos_open(&mut repos, path.as_ptr(), pool.as_mut_ptr())
            };
            if ret.is_null() {
                Ok(repos)
            } else {
                Err(crate::Error(ret))
            }
        })?))
    }

    pub fn capabilities(&mut self) -> Result<std::collections::HashSet<String>, crate::Error> {
        let mut capabilities =
            apr::hash::Hash::<&str, &str>::from_raw(PooledPtr::initialize(|pool| {
                let mut capabilities: *mut apr::hash::apr_hash_t = std::ptr::null_mut();
                let mut scratch_pool = apr::Pool::new();
                let ret = unsafe {
                    crate::generated::svn_repos_capabilities(
                        &mut capabilities,
                        self.0.as_mut_ptr(),
                        pool.as_mut_ptr(),
                        scratch_pool.as_mut_ptr(),
                    )
                };
                if ret.is_null() {
                    Ok(capabilities)
                } else {
                    Err(crate::Error(ret))
                }
            })?);

        Ok(capabilities
            .keys()
            .map(|k| String::from_utf8_lossy(k).to_string())
            .collect::<std::collections::HashSet<String>>())
    }

    pub fn has_capability(&mut self, capability: &str) -> Result<bool, crate::Error> {
        let capability = std::ffi::CString::new(capability).unwrap();
        let mut pool = apr::Pool::new();
        unsafe {
            let mut has: crate::generated::svn_boolean_t = 0;
            let ret = crate::generated::svn_repos_has_capability(
                self.0.as_mut_ptr(),
                &mut has,
                capability.as_ptr(),
                pool.as_mut_ptr(),
            );
            if ret.is_null() {
                Ok(has != 0)
            } else {
                Err(crate::Error(ret))
            }
        }
    }
}

pub struct Notify<'pool>(PooledPtr<'pool, crate::generated::svn_repos_notify_t>);

extern "C" fn wrap_notify_func(
    baton: *mut std::ffi::c_void,
    notify: *const crate::generated::svn_repos_notify_t,
    pool: *mut apr::apr_pool_t,
) {
    let pool = apr::Pool::from_raw(pool);
    let notify = unsafe { &*notify };
    let baton = unsafe { &mut *(baton as *mut Box<dyn FnMut(&Notify)>) };
    unsafe {
        baton(&Notify(PooledPtr::in_pool(
            std::rc::Rc::new(pool),
            notify as *const _ as *mut _,
        )));
    }
}

pub fn upgrade(
    path: &std::path::Path,
    nonblocking: bool,
    notify_func: Option<&mut dyn FnMut(&Notify)>,
) -> Result<(), crate::Error> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let notify_func = notify_func.map(|_notify_func| wrap_notify_func as _);
    let notify_baton = Box::into_raw(Box::new(notify_func)).cast();
    let mut pool = apr::Pool::new();
    let ret = unsafe {
        crate::generated::svn_repos_upgrade2(
            path.as_ptr(),
            nonblocking as i32,
            notify_func,
            notify_baton,
            pool.as_mut_ptr(),
        )
    };
    if ret.is_null() {
        Ok(())
    } else {
        Err(crate::Error(ret))
    }
}

pub fn delete(path: &std::path::Path) -> Result<(), crate::Error> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let mut pool = apr::Pool::new();
    let ret = unsafe { crate::generated::svn_repos_delete(path.as_ptr(), pool.as_mut_ptr()) };
    if ret.is_null() {
        Ok(())
    } else {
        Err(crate::Error(ret))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_create_delete() {
        let td = tempfile::tempdir().unwrap();
        super::Repos::create(td.path()).unwrap();
        super::Repos::open(td.path()).unwrap();
        super::delete(td.path()).unwrap();
    }

    #[ignore = "needs error fixes"]
    #[test]
    fn test_capabilities() {
        let td = tempfile::tempdir().unwrap();
        let mut repos = super::Repos::create(td.path()).unwrap();
        assert!(repos.capabilities().unwrap().contains("mergeinfo"));
        assert!(!repos.capabilities().unwrap().contains("mergeinfo2"));
        assert!(repos.has_capability("mergeinfo").unwrap());
        assert!(!repos.has_capability("unknown").unwrap());
    }
}
