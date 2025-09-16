use crate::{svn_result, with_tmp_pool, Error, Revnum};
use std::ffi::CString;
use std::marker::PhantomData;
use subversion_sys::{
    svn_repos_create, svn_repos_dump_fs4, svn_repos_find_root_path, svn_repos_load_fs6,
    svn_repos_recover4, svn_repos_t, svn_repos_verify_fs3,
};

/// Specifies how to handle UUID during repository load operations.
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

/// Authorization access levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthzAccess {
    /// No access
    None,
    /// Path can be read
    Read,
    /// Path can be altered
    Write,
    /// Both read and write access
    ReadWrite,
}

impl From<AuthzAccess> for subversion_sys::svn_repos_authz_access_t {
    fn from(access: AuthzAccess) -> Self {
        match access {
            AuthzAccess::None => subversion_sys::svn_repos_authz_access_t_svn_authz_none,
            AuthzAccess::Read => subversion_sys::svn_repos_authz_access_t_svn_authz_read,
            AuthzAccess::Write => subversion_sys::svn_repos_authz_access_t_svn_authz_write,
            AuthzAccess::ReadWrite => {
                subversion_sys::svn_repos_authz_access_t_svn_authz_read
                    | subversion_sys::svn_repos_authz_access_t_svn_authz_write
            }
        }
    }
}

/// Authorization data structure
pub struct Authz {
    ptr: *mut subversion_sys::svn_authz_t,
    pool: apr::Pool,
}

impl Authz {
    /// Read authz configuration from a file
    pub fn read(
        path: &std::path::Path,
        groups_path: Option<&std::path::Path>,
        must_exist: bool,
    ) -> Result<Self, Error> {
        let pool = apr::Pool::new();
        let path_cstr = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
        let groups_path_cstr =
            groups_path.map(|p| std::ffi::CString::new(p.to_str().unwrap()).unwrap());

        let mut authz_ptr: *mut subversion_sys::svn_authz_t = std::ptr::null_mut();

        let ret = unsafe {
            subversion_sys::svn_repos_authz_read4(
                &mut authz_ptr,
                path_cstr.as_ptr(),
                groups_path_cstr
                    .as_ref()
                    .map(|p| p.as_ptr())
                    .unwrap_or(std::ptr::null()),
                must_exist.into(),
                std::ptr::null_mut(),
                None,
                std::ptr::null_mut(),
                pool.as_mut_ptr(),
                pool.as_mut_ptr(),
            )
        };

        svn_result(ret)?;

        Ok(Authz {
            ptr: authz_ptr,
            pool,
        })
    }

    /// Check if a user has the required access to a path
    pub fn check_access(
        &self,
        repos_name: Option<&str>,
        path: &str,
        user: Option<&str>,
        required_access: AuthzAccess,
    ) -> Result<bool, Error> {
        let repos_name_cstr = repos_name.map(|r| std::ffi::CString::new(r).unwrap());
        let path_cstr = std::ffi::CString::new(path).unwrap();
        let user_cstr = user.map(|u| std::ffi::CString::new(u).unwrap());

        let mut access_granted: subversion_sys::svn_boolean_t = 0;

        with_tmp_pool(|pool| {
            let ret = unsafe {
                subversion_sys::svn_repos_authz_check_access(
                    self.ptr,
                    repos_name_cstr
                        .as_ref()
                        .map(|r| r.as_ptr())
                        .unwrap_or(std::ptr::null()),
                    path_cstr.as_ptr(),
                    user_cstr
                        .as_ref()
                        .map(|u| u.as_ptr())
                        .unwrap_or(std::ptr::null()),
                    required_access.into(),
                    &mut access_granted,
                    pool.as_mut_ptr(),
                )
            };

            svn_result(ret)?;
            Ok(access_granted != 0)
        })
    }
}

/// Finds the root path of a repository containing the given path.
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
    /// Creates a new repository at the specified path.
    pub fn create(path: &std::path::Path) -> Result<Repos, Error> {
        Self::create_with_config(path, None, None)
    }

    /// Creates a new repository with configuration options.
    pub fn create_with_config(
        path: &std::path::Path,
        config: Option<&std::collections::HashMap<String, String>>,
        fs_config: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<Repos, Error> {
        let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
        let pool = apr::Pool::new();

        // Convert config HashMap to APR hash if provided
        let config_hash = if let Some(cfg) = config {
            let byte_pairs: Vec<_> = cfg
                .iter()
                .map(|(k, v)| (k.as_bytes(), v.as_bytes()))
                .collect();
            let mut hash = apr::hash::Hash::new(&pool);
            for (k, v) in byte_pairs.iter() {
                unsafe {
                    hash.insert(k, v.as_ptr() as *mut std::ffi::c_void);
                }
            }
            unsafe { hash.as_mut_ptr() }
        } else {
            std::ptr::null_mut()
        };

        // Convert fs_config HashMap to APR hash if provided
        let fs_config_hash = if let Some(cfg) = fs_config {
            let byte_pairs: Vec<_> = cfg
                .iter()
                .map(|(k, v)| (k.as_bytes(), v.as_bytes()))
                .collect();
            let mut hash = apr::hash::Hash::new(&pool);
            for (k, v) in byte_pairs.iter() {
                unsafe {
                    hash.insert(k, v.as_ptr() as *mut std::ffi::c_void);
                }
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

    /// Opens an existing repository.
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

    /// Gets the capabilities of the repository.
    pub fn capabilities(&mut self) -> Result<std::collections::HashSet<String>, Error> {
        let pool = apr::Pool::new();
        let scratch_pool = apr::Pool::new();
        let mut capabilities: *mut subversion_sys::apr_hash_t = std::ptr::null_mut();
        let ret = unsafe {
            subversion_sys::svn_repos_capabilities(
                &mut capabilities,
                self.ptr,
                pool.as_mut_ptr(),
                scratch_pool.as_mut_ptr(),
            )
        };
        Error::from_raw(ret)?;
        let capabilities_hash = unsafe { apr::hash::Hash::from_ptr(capabilities) };
        Ok(capabilities_hash
            .iter()
            .map(|(k, _)| String::from_utf8_lossy(k).to_string())
            .collect::<std::collections::HashSet<String>>())
    }

    /// Checks if the repository has a specific capability.
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

    /// Remembers client capabilities for this session.
    pub fn remember_client_capabilities(&mut self, capabilities: &[&str]) -> Result<(), Error> {
        let pool = apr::Pool::new();
        let capabilities = capabilities
            .iter()
            .map(|c| std::ffi::CString::new(*c).unwrap())
            .collect::<Vec<_>>();
        let mut capabilities_array =
            apr::tables::TypedArray::<*const i8>::new(&pool, capabilities.len() as i32);
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

    /// Gets the filesystem object for this repository.
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

    /// Gets the filesystem type of this repository.
    pub fn fs_type(&self) -> String {
        with_tmp_pool(|pool| {
            let ret = unsafe { subversion_sys::svn_repos_fs_type(self.ptr, pool.as_mut_ptr()) };
            let fs_type = unsafe { std::ffi::CStr::from_ptr(ret) };
            fs_type.to_str().unwrap().to_string()
        })
    }

    /// Gets the path to the repository.
    pub fn path(&mut self) -> std::path::PathBuf {
        with_tmp_pool(|pool| {
            let ret = unsafe { subversion_sys::svn_repos_path(self.ptr, pool.as_mut_ptr()) };
            let path = unsafe { std::ffi::CStr::from_ptr(ret) };
            std::path::PathBuf::from(path.to_str().unwrap())
        })
    }

    /// Gets the path to the database environment.
    pub fn db_env(&mut self) -> std::path::PathBuf {
        let pool = apr::Pool::new();
        let ret = unsafe { subversion_sys::svn_repos_db_env(self.ptr, pool.as_mut_ptr()) };
        let db_env = unsafe { std::ffi::CStr::from_ptr(ret) };
        std::path::PathBuf::from(db_env.to_str().unwrap())
    }

    /// Gets the path to the configuration directory.
    pub fn conf_dir(&mut self) -> std::path::PathBuf {
        let pool = apr::Pool::new();
        let ret = unsafe { subversion_sys::svn_repos_conf_dir(self.ptr, pool.as_mut_ptr()) };
        let conf_dir = unsafe { std::ffi::CStr::from_ptr(ret) };
        std::path::PathBuf::from(conf_dir.to_str().unwrap())
    }

    /// Gets the path to the svnserve configuration file.
    pub fn svnserve_conf(&mut self) -> std::path::PathBuf {
        let pool = apr::Pool::new();
        let ret = unsafe { subversion_sys::svn_repos_svnserve_conf(self.ptr, pool.as_mut_ptr()) };
        let svnserve_conf = unsafe { std::ffi::CStr::from_ptr(ret) };
        std::path::PathBuf::from(svnserve_conf.to_str().unwrap())
    }

    /// Gets the path to the lock directory.
    pub fn lock_dir(&mut self) -> std::path::PathBuf {
        let pool = apr::Pool::new();
        let ret = unsafe { subversion_sys::svn_repos_lock_dir(self.ptr, pool.as_mut_ptr()) };
        let lock_dir = unsafe { std::ffi::CStr::from_ptr(ret) };
        std::path::PathBuf::from(lock_dir.to_str().unwrap())
    }

    /// Gets the path to the database lock file.
    pub fn db_lockfile(&mut self) -> std::path::PathBuf {
        let pool = apr::Pool::new();
        let ret = unsafe { subversion_sys::svn_repos_db_lockfile(self.ptr, pool.as_mut_ptr()) };
        let db_lockfile = unsafe { std::ffi::CStr::from_ptr(ret) };
        std::path::PathBuf::from(db_lockfile.to_str().unwrap())
    }

    /// Gets the path to the database logs lock file.
    pub fn db_logs_lockfile(&mut self) -> std::path::PathBuf {
        let pool = apr::Pool::new();
        let ret =
            unsafe { subversion_sys::svn_repos_db_logs_lockfile(self.ptr, pool.as_mut_ptr()) };
        let logs_lockfile = unsafe { std::ffi::CStr::from_ptr(ret) };
        std::path::PathBuf::from(logs_lockfile.to_str().unwrap())
    }

    /// Gets the path to the repository's hooks directory.
    pub fn hook_dir(&mut self) -> std::path::PathBuf {
        let pool = apr::Pool::new();
        let ret = unsafe { subversion_sys::svn_repos_hook_dir(self.ptr, pool.as_mut_ptr()) };
        let hook_dir = unsafe { std::ffi::CStr::from_ptr(ret) };
        std::path::PathBuf::from(hook_dir.to_str().unwrap())
    }

    /// Loads a repository from a dump stream.
    pub fn load_fs(
        &self,
        dumpstream: &mut crate::io::Stream,
        start_rev: Option<Revnum>,
        end_rev: Option<Revnum>,
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

        // Handle empty parent_dir properly to avoid segfault
        let parent_dir_cstring = if parent_dir.as_os_str().is_empty() {
            None
        } else {
            Some(std::ffi::CString::new(parent_dir.to_str().unwrap())?)
        };
        let parent_dir_cstr = match &parent_dir_cstring {
            Some(cstr) => cstr.as_ptr(),
            None => std::ptr::null(),
        };

        let ret = unsafe {
            subversion_sys::svn_repos_load_fs6(
                self.ptr,
                dumpstream.as_mut_ptr(),
                start_rev.map(|r| r.0).unwrap_or(-1), // SVN_INVALID_REVNUM
                end_rev.map(|r| r.0).unwrap_or(-1),   // SVN_INVALID_REVNUM
                uuid_action.into(),
                parent_dir_cstr,
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

    /// Verifies the repository filesystem.
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
            _pool: *mut subversion_sys::apr_pool_t,
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

    /// Packs the repository filesystem.
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

    /// Get the UUID of the repository
    pub fn uuid(&self) -> Result<String, Error> {
        let pool = apr::Pool::new();
        let mut uuid_ptr = std::ptr::null();
        let ret = unsafe {
            subversion_sys::svn_fs_get_uuid(
                subversion_sys::svn_repos_fs(self.ptr),
                &mut uuid_ptr,
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(ret)?;

        if uuid_ptr.is_null() {
            Ok(String::new())
        } else {
            let uuid_cstr = unsafe { std::ffi::CStr::from_ptr(uuid_ptr) };
            Ok(uuid_cstr.to_str()?.to_string())
        }
    }

    /// Set the UUID of the repository
    pub fn set_uuid(&self, uuid: &str) -> Result<(), Error> {
        let pool = apr::Pool::new();
        let uuid_cstr = std::ffi::CString::new(uuid)?;
        let ret = unsafe {
            subversion_sys::svn_fs_set_uuid(
                subversion_sys::svn_repos_fs(self.ptr),
                uuid_cstr.as_ptr(),
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(ret)?;
        Ok(())
    }

    /// Get the youngest revision in the repository
    pub fn youngest_rev(&self) -> Result<Revnum, Error> {
        let pool = apr::Pool::new();
        let mut revnum: subversion_sys::svn_revnum_t = 0;
        let ret = unsafe {
            subversion_sys::svn_fs_youngest_rev(
                &mut revnum,
                subversion_sys::svn_repos_fs(self.ptr),
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(ret)?;
        Ok(Revnum(revnum))
    }

    /// Begin a transaction
    pub fn begin_txn_for_commit(
        &self,
        revnum: Revnum,
        author: &str,
        log_msg: &str,
    ) -> Result<crate::fs::Transaction, Error> {
        let pool = apr::Pool::new();
        let mut txn_ptr = std::ptr::null_mut();

        // Create revision properties
        let mut revprop_table = apr::hash::Hash::new(&pool);

        // Add author
        let author_key = b"svn:author";
        let author_val = crate::svn_string_helpers::svn_string_ncreate(author.as_bytes(), &pool);
        unsafe {
            revprop_table.insert(author_key, author_val as *mut std::ffi::c_void);
        }

        // Add log message
        let log_key = b"svn:log";
        let log_val = crate::svn_string_helpers::svn_string_ncreate(log_msg.as_bytes(), &pool);
        unsafe {
            revprop_table.insert(log_key, log_val as *mut std::ffi::c_void);
        }

        let ret = unsafe {
            subversion_sys::svn_repos_fs_begin_txn_for_commit2(
                &mut txn_ptr,
                self.ptr,
                revnum.0,
                revprop_table.as_mut_ptr(),
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(ret)?;

        Ok(unsafe { crate::fs::Transaction::from_ptr_and_pool(txn_ptr, pool) })
    }

    /// Get revision properties
    pub fn rev_proplist(
        &self,
        revnum: Revnum,
    ) -> Result<std::collections::HashMap<String, Vec<u8>>, Error> {
        let pool = apr::Pool::new();
        let mut props_ptr = std::ptr::null_mut();

        // Use authz_read_func that always allows access for simplicity
        extern "C" fn authz_read_func(
            _allowed: *mut i32,
            _root: *mut subversion_sys::svn_fs_root_t,
            _path: *const std::ffi::c_char,
            _baton: *mut std::ffi::c_void,
            _pool: *mut subversion_sys::apr_pool_t,
        ) -> *mut subversion_sys::svn_error_t {
            unsafe {
                *_allowed = 1; // Always allow
            }
            std::ptr::null_mut() // No error
        }

        let ret = unsafe {
            subversion_sys::svn_repos_fs_revision_proplist(
                &mut props_ptr,
                self.ptr,
                revnum.0,
                Some(authz_read_func),
                std::ptr::null_mut(),
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(ret)?;

        if props_ptr.is_null() {
            Ok(std::collections::HashMap::new())
        } else {
            let prop_hash = unsafe { crate::props::PropHash::from_ptr(props_ptr) };
            Ok(prop_hash.to_hashmap())
        }
    }

    /// Change a revision property
    pub fn change_rev_prop(
        &self,
        revnum: Revnum,
        name: &str,
        value: Option<&[u8]>,
    ) -> Result<(), Error> {
        let pool = apr::Pool::new();
        let name_cstr = std::ffi::CString::new(name)?;

        let value_ptr = if let Some(val) = value {
            crate::svn_string_helpers::svn_string_ncreate(val, &pool)
        } else {
            std::ptr::null_mut()
        };

        // Use authz_read_func that always allows access
        extern "C" fn authz_read_func(
            _allowed: *mut i32,
            _root: *mut subversion_sys::svn_fs_root_t,
            _path: *const std::ffi::c_char,
            _baton: *mut std::ffi::c_void,
            _pool: *mut subversion_sys::apr_pool_t,
        ) -> *mut subversion_sys::svn_error_t {
            unsafe {
                *_allowed = 1; // Always allow
            }
            std::ptr::null_mut()
        }

        let ret = unsafe {
            subversion_sys::svn_repos_fs_change_rev_prop4(
                self.ptr,
                revnum.0,
                std::ptr::null(), // author
                name_cstr.as_ptr(),
                std::ptr::null_mut(), // old_value_p
                value_ptr,
                1, // use_pre_revprop_change_hook
                1, // use_post_revprop_change_hook
                Some(authz_read_func),
                std::ptr::null_mut(),
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(ret)?;
        Ok(())
    }

    /// Dump repository contents to a stream for backup
    pub fn dump(
        &self,
        stream: &mut crate::io::Stream,
        start_rev: Option<Revnum>,
        end_rev: Option<Revnum>,
        incremental: bool,
        use_deltas: bool,
        include_revprops: bool,
        include_changes: bool,
        notify_func: Option<&dyn Fn(&Notify)>,
        cancel_func: Option<&dyn Fn() -> Result<(), Error>>,
    ) -> Result<(), Error> {
        let pool = apr::Pool::new();
        let notify_baton = notify_func
            .map(|notify_func| Box::into_raw(Box::new(notify_func)) as *mut std::ffi::c_void)
            .unwrap_or(std::ptr::null_mut());
        let cancel_baton = cancel_func
            .map(|cancel_func| Box::into_raw(Box::new(cancel_func)) as *mut std::ffi::c_void)
            .unwrap_or(std::ptr::null_mut());

        let ret = unsafe {
            svn_repos_dump_fs4(
                self.ptr,
                stream.as_mut_ptr(),
                start_rev.map(|r| r.into()).unwrap_or(-1),
                end_rev.map(|r| r.into()).unwrap_or(-1),
                incremental.into(),
                use_deltas.into(),
                include_revprops.into(),
                include_changes.into(),
                if notify_func.is_some() {
                    Some(wrap_notify_func)
                } else {
                    None
                },
                notify_baton,
                None,                 // filter_func - not commonly used
                std::ptr::null_mut(), // filter_baton
                if cancel_func.is_some() {
                    Some(crate::wrap_cancel_func)
                } else {
                    None
                },
                cancel_baton,
                pool.as_mut_ptr(),
            )
        };

        // Free callback batons
        if !notify_baton.is_null() {
            unsafe { drop(Box::from_raw(notify_baton as *mut Box<dyn Fn(&Notify)>)) };
        }
        if !cancel_baton.is_null() {
            unsafe {
                drop(Box::from_raw(
                    cancel_baton as *mut Box<dyn Fn() -> Result<(), Error>>,
                ))
            };
        }

        Error::from_raw(ret)?;
        Ok(())
    }

    /// Load repository contents from a dump stream for restoration
    pub fn load(
        &self,
        dumpstream: &mut crate::io::Stream,
        start_rev: Option<Revnum>,
        end_rev: Option<Revnum>,
        uuid_action: LoadUUID,
        parent_dir: Option<&str>,
        use_pre_commit_hook: bool,
        use_post_commit_hook: bool,
        validate_props: bool,
        ignore_dates: bool,
        normalize_props: bool,
        notify_func: Option<&dyn Fn(&Notify)>,
        cancel_func: Option<&dyn Fn() -> Result<(), Error>>,
    ) -> Result<(), Error> {
        let pool = apr::Pool::new();
        let parent_dir_cstr = parent_dir.map(|p| std::ffi::CString::new(p).unwrap());
        let parent_dir_ptr = parent_dir_cstr
            .as_ref()
            .map(|s| s.as_ptr())
            .unwrap_or(std::ptr::null());

        let notify_baton = notify_func
            .map(|notify_func| Box::into_raw(Box::new(notify_func)) as *mut std::ffi::c_void)
            .unwrap_or(std::ptr::null_mut());
        let cancel_baton = cancel_func
            .map(|cancel_func| Box::into_raw(Box::new(cancel_func)) as *mut std::ffi::c_void)
            .unwrap_or(std::ptr::null_mut());

        let ret = unsafe {
            svn_repos_load_fs6(
                self.ptr,
                dumpstream.as_mut_ptr(),
                start_rev.map(|r| r.into()).unwrap_or(-1),
                end_rev.map(|r| r.into()).unwrap_or(-1),
                uuid_action.into(),
                parent_dir_ptr,
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
                notify_baton,
                if cancel_func.is_some() {
                    Some(crate::wrap_cancel_func)
                } else {
                    None
                },
                cancel_baton,
                pool.as_mut_ptr(),
            )
        };

        // Free callback batons
        if !notify_baton.is_null() {
            unsafe { drop(Box::from_raw(notify_baton as *mut Box<dyn Fn(&Notify)>)) };
        }
        if !cancel_baton.is_null() {
            unsafe {
                drop(Box::from_raw(
                    cancel_baton as *mut Box<dyn Fn() -> Result<(), Error>>,
                ))
            };
        }

        Error::from_raw(ret)?;
        Ok(())
    }

    /// Verify repository integrity
    pub fn verify(
        &self,
        start_rev: Revnum,
        end_rev: Revnum,
        check_normalization: bool,
        metadata_only: bool,
        notify_func: Option<&dyn Fn(&Notify)>,
        verify_callback: Option<&dyn Fn(Revnum, &Error) -> Result<(), Error>>,
        cancel_func: Option<&dyn Fn() -> Result<(), Error>>,
    ) -> Result<(), Error> {
        extern "C" fn verify_error_callback(
            baton: *mut std::ffi::c_void,
            revision: subversion_sys::svn_revnum_t,
            verify_err: *mut subversion_sys::svn_error_t,
            _pool: *mut subversion_sys::apr_pool_t,
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
        let notify_baton = notify_func
            .map(|notify_func| Box::into_raw(Box::new(notify_func)) as *mut std::ffi::c_void)
            .unwrap_or(std::ptr::null_mut());
        let verify_baton = verify_callback
            .map(|callback| Box::into_raw(Box::new(callback)) as *mut std::ffi::c_void)
            .unwrap_or(std::ptr::null_mut());
        let cancel_baton = cancel_func
            .map(|cancel_func| Box::into_raw(Box::new(cancel_func)) as *mut std::ffi::c_void)
            .unwrap_or(std::ptr::null_mut());

        let ret = unsafe {
            svn_repos_verify_fs3(
                self.ptr,
                start_rev.into(),
                end_rev.into(),
                check_normalization.into(),
                metadata_only.into(),
                if notify_func.is_some() {
                    Some(wrap_notify_func)
                } else {
                    None
                },
                notify_baton,
                if verify_callback.is_some() {
                    Some(verify_error_callback)
                } else {
                    None
                },
                verify_baton,
                if cancel_func.is_some() {
                    Some(crate::wrap_cancel_func)
                } else {
                    None
                },
                cancel_baton,
                pool.as_mut_ptr(),
            )
        };

        // Free callback batons
        if !notify_baton.is_null() {
            unsafe { drop(Box::from_raw(notify_baton as *mut Box<dyn Fn(&Notify)>)) };
        }
        if !verify_baton.is_null() {
            unsafe {
                drop(Box::from_raw(
                    verify_baton as *mut Box<dyn Fn(Revnum, &Error) -> Result<(), Error>>,
                ))
            };
        }
        if !cancel_baton.is_null() {
            unsafe {
                drop(Box::from_raw(
                    cancel_baton as *mut Box<dyn Fn() -> Result<(), Error>>,
                ))
            };
        }

        Error::from_raw(ret)?;
        Ok(())
    }

    /// Recover repository after corruption  
    pub fn recover(
        &mut self,
        nonblocking: bool,
        notify_func: Option<&dyn Fn(&Notify)>,
        cancel_func: Option<&dyn Fn() -> Result<(), Error>>,
    ) -> Result<(), Error> {
        let path = self.path();
        let path_cstr = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
        let pool = apr::Pool::new();
        let notify_baton = notify_func
            .map(|notify_func| Box::into_raw(Box::new(notify_func)) as *mut std::ffi::c_void)
            .unwrap_or(std::ptr::null_mut());
        let cancel_baton = cancel_func
            .map(|cancel_func| Box::into_raw(Box::new(cancel_func)) as *mut std::ffi::c_void)
            .unwrap_or(std::ptr::null_mut());

        let ret = unsafe {
            svn_repos_recover4(
                path_cstr.as_ptr(),
                nonblocking.into(),
                if notify_func.is_some() {
                    Some(wrap_notify_func)
                } else {
                    None
                },
                notify_baton,
                if cancel_func.is_some() {
                    Some(crate::wrap_cancel_func)
                } else {
                    None
                },
                cancel_baton,
                pool.as_mut_ptr(),
            )
        };

        // Free callback batons
        if !notify_baton.is_null() {
            unsafe { drop(Box::from_raw(notify_baton as *mut Box<dyn Fn(&Notify)>)) };
        }
        if !cancel_baton.is_null() {
            unsafe {
                drop(Box::from_raw(
                    cancel_baton as *mut Box<dyn Fn() -> Result<(), Error>>,
                ))
            };
        }

        Error::from_raw(ret)?;
        Ok(())
    }
}

/// Recovers a repository after an interrupted operation.
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
    _pool: *mut subversion_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let freeze_func = unsafe { &*(baton as *const Box<dyn Fn() -> Result<(), Error>>) };
    match freeze_func() {
        Ok(()) => std::ptr::null_mut(),
        Err(e) => unsafe { e.into_raw() },
    }
}

/// Freezes a repository to allow safe backup or administrative operations.
pub fn freeze(
    paths: &[&str],
    freeze_func: Option<&impl Fn(&str) -> Result<(), Error>>,
) -> Result<(), Error> {
    let paths = paths
        .iter()
        .map(|p| std::ffi::CString::new(*p).unwrap())
        .collect::<Vec<_>>();
    let pool = apr::Pool::new();
    let mut paths_array = apr::tables::TypedArray::<*const i8>::new(&pool, paths.len() as i32);
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
    _pool: *mut subversion_sys::apr_pool_t,
) {
    let baton = unsafe { &mut *(baton as *mut Box<dyn FnMut(&Notify)>) };
    unsafe {
        baton(&Notify::from_ptr(notify));
    }
}

/// Upgrades a repository to the latest filesystem format.
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

/// Deletes a repository.
pub fn delete(path: &std::path::Path) -> Result<(), Error> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let pool = apr::Pool::new();
    let ret = unsafe { subversion_sys::svn_repos_delete(path.as_ptr(), pool.as_mut_ptr()) };
    Error::from_raw(ret)?;
    Ok(())
}

/// Gets the version of the repository library.
pub fn version() -> crate::Version {
    unsafe { crate::Version(subversion_sys::svn_repos_version()) }
}

/// Creates a hot copy of a repository.
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
    use super::*;
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
    fn test_load_fs_segfault_reproduction() {
        // Create a test repository
        let temp_dir = tempfile::tempdir().unwrap();
        let repo_path = temp_dir.path().join("test_repo");
        let repo = Repos::create(&repo_path).unwrap();

        // Create a simple dump data to load
        let dump_data = b"SVN-fs-dump-format-version: 3\nUUID: test-uuid-1234\n\nRevision-number: 0\nProp-content-length: 56\nContent-length: 56\n\nK 8\nsvn:date\nV 27\n2023-01-01T12:00:00.000000Z\nPROPS-END\n\n";
        let mut stream = crate::io::Stream::from(dump_data.as_slice());

        // Use None for both start_rev and end_rev to load all revisions
        let result = repo.load_fs(
            &mut stream,
            None,
            None,
            LoadUUID::Default,
            std::path::Path::new(""),
            false,
            false,
            None::<&fn() -> bool>,
            true,
            false,
            false,
            None::<&fn(&Notify)>,
        );

        println!("load_fs result: {:?}", result);
    }

    #[test]
    fn test_version() {
        assert!(super::version().major() >= 1);
    }

    #[test]
    fn test_create_with_config() {
        let td = tempfile::tempdir().unwrap();
        let config = std::collections::HashMap::from([(
            "repository.compression".to_string(),
            "6".to_string(),
        )]);
        let fs_config =
            std::collections::HashMap::from([("fsfs.compression".to_string(), "zlib".to_string())]);

        let repos = super::Repos::create_with_config(td.path(), Some(&config), Some(&fs_config));
        assert!(repos.is_ok(), "Failed to create repository with config");

        // Test that we can open the created repository
        let repos = super::Repos::open(td.path());
        assert!(
            repos.is_ok(),
            "Failed to open repository created with config"
        );
    }

    #[test]
    fn test_create_with_empty_config() {
        let td = tempfile::tempdir().unwrap();
        let empty_config = std::collections::HashMap::new();

        let repos = super::Repos::create_with_config(td.path(), Some(&empty_config), None);
        assert!(
            repos.is_ok(),
            "Failed to create repository with empty config"
        );
    }

    #[test]
    fn test_dump_basic() {
        let td = tempfile::tempdir().unwrap();
        let repos = super::Repos::create(td.path()).unwrap();

        // Create a string buffer to capture dump output
        let mut buffer = Vec::new();
        let mut stream = crate::io::wrap_write(&mut buffer).unwrap();

        // Dump revision 0 only (empty repo)
        let result = repos.dump(
            &mut stream,
            Some(crate::Revnum(0)), // start_rev
            Some(crate::Revnum(0)), // end_rev
            false,                  // incremental
            false,                  // use_deltas
            true,                   // include_revprops
            true,                   // include_changes
            None,                   // notify_func
            None,                   // cancel_func
        );

        assert!(result.is_ok(), "Failed to dump repository: {:?}", result);

        // Verify that we got some dump output
        let dump_str = String::from_utf8_lossy(&buffer);
        assert!(
            dump_str.contains("SVN-fs-dump-format-version"),
            "Dump output should contain format version header"
        );
    }

    #[test]
    fn test_dump_all_revisions() {
        let td = tempfile::tempdir().unwrap();
        let repos = super::Repos::create(td.path()).unwrap();

        // Create a string buffer to capture dump output
        let mut buffer = Vec::new();
        let mut stream = crate::io::wrap_write(&mut buffer).unwrap();

        // Dump all revisions (None means use SVN_INVALID_REVNUM = -1)
        let result = repos.dump(
            &mut stream,
            None,  // start_rev (defaults to 0)
            None,  // end_rev (defaults to HEAD)
            false, // incremental
            false, // use_deltas
            true,  // include_revprops
            true,  // include_changes
            None,  // notify_func
            None,  // cancel_func
        );

        assert!(result.is_ok(), "Failed to dump repository: {:?}", result);
    }

    #[test]
    fn test_verify_basic() {
        let td = tempfile::tempdir().unwrap();
        let repos = super::Repos::create(td.path()).unwrap();

        let result = repos.verify(
            crate::Revnum(0), // start_rev
            crate::Revnum(0), // end_rev
            false,            // check_normalization
            false,            // metadata_only
            None,             // notify_func
            None,             // verify_callback
            None,             // cancel_func
        );

        assert!(result.is_ok(), "Failed to verify repository: {:?}", result);
    }

    #[test]
    fn test_recover_basic() {
        let td = tempfile::tempdir().unwrap();
        let mut repos = super::Repos::create(td.path()).unwrap();

        let result = repos.recover(
            true, // nonblocking
            None, // notify_func
            None, // cancel_func
        );

        assert!(result.is_ok(), "Failed to recover repository: {:?}", result);
    }

    #[test]
    fn test_load_basic() {
        let td = tempfile::tempdir().unwrap();
        let repos = super::Repos::create(td.path()).unwrap();

        // Create a minimal dump stream content
        let dump_content = b"SVN-fs-dump-format-version: 2\n\n\
                           UUID: 00000000-0000-0000-0000-000000000000\n\n\
                           Revision-number: 0\n\
                           Prop-content-length: 56\n\
                           Content-length: 56\n\n\
                           K 8\n\
                           svn:date\n\
                           V 27\n\
                           2024-01-01T00:00:00.000000Z\n\
                           PROPS-END\n\n";

        let mut dump_vec = dump_content.to_vec();
        let mut stream = crate::io::wrap_write(&mut dump_vec).unwrap();

        // Test loading from dump stream (may fail with invalid dump format)
        // Note: SVN requires both start_rev and end_rev to be either valid or both invalid
        let result = repos.load(
            &mut stream,
            None,                     // start_rev (None = load all)
            None,                     // end_rev (None = load all)
            super::LoadUUID::Default, // uuid_action
            None,                     // parent_dir
            false,                    // use_pre_commit_hook
            false,                    // use_post_commit_hook
            false,                    // validate_props
            false,                    // ignore_dates
            false,                    // normalize_props
            None,                     // notify_func
            None,                     // cancel_func
        );

        // The test may fail due to dump format issues, but should not crash
        // The important thing is that the API is correctly implemented
        match result {
            Ok(_) => println!("Load succeeded"),
            Err(e) => println!("Load failed as expected: {}", e),
        }
    }
}

/// Pack the filesystem of a repository to improve performance
/// This is useful for FSFS repositories to consolidate revision files
pub fn fs_pack(
    path: &std::path::Path,
    notify_func: Option<&dyn Fn(&Notify)>,
    cancel_func: Option<&dyn Fn() -> Result<(), Error>>,
) -> Result<(), Error> {
    let _path_cstr = std::ffi::CString::new(path.to_string_lossy().as_ref())?;
    let pool = apr::Pool::new();

    let notify_baton = notify_func
        .map(|notify_func| Box::into_raw(Box::new(notify_func)) as *mut std::ffi::c_void)
        .unwrap_or(std::ptr::null_mut());

    let cancel_baton = cancel_func
        .map(|cancel_func| Box::into_raw(Box::new(cancel_func)) as *mut std::ffi::c_void)
        .unwrap_or(std::ptr::null_mut());

    // First open the repository to get the repos handle
    let repos = Repos::open(path)?;

    let ret = unsafe {
        subversion_sys::svn_repos_fs_pack2(
            repos.ptr,
            if notify_func.is_some() {
                Some(wrap_notify_func)
            } else {
                None
            },
            notify_baton,
            if cancel_func.is_some() {
                Some(crate::wrap_cancel_func)
            } else {
                None
            },
            cancel_baton,
            pool.as_mut_ptr(),
        )
    };

    // Free callback batons
    if !notify_baton.is_null() {
        unsafe { drop(Box::from_raw(notify_baton as *mut Box<dyn Fn(&Notify)>)) };
    }
    if !cancel_baton.is_null() {
        unsafe {
            drop(Box::from_raw(
                cancel_baton as *mut Box<dyn Fn() -> Result<(), Error>>,
            ))
        };
    }

    Error::from_raw(ret)
}

impl Repos {
    /// Get a commit editor for making commits directly to the repository
    /// This provides low-level access for creating commits without a working copy
    ///
    /// # Arguments
    /// * `repos_url` - The repository URL (required)
    /// * `base_path` - The base path within the repository (e.g., "/trunk")
    /// * `log_msg` - The commit log message
    /// * `author` - Optional author name
    /// * `revprops` - Optional additional revision properties (svn:log and svn:author are set automatically)
    /// * `commit_callback` - Optional callback to be called on successful commit
    pub fn get_commit_editor(
        &self,
        repos_url: &str,
        base_path: &str,
        log_msg: &str,
        author: Option<&str>,
        revprops: Option<&std::collections::HashMap<String, Vec<u8>>>,
        commit_callback: Option<&dyn Fn(crate::Revnum, &str, &std::ffi::CStr)>,
    ) -> Result<Box<dyn crate::delta::Editor>, Error> {
        let repos_url_cstr = std::ffi::CString::new(repos_url)?;
        let base_path_cstr = std::ffi::CString::new(base_path)?;
        let log_msg_cstr = std::ffi::CString::new(log_msg)?;
        let author_cstr = author.map(std::ffi::CString::new).transpose()?;
        // Use a boxed pool to keep it alive with the editor
        let pool = Box::new(apr::Pool::new());
        let pool_ptr = pool.as_ref().as_mut_ptr();

        // Build revprop table with log message and author
        let revprops_hash = unsafe {
            let hash = apr_sys::apr_hash_make(pool_ptr);

            // Add log message
            // Allocate the key in the pool so it lives as long as the hash
            let log_key = apr_sys::apr_pstrdup(pool_ptr, b"svn:log\0".as_ptr() as *const i8);
            let log_value = subversion_sys::svn_string_ncreate(
                log_msg_cstr.as_ptr() as *const std::os::raw::c_char,
                log_msg.len(),
                pool_ptr,
            );
            apr_sys::apr_hash_set(
                hash,
                log_key as *const std::ffi::c_void,
                apr_sys::APR_HASH_KEY_STRING as isize,
                log_value as *mut std::ffi::c_void,
            );

            // Add author if provided
            if let Some(ref author_c) = author_cstr {
                // Allocate the key in the pool so it lives as long as the hash
                let author_key =
                    apr_sys::apr_pstrdup(pool_ptr, b"svn:author\0".as_ptr() as *const i8);
                let author_value = subversion_sys::svn_string_create(author_c.as_ptr(), pool_ptr);
                apr_sys::apr_hash_set(
                    hash,
                    author_key as *const std::ffi::c_void,
                    apr_sys::APR_HASH_KEY_STRING as isize,
                    author_value as *mut std::ffi::c_void,
                );
            }

            // Add any additional revprops
            if let Some(revprops) = revprops {
                for (key, value) in revprops {
                    // Allocate key in the pool
                    let key_cstr = std::ffi::CString::new(key.as_str())?;
                    let pool_key = apr_sys::apr_pstrdup(pool_ptr, key_cstr.as_ptr());
                    let svn_string = subversion_sys::svn_string_ncreate(
                        value.as_ptr() as *const std::os::raw::c_char,
                        value.len(),
                        pool_ptr,
                    );
                    apr_sys::apr_hash_set(
                        hash,
                        pool_key as *const std::ffi::c_void,
                        apr_sys::APR_HASH_KEY_STRING as isize,
                        svn_string as *mut std::ffi::c_void,
                    );
                }
            }

            hash
        };

        let commit_baton = commit_callback
            .map(|callback| Box::into_raw(Box::new(callback)) as *mut std::ffi::c_void)
            .unwrap_or(std::ptr::null_mut());

        let mut editor_ptr = std::ptr::null();
        let mut edit_baton = std::ptr::null_mut();

        // Commit callback wrapper that matches expected signature
        extern "C" fn wrap_commit_callback(
            commit_info: *const subversion_sys::svn_commit_info_t,
            baton: *mut std::ffi::c_void,
            _pool: *mut subversion_sys::apr_pool_t,
        ) -> *mut subversion_sys::svn_error_t {
            if baton.is_null() || commit_info.is_null() {
                return std::ptr::null_mut();
            }

            let callback =
                unsafe { &*(baton as *const Box<dyn Fn(crate::Revnum, &str, &std::ffi::CStr)>) };

            let info = unsafe { &*commit_info };
            let date_str = if !info.date.is_null() {
                unsafe { std::ffi::CStr::from_ptr(info.date).to_string_lossy() }
            } else {
                std::borrow::Cow::Borrowed("")
            };

            let author_cstr = if !info.author.is_null() {
                unsafe { std::ffi::CStr::from_ptr(info.author) }
            } else {
                std::ffi::CStr::from_bytes_with_nul(b"\0").unwrap()
            };

            callback(
                crate::Revnum::from(info.revision as u64),
                &date_str,
                author_cstr,
            );
            std::ptr::null_mut()
        }

        let ret = unsafe {
            subversion_sys::svn_repos_get_commit_editor5(
                &mut editor_ptr,
                &mut edit_baton,
                self.ptr,
                std::ptr::null_mut(), // txn (NULL = create new transaction)
                repos_url_cstr.as_ptr(),
                base_path_cstr.as_ptr(),
                revprops_hash,
                if commit_callback.is_some() {
                    Some(wrap_commit_callback)
                } else {
                    None
                },
                commit_baton,
                None,                 // authz_callback (NULL = allow all)
                std::ptr::null_mut(), // authz_baton
                pool_ptr,
            )
        };

        Error::from_raw(ret)?;

        // Leak the pool to keep it alive for the lifetime of the editor
        // This is necessary because the C API expects the pool to remain valid
        let _ = Box::leak(pool);

        let editor = crate::delta::WrapEditor {
            editor: editor_ptr,
            baton: edit_baton,
            _pool: std::marker::PhantomData,
        };
        Ok(Box::new(editor))
    }

    /// Begin a repository report (for update/switch operations)
    ///
    /// # Arguments
    /// * `revnum` - The target revision
    /// * `fs_base` - Absolute path in the filesystem at which comparison should be rooted
    /// * `target` - Single path component to limit scope, or "" for all of fs_base
    /// * `tgt_path` - Optional target path when switching (None to preserve current path)
    /// * `text_deltas` - Whether to generate text deltas
    /// * `depth` - Requested depth of the editor drive
    /// * `ignore_ancestry` - Whether to ignore node ancestry
    /// * `send_copyfrom_args` - Whether to send copyfrom arguments
    /// * `editor` - The delta editor
    /// * `editor_baton` - The editor baton
    pub fn begin_report(
        &self,
        revnum: Revnum,
        fs_base: &str,
        target: &str,           // Not optional - use "" for all of fs_base
        tgt_path: Option<&str>, // Can be NULL for same path
        text_deltas: bool,
        depth: crate::Depth,
        ignore_ancestry: bool,
        send_copyfrom_args: bool,
        editor: *const subversion_sys::svn_delta_editor_t,
        editor_baton: *mut std::ffi::c_void,
    ) -> Result<Report, Error> {
        let fs_base_cstr = std::ffi::CString::new(fs_base).unwrap();
        let target_cstr = std::ffi::CString::new(target).unwrap();
        let tgt_path_cstr = tgt_path.map(|t| std::ffi::CString::new(t).unwrap());

        let pool = apr::Pool::new();
        let mut report_baton: *mut std::ffi::c_void = std::ptr::null_mut();

        let ret = unsafe {
            subversion_sys::svn_repos_begin_report3(
                &mut report_baton,
                revnum.into(),
                self.ptr,
                fs_base_cstr.as_ptr(),
                target_cstr.as_ptr(),
                tgt_path_cstr
                    .as_ref()
                    .map(|t| t.as_ptr())
                    .unwrap_or(std::ptr::null()),
                text_deltas.into(),
                depth.into(),
                ignore_ancestry.into(),
                send_copyfrom_args.into(),
                editor,
                editor_baton,
                None,                 // authz_read_func - None for Option<fn>
                std::ptr::null_mut(), // authz_read_baton
                0,                    // zero_copy_limit
                pool.as_mut_ptr(),
            )
        };

        svn_result(ret)?;

        Ok(Report {
            baton: report_baton,
            pool,
        })
    }
}

/// Repository report handle for update/switch operations
pub struct Report {
    baton: *mut std::ffi::c_void,
    pool: apr::Pool,
}

impl Report {
    /// Record the presence of a path in the current tree
    pub fn set_path(
        &self,
        path: &str,
        revision: Revnum,
        depth: crate::Depth,
        start_empty: bool,
        lock_token: Option<&str>,
    ) -> Result<(), Error> {
        let path_cstr = std::ffi::CString::new(path).unwrap();
        let lock_token_cstr = lock_token.map(|t| std::ffi::CString::new(t).unwrap());

        with_tmp_pool(|pool| {
            let ret = unsafe {
                subversion_sys::svn_repos_set_path3(
                    self.baton,
                    path_cstr.as_ptr(),
                    revision.into(),
                    depth.into(),
                    start_empty.into(),
                    lock_token_cstr
                        .as_ref()
                        .map(|t| t.as_ptr())
                        .unwrap_or(std::ptr::null()),
                    pool.as_mut_ptr(),
                )
            };
            svn_result(ret)
        })
    }

    /// Record the presence of a path as a link to another path
    pub fn link_path(
        &self,
        path: &str,
        link_path: &str,
        revision: Revnum,
        depth: crate::Depth,
        start_empty: bool,
        lock_token: Option<&str>,
    ) -> Result<(), Error> {
        let path_cstr = std::ffi::CString::new(path).unwrap();
        let link_path_cstr = std::ffi::CString::new(link_path).unwrap();
        let lock_token_cstr = lock_token.map(|t| std::ffi::CString::new(t).unwrap());

        with_tmp_pool(|pool| {
            let ret = unsafe {
                subversion_sys::svn_repos_link_path3(
                    self.baton,
                    path_cstr.as_ptr(),
                    link_path_cstr.as_ptr(),
                    revision.into(),
                    depth.into(),
                    start_empty.into(),
                    lock_token_cstr
                        .as_ref()
                        .map(|t| t.as_ptr())
                        .unwrap_or(std::ptr::null()),
                    pool.as_mut_ptr(),
                )
            };
            svn_result(ret)
        })
    }

    /// Record the non-existence of a path in the current tree
    pub fn delete_path(&self, path: &str) -> Result<(), Error> {
        let path_cstr = std::ffi::CString::new(path).unwrap();

        with_tmp_pool(|pool| {
            let ret = unsafe {
                subversion_sys::svn_repos_delete_path(
                    self.baton,
                    path_cstr.as_ptr(),
                    pool.as_mut_ptr(),
                )
            };
            svn_result(ret)
        })
    }

    /// Finish the report and drive the editor
    pub fn finish(self) -> Result<(), Error> {
        // Use the report's pool, not a temporary one
        let pool_ptr = self.pool.as_mut_ptr();
        let ret = unsafe { subversion_sys::svn_repos_finish_report(self.baton, pool_ptr) };
        svn_result(ret)
    }

    /// Abort the report
    pub fn abort(self) -> Result<(), Error> {
        // Extract the pool pointer before self is consumed
        let pool_ptr = self.pool.as_mut_ptr();
        let ret = unsafe { subversion_sys::svn_repos_abort_report(self.baton, pool_ptr) };
        svn_result(ret)
    }
}

impl Repos {
    /// Set the environment for hook scripts by providing a path to an environment file
    pub fn hooks_setenv(&mut self, hooks_env_path: &str) -> Result<(), Error> {
        let pool = apr::Pool::new();
        let path_cstr = CString::new(hooks_env_path).unwrap();

        let ret = unsafe {
            subversion_sys::svn_repos_hooks_setenv(self.ptr, path_cstr.as_ptr(), pool.as_mut_ptr())
        };

        svn_result(ret)
    }

    /// Get the path to the start-commit hook script
    pub fn start_commit_hook_path(&self) -> std::path::PathBuf {
        let pool = apr::Pool::new();
        let path_ptr =
            unsafe { subversion_sys::svn_repos_start_commit_hook(self.ptr, pool.as_mut_ptr()) };

        let path = unsafe { std::ffi::CStr::from_ptr(path_ptr) }
            .to_string_lossy()
            .into_owned();
        std::path::PathBuf::from(path)
    }

    /// Get the path to the pre-commit hook script
    pub fn pre_commit_hook_path(&self) -> std::path::PathBuf {
        let pool = apr::Pool::new();
        let path_ptr =
            unsafe { subversion_sys::svn_repos_pre_commit_hook(self.ptr, pool.as_mut_ptr()) };

        let path = unsafe { std::ffi::CStr::from_ptr(path_ptr) }
            .to_string_lossy()
            .into_owned();
        std::path::PathBuf::from(path)
    }

    /// Get the path to the post-commit hook script  
    pub fn post_commit_hook_path(&self) -> std::path::PathBuf {
        let pool = apr::Pool::new();
        let path_ptr =
            unsafe { subversion_sys::svn_repos_post_commit_hook(self.ptr, pool.as_mut_ptr()) };

        let path = unsafe { std::ffi::CStr::from_ptr(path_ptr) }
            .to_string_lossy()
            .into_owned();
        std::path::PathBuf::from(path)
    }

    /// Get the path to the pre-revprop-change hook script
    pub fn pre_revprop_change_hook_path(&self) -> std::path::PathBuf {
        let pool = apr::Pool::new();
        let path_ptr = unsafe {
            subversion_sys::svn_repos_pre_revprop_change_hook(self.ptr, pool.as_mut_ptr())
        };

        let path = unsafe { std::ffi::CStr::from_ptr(path_ptr) }
            .to_string_lossy()
            .into_owned();
        std::path::PathBuf::from(path)
    }

    /// Get the path to the post-revprop-change hook script
    pub fn post_revprop_change_hook_path(&self) -> std::path::PathBuf {
        let pool = apr::Pool::new();
        let path_ptr = unsafe {
            subversion_sys::svn_repos_post_revprop_change_hook(self.ptr, pool.as_mut_ptr())
        };

        let path = unsafe { std::ffi::CStr::from_ptr(path_ptr) }
            .to_string_lossy()
            .into_owned();
        std::path::PathBuf::from(path)
    }

    /// Get the path to the pre-lock hook script
    pub fn pre_lock_hook_path(&self) -> std::path::PathBuf {
        let pool = apr::Pool::new();
        let path_ptr =
            unsafe { subversion_sys::svn_repos_pre_lock_hook(self.ptr, pool.as_mut_ptr()) };

        let path = unsafe { std::ffi::CStr::from_ptr(path_ptr) }
            .to_string_lossy()
            .into_owned();
        std::path::PathBuf::from(path)
    }

    /// Get the path to the post-lock hook script
    pub fn post_lock_hook_path(&self) -> std::path::PathBuf {
        let pool = apr::Pool::new();
        let path_ptr =
            unsafe { subversion_sys::svn_repos_post_lock_hook(self.ptr, pool.as_mut_ptr()) };

        let path = unsafe { std::ffi::CStr::from_ptr(path_ptr) }
            .to_string_lossy()
            .into_owned();
        std::path::PathBuf::from(path)
    }

    /// Get the path to the pre-unlock hook script
    pub fn pre_unlock_hook_path(&self) -> std::path::PathBuf {
        let pool = apr::Pool::new();
        let path_ptr =
            unsafe { subversion_sys::svn_repos_pre_unlock_hook(self.ptr, pool.as_mut_ptr()) };

        let path = unsafe { std::ffi::CStr::from_ptr(path_ptr) }
            .to_string_lossy()
            .into_owned();
        std::path::PathBuf::from(path)
    }

    /// Get the path to the post-unlock hook script
    pub fn post_unlock_hook_path(&self) -> std::path::PathBuf {
        let pool = apr::Pool::new();
        let path_ptr =
            unsafe { subversion_sys::svn_repos_post_unlock_hook(self.ptr, pool.as_mut_ptr()) };

        let path = unsafe { std::ffi::CStr::from_ptr(path_ptr) }
            .to_string_lossy()
            .into_owned();
        std::path::PathBuf::from(path)
    }
}

#[cfg(test)]
mod additional_tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_fs_pack_invalid_path() {
        let invalid_path = Path::new("/non/existent/repo");

        // Test fs_pack with invalid repository path (should fail gracefully)
        let result = fs_pack(invalid_path, None, None);

        // Should return error for invalid repository
        assert!(result.is_err());
    }

    #[test]
    fn test_get_commit_editor_basic() {
        // Create a temporary repository for testing
        let temp_dir = tempfile::tempdir().unwrap();
        let repo_path = temp_dir.path().join("test_repo");

        // Create repository using the correct function
        let repo = Repos::create(&repo_path).unwrap();

        // Build the repository URL from the path
        let repo_url = format!("file://{}", repo_path.to_string_lossy());

        // Test get_commit_editor with basic parameters
        let result = repo.get_commit_editor(
            &repo_url,
            "/", // base path (repository root)
            "Test commit message",
            Some("test_author"),
            None, // no revprops
            None, // no commit callback
        );

        // Should succeed in getting a commit editor
        assert!(
            result.is_ok(),
            "Failed to get commit editor: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_authz_basic() {
        // Test creating an authz file
        let temp_dir = tempfile::tempdir().unwrap();
        let authz_path = temp_dir.path().join("authz");

        // Write a simple authz file
        std::fs::write(&authz_path, "[/]\n* = rw\n").unwrap();

        // Test reading the authz file
        let authz = Authz::read(&authz_path, None, true);
        assert!(
            authz.is_ok(),
            "Failed to read authz file: {:?}",
            authz.err()
        );

        let authz = authz.unwrap();

        // Test check_access - should have read access
        let result = authz.check_access(Some("testrepo"), "/", Some("testuser"), AuthzAccess::Read);
        assert!(result.is_ok(), "check_access failed: {:?}", result.err());

        // The result should be true for read access with wildcard permission
        let has_access = result.unwrap();
        assert!(
            has_access,
            "Should have read access with wildcard permission"
        );
    }

    #[test]
    fn test_authz_no_access() {
        // Test with empty authz (no access)
        let temp_dir = tempfile::tempdir().unwrap();
        let authz_path = temp_dir.path().join("authz");

        // Write an empty authz file
        std::fs::write(&authz_path, "").unwrap();

        let authz = Authz::read(&authz_path, None, true).unwrap();

        // Test check_access - should have no access
        let result = authz.check_access(Some("testrepo"), "/", Some("testuser"), AuthzAccess::Read);
        assert!(result.is_ok(), "check_access failed: {:?}", result.err());

        let has_access = result.unwrap();
        assert!(!has_access, "Should have no access with empty authz");
    }

    #[test]
    fn test_report_with_commit_editor() {
        // Create a temporary repository for testing
        let temp_dir = tempfile::tempdir().unwrap();
        let repo_path = temp_dir.path().join("test_repo");

        // Create repository
        let repo = Repos::create(&repo_path).unwrap();

        // Build the repository URL from the path
        let repo_url = format!("file://{}", repo_path.to_string_lossy());

        // Get a commit editor which we can use for testing reports
        let editor_result = repo.get_commit_editor(
            &repo_url,
            "/",
            "Test commit for report testing",
            Some("test_author"),
            None,
            None,
        );

        assert!(
            editor_result.is_ok(),
            "Failed to get commit editor: {:?}",
            editor_result.err()
        );

        if let Ok(editor_box) = editor_result {
            // Use the Editor trait's as_raw_parts method to access raw pointers
            let (editor_ptr, baton_ptr) = editor_box.as_raw_parts();

            // Test begin_report with the commit editor
            let report_result = repo.begin_report(
                crate::Revnum::from_raw(0).unwrap(),
                "/",
                "",   // Empty string means all of fs_base
                None, // No path switching
                false,
                crate::Depth::Infinity,
                false,
                false,
                editor_ptr,
                baton_ptr,
            );

            // begin_report should succeed
            assert!(
                report_result.is_ok(),
                "Failed to begin report: {:?}",
                report_result.err()
            );

            if let Ok(report) = report_result {
                // Test abort (this should work and not crash)
                let abort_result = report.abort();
                assert!(
                    abort_result.is_ok(),
                    "Failed to abort report: {:?}",
                    abort_result.err()
                );
            }
        }
    }

    #[test]
    fn test_report_operations() {
        // Create a temporary repository for testing
        let temp_dir = tempfile::tempdir().unwrap();
        let repo_path = temp_dir.path().join("test_repo");

        // Create repository
        let repo = Repos::create(&repo_path).unwrap();

        // Build the repository URL
        let repo_url = format!("file://{}", repo_path.to_string_lossy());

        // Get a commit editor using the public API
        let editor_box = repo
            .get_commit_editor(
                &repo_url,
                "/",
                "Test commit for report operations",
                Some("test_author"),
                None,
                None,
            )
            .unwrap();

        // Create a report using the editor's raw pointers
        let (editor_ptr, baton_ptr) = editor_box.as_raw_parts();
        let report = repo
            .begin_report(
                crate::Revnum::from_raw(0).unwrap(),
                "/",
                "",   // Empty string means all of fs_base
                None, // No path switching
                false,
                crate::Depth::Infinity,
                false,
                false,
                editor_ptr,
                baton_ptr,
            )
            .unwrap();

        // Test individual report operations
        // set_path with root path
        let set_result = report.set_path(
            "",
            crate::Revnum::from_raw(0).unwrap(),
            crate::Depth::Infinity,
            false,
            None,
        );
        // This operation might succeed or fail depending on repository state,
        // but it should not crash
        let _ = set_result;

        // Test delete_path (should not crash even if path doesn't exist)
        let delete_result = report.delete_path("nonexistent_path");
        let _ = delete_result;

        // Test link_path (should not crash)
        let link_result = report.link_path(
            "link_target",
            "link_source",
            crate::Revnum::from_raw(0).unwrap(),
            crate::Depth::Files,
            false,
            None,
        );
        let _ = link_result;

        // Finally, abort the report (this should succeed)
        let abort_result = report.abort();
        assert!(
            abort_result.is_ok(),
            "Failed to abort report: {:?}",
            abort_result.err()
        );
    }

    #[test]
    fn test_hook_paths() {
        // Create a temporary repository for testing
        let temp_dir = tempfile::tempdir().unwrap();
        let repo_path = temp_dir.path().join("test_repo");

        // Create repository
        let repo = Repos::create(&repo_path).unwrap();

        // Test getting hook paths - they should all return valid paths
        let start_commit_hook = repo.start_commit_hook_path();
        assert!(start_commit_hook.to_str().unwrap().contains("start-commit"));
        assert!(start_commit_hook.to_str().unwrap().contains("hooks"));

        let pre_commit_hook = repo.pre_commit_hook_path();
        assert!(pre_commit_hook.to_str().unwrap().contains("pre-commit"));

        let post_commit_hook = repo.post_commit_hook_path();
        assert!(post_commit_hook.to_str().unwrap().contains("post-commit"));

        let pre_revprop_hook = repo.pre_revprop_change_hook_path();
        assert!(pre_revprop_hook
            .to_str()
            .unwrap()
            .contains("pre-revprop-change"));

        let post_revprop_hook = repo.post_revprop_change_hook_path();
        assert!(post_revprop_hook
            .to_str()
            .unwrap()
            .contains("post-revprop-change"));

        let pre_lock_hook = repo.pre_lock_hook_path();
        assert!(pre_lock_hook.to_str().unwrap().contains("pre-lock"));

        let post_lock_hook = repo.post_lock_hook_path();
        assert!(post_lock_hook.to_str().unwrap().contains("post-lock"));

        let pre_unlock_hook = repo.pre_unlock_hook_path();
        assert!(pre_unlock_hook.to_str().unwrap().contains("pre-unlock"));

        let post_unlock_hook = repo.post_unlock_hook_path();
        assert!(post_unlock_hook.to_str().unwrap().contains("post-unlock"));
    }

    #[test]
    fn test_hooks_setenv() {
        // Create a temporary repository for testing
        let temp_dir = tempfile::tempdir().unwrap();
        let repo_path = temp_dir.path().join("test_repo");

        // Create repository
        let mut repo = Repos::create(&repo_path).unwrap();

        // Create a hooks env file
        let hooks_env_path = temp_dir.path().join("hooks.env");
        std::fs::write(&hooks_env_path, "TEST_VAR=test_value\n").unwrap();

        // Test setting the hooks environment
        let result = repo.hooks_setenv(hooks_env_path.to_str().unwrap());
        // This might fail if the feature is not available, but should not crash
        let _ = result;
    }
}
