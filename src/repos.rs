use crate::{svn_result, with_tmp_pool, Error, Revnum};
use std::marker::PhantomData;
use subversion_sys::{svn_repos_create, svn_repos_find_root_path, svn_repos_t};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoadUUID {
    #[default]
    /// Only update UUID if the repos has no revisions
    Default,

    /// Never update UUID
    Ignore,

    /// Always update UUID
    Force,
}

impl From<subversion_sys::svn_repos_load_uuid> for LoadUUID {
    fn from(load_uuid: subversion_sys::svn_repos_load_uuid) -> Self {
        match load_uuid {
            subversion_sys::svn_repos_load_uuid_svn_repos_load_uuid_default => Self::Default,
            subversion_sys::svn_repos_load_uuid_svn_repos_load_uuid_ignore => Self::Ignore,
            subversion_sys::svn_repos_load_uuid_svn_repos_load_uuid_force => Self::Force,
            _ => unreachable!(),
        }
    }
}

impl From<LoadUUID> for subversion_sys::svn_repos_load_uuid {
    fn from(load_uuid: LoadUUID) -> Self {
        match load_uuid {
            LoadUUID::Default => subversion_sys::svn_repos_load_uuid_svn_repos_load_uuid_default,
            LoadUUID::Ignore => subversion_sys::svn_repos_load_uuid_svn_repos_load_uuid_ignore,
            LoadUUID::Force => subversion_sys::svn_repos_load_uuid_svn_repos_load_uuid_force,
        }
    }
}

pub fn find_root_path(path: &std::path::Path) -> Option<std::path::PathBuf> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    with_tmp_pool(|pool| {
        let ret = unsafe { svn_repos_find_root_path(path.as_ptr(), pool.as_mut_ptr()) };
        if ret.is_null() {
            None
        } else {
            let root_path = unsafe { std::ffi::CStr::from_ptr(ret) };
            Some(std::path::PathBuf::from(root_path.to_str().unwrap()))
        }
    })
}

/// Repository handle with RAII cleanup
pub struct Repos {
    ptr: *mut svn_repos_t,
    pool: apr::Pool,
    _phantom: PhantomData<*mut ()>, // !Send + !Sync
}

impl Drop for Repos {
    fn drop(&mut self) {
        // Pool drop will clean up repos
    }
}

impl Repos {
    pub fn create(path: &std::path::Path) -> Result<Repos, Error> {
        Self::create_with_config(path, None, None)
    }
    
    pub fn create_with_config(
        path: &std::path::Path,
        config: Option<&std::collections::HashMap<String, String>>,
        fs_config: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Repos, Error> {
        let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
        let pool = apr::Pool::new();
        
        // Convert config HashMap to APR hash if provided
        let config_hash = if let Some(cfg) = config {
            let mut hash = apr::hash::Hash::<&[u8], &[u8]>::new(&pool);
            for (k, v) in cfg.iter() {
                hash.set(&k.as_bytes(), &v.as_bytes());
            }
            unsafe { hash.as_mut_ptr() }
        } else {
            std::ptr::null_mut()
        };
        
        // Convert fs_config HashMap to APR hash if provided  
        let fs_config_hash = if let Some(cfg) = fs_config {
            let mut hash = apr::hash::Hash::<&[u8], &[u8]>::new(&pool);
            for (k, v) in cfg.iter() {
                hash.set(&k.as_bytes(), &v.as_bytes());
            }
            unsafe { hash.as_mut_ptr() }
        } else {
            std::ptr::null_mut()
        };

        unsafe {
            let mut repos: *mut svn_repos_t = std::ptr::null_mut();
            let ret = svn_repos_create(
                &mut repos,
                path.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                config_hash,
                fs_config_hash,
                pool.as_mut_ptr(),
            );
            svn_result(ret)?;
            Ok(Repos {
                ptr: repos,
                pool,
                _phantom: PhantomData,
            })
        }
    }

    pub fn open(path: &std::path::Path) -> Result<Repos, Error> {
        let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
        let pool = apr::Pool::new();

        unsafe {
            let mut repos: *mut svn_repos_t = std::ptr::null_mut();
            let ret = subversion_sys::svn_repos_open(&mut repos, path.as_ptr(), pool.as_mut_ptr());
            svn_result(ret)?;
            Ok(Repos {
                ptr: repos,
                pool,
                _phantom: PhantomData,
            })
        }
    }

    pub fn capabilities(&mut self) -> Result<std::collections::HashSet<String>, Error> {
        let pool = apr::Pool::new();
        let scratch_pool = apr::Pool::new();
        let mut capabilities: *mut apr_sys::apr_hash_t = std::ptr::null_mut();
        let ret = unsafe {
            subversion_sys::svn_repos_capabilities(
                &mut capabilities,
                self.ptr,
                pool.as_mut_ptr(),
                scratch_pool.as_mut_ptr(),
            )
        };
        Error::from_raw(ret)?;
        let mut capabilities_hash = apr::hash::Hash::<&str, &str>::from_ptr(capabilities);
        Ok(capabilities_hash
            .keys(&pool)
            .map(|k| String::from_utf8_lossy(k).to_string())
            .collect::<std::collections::HashSet<String>>())
    }

    pub fn has_capability(&mut self, capability: &str) -> Result<bool, Error> {
        let capability = std::ffi::CString::new(capability).unwrap();
        let pool = apr::Pool::new();
        unsafe {
            let mut has: subversion_sys::svn_boolean_t = 0;
            let ret = subversion_sys::svn_repos_has_capability(
                self.ptr,
                &mut has,
                capability.as_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(ret)?;
            Ok(has != 0)
        }
    }

    pub fn remember_client_capabilities(&mut self, capabilities: &[&str]) -> Result<(), Error> {
        let pool = apr::Pool::new();
        let capabilities = capabilities
            .iter()
            .map(|c| std::ffi::CString::new(*c).unwrap())
            .collect::<Vec<_>>();
        let mut capabilities_array = apr::tables::ArrayHeader::<*const i8>::new(&pool);
        for cap in capabilities.iter() {
            capabilities_array.push(cap.as_ptr());
        }
        let ret = unsafe {
            subversion_sys::svn_repos_remember_client_capabilities(
                self.ptr,
                capabilities_array.as_ptr(),
            )
        };
        Error::from_raw(ret)?;
        Ok(())
    }

    pub fn fs(&self) -> Option<crate::fs::Fs> {
        let fs_ptr = unsafe { subversion_sys::svn_repos_fs(self.ptr) };

        if fs_ptr.is_null() {
            None
        } else {
            // Create a child pool from the repos pool to keep the fs alive
            let child_pool = apr::Pool::new();
            Some(unsafe { crate::fs::Fs::from_ptr_and_pool(fs_ptr, child_pool) })
        }
    }

    pub fn fs_type(&self) -> String {
        with_tmp_pool(|pool| {
            let ret = unsafe { subversion_sys::svn_repos_fs_type(self.ptr, pool.as_mut_ptr()) };
            let fs_type = unsafe { std::ffi::CStr::from_ptr(ret) };
            fs_type.to_str().unwrap().to_string()
        })
    }

    pub fn path(&mut self) -> std::path::PathBuf {
        with_tmp_pool(|pool| {
            let ret = unsafe { subversion_sys::svn_repos_path(self.ptr, pool.as_mut_ptr()) };
            let path = unsafe { std::ffi::CStr::from_ptr(ret) };
            std::path::PathBuf::from(path.to_str().unwrap())
        })
    }

    pub fn db_env(&mut self) -> std::path::PathBuf {
        let pool = apr::Pool::new();
        let ret = unsafe { subversion_sys::svn_repos_db_env(self.ptr, pool.as_mut_ptr()) };
        let db_env = unsafe { std::ffi::CStr::from_ptr(ret) };
        std::path::PathBuf::from(db_env.to_str().unwrap())
    }

    pub fn conf_dir(&mut self) -> std::path::PathBuf {
        let pool = apr::Pool::new();
        let ret = unsafe { subversion_sys::svn_repos_conf_dir(self.ptr, pool.as_mut_ptr()) };
        let conf_dir = unsafe { std::ffi::CStr::from_ptr(ret) };
        std::path::PathBuf::from(conf_dir.to_str().unwrap())
    }

    pub fn svnserve_conf(&mut self) -> std::path::PathBuf {
        let pool = apr::Pool::new();
        let ret = unsafe { subversion_sys::svn_repos_svnserve_conf(self.ptr, pool.as_mut_ptr()) };
        let svnserve_conf = unsafe { std::ffi::CStr::from_ptr(ret) };
        std::path::PathBuf::from(svnserve_conf.to_str().unwrap())
    }

    pub fn lock_dir(&mut self) -> std::path::PathBuf {
        let pool = apr::Pool::new();
        let ret = unsafe { subversion_sys::svn_repos_lock_dir(self.ptr, pool.as_mut_ptr()) };
        let lock_dir = unsafe { std::ffi::CStr::from_ptr(ret) };
        std::path::PathBuf::from(lock_dir.to_str().unwrap())
    }

    pub fn db_lockfile(&mut self) -> std::path::PathBuf {
        let pool = apr::Pool::new();
        let ret = unsafe { subversion_sys::svn_repos_db_lockfile(self.ptr, pool.as_mut_ptr()) };
        let db_lockfile = unsafe { std::ffi::CStr::from_ptr(ret) };
        std::path::PathBuf::from(db_lockfile.to_str().unwrap())
    }

    pub fn db_logs_lockfile(&mut self) -> std::path::PathBuf {
        let pool = apr::Pool::new();
        let ret =
            unsafe { subversion_sys::svn_repos_db_logs_lockfile(self.ptr, pool.as_mut_ptr()) };
        let logs_lockfile = unsafe { std::ffi::CStr::from_ptr(ret) };
        std::path::PathBuf::from(logs_lockfile.to_str().unwrap())
    }

    pub fn hook_dir(&mut self) -> std::path::PathBuf {
        let pool = apr::Pool::new();
        let ret = unsafe { subversion_sys::svn_repos_hook_dir(self.ptr, pool.as_mut_ptr()) };
        let hook_dir = unsafe { std::ffi::CStr::from_ptr(ret) };
        std::path::PathBuf::from(hook_dir.to_str().unwrap())
    }

    pub fn load_fs(
        &self,
        dumpstream: &mut crate::io::Stream,
        start_rev: Revnum,
        end_rev: Revnum,
        uuid_action: LoadUUID,
        parent_dir: &std::path::Path,
        use_pre_commit_hook: bool,
        use_post_commit_hook: bool,
        cancel_check: Option<&impl Fn() -> bool>,
        validate_props: bool,
        ignore_dates: bool,
        normalize_props: bool,
        notify_func: Option<&impl Fn(&Notify)>,
    ) -> Result<(), Error> {
        let pool = apr::Pool::new();
        let ret = unsafe {
            subversion_sys::svn_repos_load_fs6(
                self.ptr,
                dumpstream.as_mut_ptr(),
                start_rev.0,
                end_rev.0,
                uuid_action.into(),
                parent_dir.to_str().unwrap().as_ptr() as *const i8,
                use_pre_commit_hook.into(),
                use_post_commit_hook.into(),
                validate_props.into(),
                ignore_dates.into(),
                normalize_props.into(),
                if notify_func.is_some() {
                    Some(wrap_notify_func)
                } else {
                    None
                },
                notify_func
                    .map(|notify_func| {
                        Box::into_raw(Box::new(notify_func)) as *mut std::ffi::c_void
                    })
                    .unwrap_or(std::ptr::null_mut()),
                if cancel_check.is_some() {
                    Some(crate::wrap_cancel_func)
                } else {
                    None
                },
                cancel_check
                    .map(|cancel_check| {
                        Box::into_raw(Box::new(cancel_check)) as *mut std::ffi::c_void
                    })
                    .unwrap_or(std::ptr::null_mut()),
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(ret)?;
        Ok(())
    }

    pub fn verify_fs(
        &self,
        start_rev: Revnum,
        end_rev: Revnum,
        check_normalization: bool,
        metadata_only: bool,
        notify_func: Option<&impl Fn(&Notify)>,
        callback_func: &impl Fn(Revnum, &Error) -> Result<(), Error>,
        cancel_func: Option<&impl Fn() -> Result<(), Error>>,
    ) -> Result<(), Error> {
        extern "C" fn verify_callback(
            baton: *mut std::ffi::c_void,
            revision: subversion_sys::svn_revnum_t,
            verify_err: *mut subversion_sys::svn_error_t,
            _pool: *mut apr_sys::apr_pool_t,
        ) -> *mut subversion_sys::svn_error_t {
            let baton = unsafe {
                &mut *(baton as *mut Box<dyn FnMut(Revnum, &Error) -> Result<(), Error>>)
            };
            let verify_err = Error::from_raw(verify_err).unwrap_err();
            match baton(Revnum::from_raw(revision).unwrap(), &verify_err) {
                Ok(()) => std::ptr::null_mut(),
                Err(e) => unsafe { e.into_raw() },
            }
        }
        let pool = apr::Pool::new();
        let ret = unsafe {
            subversion_sys::svn_repos_verify_fs3(
                self.ptr,
                start_rev.0,
                end_rev.0,
                check_normalization.into(),
                metadata_only.into(),
                if notify_func.is_some() {
                    Some(wrap_notify_func)
                } else {
                    None
                },
                notify_func
                    .map(|notify_func| {
                        Box::into_raw(Box::new(notify_func)) as *mut std::ffi::c_void
                    })
                    .unwrap_or(std::ptr::null_mut()),
                Some(verify_callback),
                Box::into_raw(Box::new(callback_func)) as *mut std::ffi::c_void,
                if cancel_func.is_some() {
                    Some(crate::wrap_cancel_func)
                } else {
                    None
                },
                cancel_func
                    .map(|cancel_func| {
                        Box::into_raw(Box::new(cancel_func)) as *mut std::ffi::c_void
                    })
                    .unwrap_or(std::ptr::null_mut()),
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(ret)?;
        Ok(())
    }

    pub fn pack_fs(
        &self,
        notify_func: Option<&impl Fn(&Notify)>,
        cancel_func: Option<&impl Fn() -> Result<(), Error>>,
    ) -> Result<(), Error> {
        let pool = apr::Pool::new();
        let ret = unsafe {
            subversion_sys::svn_repos_fs_pack2(
                self.ptr,
                if notify_func.is_some() {
                    Some(wrap_notify_func)
                } else {
                    None
                },
                notify_func
                    .map(|notify_func| {
                        Box::into_raw(Box::new(notify_func)) as *mut std::ffi::c_void
                    })
                    .unwrap_or(std::ptr::null_mut()),
                if cancel_func.is_some() {
                    Some(crate::wrap_cancel_func)
                } else {
                    None
                },
                cancel_func
                    .map(|cancel_func| {
                        Box::into_raw(Box::new(cancel_func)) as *mut std::ffi::c_void
                    })
                    .unwrap_or(std::ptr::null_mut()),
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(ret)?;
        Ok(())
    }
}

pub fn recover(
    path: &str,
    nonblocking: bool,
    notify_func: Option<&impl Fn(&Notify)>,
    cance_func: Option<&impl Fn() -> Result<(), Error>>,
) -> Result<(), Error> {
    let path = std::ffi::CString::new(path).unwrap();
    let pool = apr::Pool::new();
    let ret = unsafe {
        subversion_sys::svn_repos_recover4(
            path.as_ptr(),
            nonblocking.into(),
            if notify_func.is_some() {
                Some(wrap_notify_func)
            } else {
                None
            },
            notify_func
                .map(|notify_func| Box::into_raw(Box::new(notify_func)) as *mut std::ffi::c_void)
                .unwrap_or(std::ptr::null_mut()),
            if cance_func.is_some() {
                Some(crate::wrap_cancel_func)
            } else {
                None
            },
            cance_func
                .map(|cancel_func| Box::into_raw(Box::new(cancel_func)) as *mut std::ffi::c_void)
                .unwrap_or(std::ptr::null_mut()),
            pool.as_mut_ptr(),
        )
    };
    Error::from_raw(ret)?;
    Ok(())
}

extern "C" fn wrap_freeze_func(
    baton: *mut std::ffi::c_void,
    _pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let freeze_func = unsafe { &*(baton as *const Box<dyn Fn() -> Result<(), Error>>) };
    match freeze_func() {
        Ok(()) => std::ptr::null_mut(),
        Err(e) => unsafe { e.into_raw() },
    }
}

pub fn freeze(
    paths: &[&str],
    freeze_func: Option<&impl Fn(&str) -> Result<(), Error>>,
) -> Result<(), Error> {
    let paths = paths
        .iter()
        .map(|p| std::ffi::CString::new(*p).unwrap())
        .collect::<Vec<_>>();
    let pool = apr::Pool::new();
    let mut paths_array = apr::tables::ArrayHeader::<*const i8>::new(&pool);
    for path in paths.iter() {
        paths_array.push(path.as_ptr());
    }
    let ret = unsafe {
        subversion_sys::svn_repos_freeze(
            paths_array.as_ptr(),
            if freeze_func.is_some() {
                Some(wrap_freeze_func)
            } else {
                None
            },
            freeze_func
                .map(|freeze_func| Box::into_raw(Box::new(freeze_func)) as *mut std::ffi::c_void)
                .unwrap_or(std::ptr::null_mut()),
            pool.as_mut_ptr(),
        )
    };
    Error::from_raw(ret)?;
    Ok(())
}

/// Notify handle - borrowed from callback
#[allow(dead_code)]
pub struct Notify<'a> {
    ptr: *const subversion_sys::svn_repos_notify_t,
    _phantom: PhantomData<&'a ()>,
}

impl<'a> Notify<'a> {
    unsafe fn from_ptr(ptr: *const subversion_sys::svn_repos_notify_t) -> Self {
        Self {
            ptr,
            _phantom: PhantomData,
        }
    }
}

extern "C" fn wrap_notify_func(
    baton: *mut std::ffi::c_void,
    notify: *const subversion_sys::svn_repos_notify_t,
    _pool: *mut apr_sys::apr_pool_t,
) {
    let baton = unsafe { &mut *(baton as *mut Box<dyn FnMut(&Notify)>) };
    unsafe {
        baton(&Notify::from_ptr(notify));
    }
}

pub fn upgrade(
    path: &std::path::Path,
    nonblocking: bool,
    notify_func: Option<&mut dyn FnMut(&Notify)>,
) -> Result<(), Error> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let notify_func = notify_func.map(|_notify_func| wrap_notify_func as _);
    let notify_baton = Box::into_raw(Box::new(notify_func)).cast();
    let pool = apr::Pool::new();
    let ret = unsafe {
        subversion_sys::svn_repos_upgrade2(
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

pub fn delete(path: &std::path::Path) -> Result<(), Error> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let pool = apr::Pool::new();
    let ret = unsafe { subversion_sys::svn_repos_delete(path.as_ptr(), pool.as_mut_ptr()) };
    Error::from_raw(ret)?;
    Ok(())
}

pub fn version() -> crate::Version {
    unsafe { crate::Version(subversion_sys::svn_repos_version()) }
}

pub fn hotcopy(
    src_path: &std::path::Path,
    dst_path: &std::path::Path,
    clean_logs: bool,
    incremental: bool,
    notify_func: Option<&impl Fn(&Notify)>,
    cancel_func: Option<&impl Fn() -> Result<(), Error>>,
) -> Result<(), Error> {
    let src_path = std::ffi::CString::new(src_path.to_str().unwrap()).unwrap();
    let dst_path = std::ffi::CString::new(dst_path.to_str().unwrap()).unwrap();
    let pool = apr::Pool::new();
    let ret = unsafe {
        subversion_sys::svn_repos_hotcopy3(
            src_path.as_ptr(),
            dst_path.as_ptr(),
            clean_logs.into(),
            incremental.into(),
            if notify_func.is_some() {
                Some(wrap_notify_func)
            } else {
                None
            },
            notify_func
                .map(|notify_func| Box::into_raw(Box::new(notify_func)) as *mut std::ffi::c_void)
                .unwrap_or(std::ptr::null_mut()),
            if cancel_func.is_some() {
                Some(crate::wrap_cancel_func)
            } else {
                None
            },
            cancel_func
                .map(|cancel_func| Box::into_raw(Box::new(cancel_func)) as *mut std::ffi::c_void)
                .unwrap_or(std::ptr::null_mut()),
            pool.as_mut_ptr(),
        )
    };
    Error::from_raw(ret)?;
    Ok(())
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

    #[test]
    fn test_create_with_config() {
        let td = tempfile::tempdir().unwrap();
        let config = std::collections::HashMap::from([
            ("repository.compression".to_string(), "6".to_string()),
        ]);
        let fs_config = std::collections::HashMap::from([
            ("fsfs.compression".to_string(), "zlib".to_string()),
        ]);
        
        let repos = super::Repos::create_with_config(td.path(), Some(&config), Some(&fs_config));
        assert!(repos.is_ok(), "Failed to create repository with config");
        
        // Test that we can open the created repository
        let repos = super::Repos::open(td.path());
        assert!(repos.is_ok(), "Failed to open repository created with config");
    }

    #[test]
    fn test_create_with_empty_config() {
        let td = tempfile::tempdir().unwrap();
        let empty_config = std::collections::HashMap::new();
        
        let repos = super::Repos::create_with_config(td.path(), Some(&empty_config), None);
        assert!(repos.is_ok(), "Failed to create repository with empty config");
    }
}
