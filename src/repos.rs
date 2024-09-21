use crate::generated::{svn_repos_create, svn_repos_find_root_path, svn_repos_t};
use crate::Error;
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

pub struct Repos(PooledPtr<svn_repos_t>);

impl Repos {
    pub fn create(path: &std::path::Path) -> Result<Repos, crate::Error> {
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
            Error::from_raw(ret)?;
            Ok::<_, Error>(repos)
        })?))
    }

    pub fn open(path: &std::path::Path) -> Result<Repos, crate::Error> {
        let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
        Ok(Self(PooledPtr::initialize(|pool| {
            let mut repos: *mut svn_repos_t = std::ptr::null_mut();
            let ret = unsafe {
                crate::generated::svn_repos_open(&mut repos, path.as_ptr(), pool.as_mut_ptr())
            };
            Error::from_raw(ret)?;
            Ok::<_, Error>(repos)
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
                Error::from_raw(ret)?;
                Ok::<_, Error>(capabilities)
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
            Error::from_raw(ret)?;
            Ok(has != 0)
        }
    }

    pub fn remember_client_capabilities(
        &mut self,
        capabilities: &[&str],
    ) -> Result<(), crate::Error> {
        let capabilities = capabilities
            .iter()
            .map(|c| std::ffi::CString::new(*c).unwrap())
            .collect::<Vec<_>>();
        let capabilities_array = capabilities
            .iter()
            .map(|c| c.as_ptr())
            .collect::<apr::tables::ArrayHeader<_>>();
        let ret = unsafe {
            crate::generated::svn_repos_remember_client_capabilities(
                self.0.as_mut_ptr(),
                capabilities_array.as_ptr(),
            )
        };
        Error::from_raw(ret)?;
        Ok(())
    }

    pub fn fs(&mut self) -> Result<crate::fs::Fs, crate::Error> {
        unsafe {
            Ok(crate::fs::Fs(PooledPtr::in_pool(
                self.0.pool(),
                crate::generated::svn_repos_fs(self.0.as_mut_ptr()),
            )))
        }
    }

    pub fn fs_type(&mut self) -> String {
        let mut pool = apr::Pool::new();
        let ret =
            unsafe { crate::generated::svn_repos_fs_type(self.0.as_mut_ptr(), pool.as_mut_ptr()) };
        let fs_type = unsafe { std::ffi::CStr::from_ptr(ret) };
        fs_type.to_str().unwrap().to_string()
    }

    pub fn path(&mut self) -> std::path::PathBuf {
        let mut pool = apr::Pool::new();
        let ret =
            unsafe { crate::generated::svn_repos_path(self.0.as_mut_ptr(), pool.as_mut_ptr()) };
        let path = unsafe { std::ffi::CStr::from_ptr(ret) };
        std::path::PathBuf::from(path.to_str().unwrap())
    }

    pub fn db_env(&mut self) -> std::path::PathBuf {
        let mut pool = apr::Pool::new();
        let ret =
            unsafe { crate::generated::svn_repos_db_env(self.0.as_mut_ptr(), pool.as_mut_ptr()) };
        let db_env = unsafe { std::ffi::CStr::from_ptr(ret) };
        std::path::PathBuf::from(db_env.to_str().unwrap())
    }

    pub fn conf_dir(&mut self) -> std::path::PathBuf {
        let mut pool = apr::Pool::new();
        let ret =
            unsafe { crate::generated::svn_repos_conf_dir(self.0.as_mut_ptr(), pool.as_mut_ptr()) };
        let conf_dir = unsafe { std::ffi::CStr::from_ptr(ret) };
        std::path::PathBuf::from(conf_dir.to_str().unwrap())
    }

    pub fn svnserve_conf(&mut self) -> std::path::PathBuf {
        let mut pool = apr::Pool::new();
        let ret = unsafe {
            crate::generated::svn_repos_svnserve_conf(self.0.as_mut_ptr(), pool.as_mut_ptr())
        };
        let svnserve_conf = unsafe { std::ffi::CStr::from_ptr(ret) };
        std::path::PathBuf::from(svnserve_conf.to_str().unwrap())
    }

    pub fn lock_dir(&mut self) -> std::path::PathBuf {
        let mut pool = apr::Pool::new();
        let ret =
            unsafe { crate::generated::svn_repos_lock_dir(self.0.as_mut_ptr(), pool.as_mut_ptr()) };
        let lock_dir = unsafe { std::ffi::CStr::from_ptr(ret) };
        std::path::PathBuf::from(lock_dir.to_str().unwrap())
    }

    pub fn db_lockfile(&mut self) -> std::path::PathBuf {
        let mut pool = apr::Pool::new();
        let ret = unsafe {
            crate::generated::svn_repos_db_lockfile(self.0.as_mut_ptr(), pool.as_mut_ptr())
        };
        let db_lockfile = unsafe { std::ffi::CStr::from_ptr(ret) };
        std::path::PathBuf::from(db_lockfile.to_str().unwrap())
    }

    pub fn db_logs_lockfile(&mut self) -> std::path::PathBuf {
        let mut pool = apr::Pool::new();
        let ret = unsafe {
            crate::generated::svn_repos_db_logs_lockfile(self.0.as_mut_ptr(), pool.as_mut_ptr())
        };
        let logs_lockfile = unsafe { std::ffi::CStr::from_ptr(ret) };
        std::path::PathBuf::from(logs_lockfile.to_str().unwrap())
    }

    pub fn hook_dir(&mut self) -> std::path::PathBuf {
        let mut pool = apr::Pool::new();
        let ret =
            unsafe { crate::generated::svn_repos_hook_dir(self.0.as_mut_ptr(), pool.as_mut_ptr()) };
        let hook_dir = unsafe { std::ffi::CStr::from_ptr(ret) };
        std::path::PathBuf::from(hook_dir.to_str().unwrap())
    }
}

#[allow(dead_code)]
pub struct Notify(PooledPtr<crate::generated::svn_repos_notify_t>);

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
    Error::from_raw(ret)?;
    Ok(())
}

pub fn delete(path: &std::path::Path) -> Result<(), crate::Error> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let mut pool = apr::Pool::new();
    let ret = unsafe { crate::generated::svn_repos_delete(path.as_ptr(), pool.as_mut_ptr()) };
    Error::from_raw(ret)?;
    Ok(())
}

pub fn version() -> crate::Version {
    unsafe { crate::Version(crate::generated::svn_client_version()) }
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

    #[test]
    fn test_capabilities() {
        let td = tempfile::tempdir().unwrap();
        let mut repos = super::Repos::create(td.path()).unwrap();
        assert!(repos.capabilities().unwrap().contains("mergeinfo"));
        assert!(!repos.capabilities().unwrap().contains("mergeinfo2"));
        assert!(repos.has_capability("mergeinfo").unwrap());
        assert!(repos.has_capability("unknown").is_err());
    }

    #[test]
    fn test_version() {
        assert!(super::version().major() >= 1);
    }
}
