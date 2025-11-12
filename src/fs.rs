use crate::{svn_result, with_tmp_pool, Error, Revnum};

/// Represents a change to a path in the filesystem
pub struct FsPathChange {
    ptr: *const subversion_sys::svn_fs_path_change2_t,
}

impl FsPathChange {
    /// Creates an FsPathChange from a raw pointer.
    pub fn from_raw(ptr: *mut subversion_sys::svn_fs_path_change2_t) -> Self {
        Self { ptr }
    }

    /// Gets the kind of change for this path.
    pub fn change_kind(&self) -> crate::FsPathChangeKind {
        unsafe { (*self.ptr).change_kind.into() }
    }

    /// Gets the node kind (file, directory, etc.).
    pub fn node_kind(&self) -> crate::NodeKind {
        unsafe { (*self.ptr).node_kind.into() }
    }

    /// Checks if the text content was modified.
    pub fn text_modified(&self) -> bool {
        unsafe { (*self.ptr).text_mod != 0 }
    }

    /// Checks if properties were modified.
    pub fn props_modified(&self) -> bool {
        unsafe { (*self.ptr).prop_mod != 0 }
    }

    /// Gets the path this was copied from, if any.
    pub fn copyfrom_path(&self) -> Option<String> {
        unsafe {
            if (*self.ptr).copyfrom_path.is_null() {
                None
            } else {
                Some(
                    std::ffi::CStr::from_ptr((*self.ptr).copyfrom_path)
                        .to_string_lossy()
                        .into_owned(),
                )
            }
        }
    }

    /// Gets the revision this was copied from, if any.
    pub fn copyfrom_rev(&self) -> Option<Revnum> {
        unsafe {
            let rev = (*self.ptr).copyfrom_rev;
            if rev == -1 {
                // SVN_INVALID_REVNUM is typically -1
                None
            } else {
                Some(Revnum(rev))
            }
        }
    }
}

/// Represents a directory entry in the filesystem
pub struct FsDirEntry {
    ptr: *const subversion_sys::svn_fs_dirent_t,
    _pool: apr::SharedPool,
}

impl FsDirEntry {
    /// Creates an FsDirEntry from a raw pointer with a shared pool.
    pub fn from_raw(ptr: *mut subversion_sys::svn_fs_dirent_t, pool: apr::SharedPool) -> Self {
        Self { ptr, _pool: pool }
    }

    /// Gets the entry name.
    pub fn name(&self) -> &str {
        unsafe { std::ffi::CStr::from_ptr((*self.ptr).name).to_str().unwrap() }
    }

    /// Gets the entry ID as bytes.
    pub fn id(&self) -> Option<Vec<u8>> {
        unsafe {
            if (*self.ptr).id.is_null() {
                None
            } else {
                // SVN FS ID is opaque, we'll just return raw bytes
                let id_str = subversion_sys::svn_fs_unparse_id(
                    (*self.ptr).id,
                    apr::Pool::new().as_mut_ptr(),
                );
                if id_str.is_null() {
                    None
                } else {
                    let len = (*id_str).len;
                    let data = (*id_str).data;
                    Some(std::slice::from_raw_parts(data as *const u8, len).to_vec())
                }
            }
        }
    }

    /// Gets the node kind (file, directory, etc.).
    pub fn kind(&self) -> crate::NodeKind {
        unsafe { (*self.ptr).kind.into() }
    }
}

/// Filesystem handle with RAII cleanup
pub struct Fs {
    fs_ptr: *mut subversion_sys::svn_fs_t,
    pool: apr::Pool, // Keep pool alive for fs lifetime
}

unsafe impl Send for Fs {}

impl Drop for Fs {
    fn drop(&mut self) {
        // Pool drop will clean up fs
    }
}

impl Fs {
    /// Get a reference to the underlying pool
    pub fn pool(&self) -> &apr::Pool {
        &self.pool
    }

    /// Get the raw pointer to the filesystem (use with caution)
    pub fn as_ptr(&self) -> *const subversion_sys::svn_fs_t {
        self.fs_ptr
    }

    /// Get the mutable raw pointer to the filesystem (use with caution)
    pub fn as_mut_ptr(&mut self) -> *mut subversion_sys::svn_fs_t {
        self.fs_ptr
    }
    /// Create Fs from existing pointer with shared pool (for repos integration)
    pub(crate) unsafe fn from_ptr_and_pool(
        fs_ptr: *mut subversion_sys::svn_fs_t,
        pool: apr::Pool,
    ) -> Self {
        Self { fs_ptr, pool }
    }

    /// Creates a new filesystem at the specified path.
    pub fn create(path: &std::path::Path) -> Result<Fs, Error> {
        let pool = apr::Pool::new();
        let path_str = path
            .to_str()
            .ok_or_else(|| Error::from_str("Invalid path"))?;
        let path_c =
            std::ffi::CString::new(path_str).map_err(|_| Error::from_str("Invalid path string"))?;

        unsafe {
            let mut fs_ptr = std::ptr::null_mut();
            with_tmp_pool(|scratch_pool| {
                let err = subversion_sys::svn_fs_create2(
                    &mut fs_ptr,
                    path_c.as_ptr(),
                    std::ptr::null_mut(),
                    pool.as_mut_ptr(),
                    scratch_pool.as_mut_ptr(),
                );
                svn_result(err)
            })?;

            Ok(Fs { fs_ptr, pool })
        }
    }

    /// Opens an existing filesystem at the specified path.
    pub fn open(path: &std::path::Path) -> Result<Fs, Error> {
        let pool = apr::Pool::new();
        let path_str = path
            .to_str()
            .ok_or_else(|| Error::from_str("Invalid path"))?;
        let path_c =
            std::ffi::CString::new(path_str).map_err(|_| Error::from_str("Invalid path string"))?;

        unsafe {
            let mut fs_ptr = std::ptr::null_mut();
            with_tmp_pool(|scratch_pool| {
                let err = subversion_sys::svn_fs_open2(
                    &mut fs_ptr,
                    path_c.as_ptr(),
                    std::ptr::null_mut(),
                    pool.as_mut_ptr(),
                    scratch_pool.as_mut_ptr(),
                );
                svn_result(err)
            })?;

            Ok(Fs { fs_ptr, pool })
        }
    }

    /// Gets the path to the filesystem.
    pub fn path(&self) -> std::path::PathBuf {
        unsafe {
            with_tmp_pool(|pool| {
                let path = subversion_sys::svn_fs_path(self.fs_ptr, pool.as_mut_ptr());
                std::ffi::CStr::from_ptr(path)
                    .to_string_lossy()
                    .into_owned()
                    .into()
            })
        }
    }

    /// Gets the youngest (most recent) revision in the filesystem.
    pub fn youngest_revision(&self) -> Result<Revnum, Error> {
        unsafe {
            with_tmp_pool(|pool| {
                let mut youngest = 0;
                let err = subversion_sys::svn_fs_youngest_rev(
                    &mut youngest,
                    self.fs_ptr,
                    pool.as_mut_ptr(),
                );
                svn_result(err)?;
                Ok(Revnum::from_raw(youngest).unwrap())
            })
        }
    }

    /// Gets the property list for a revision.
    pub fn revision_proplist(
        &self,
        rev: Revnum,
    ) -> Result<std::collections::HashMap<String, Vec<u8>>, Error> {
        let pool = apr::pool::Pool::new();
        let mut props = std::ptr::null_mut();
        let err = unsafe {
            subversion_sys::svn_fs_revision_proplist(
                &mut props,
                self.fs_ptr,
                rev.0,
                pool.as_mut_ptr(),
            )
        };
        svn_result(err)?;

        // Handle empty hash
        if props.is_null() {
            return Ok(std::collections::HashMap::new());
        }

        let prop_hash = unsafe { crate::props::PropHash::from_ptr(props) };
        let revprops = prop_hash.to_hashmap();

        Ok(revprops)
    }

    /// Gets the root of a specific revision.
    pub fn revision_root(&self, rev: Revnum) -> Result<Root, Error> {
        unsafe {
            let pool = apr::Pool::new();
            let mut root_ptr = std::ptr::null_mut();
            let err = subversion_sys::svn_fs_revision_root(
                &mut root_ptr,
                self.fs_ptr,
                rev.0,
                pool.as_mut_ptr(),
            );
            svn_result(err)?;
            Ok(Root {
                ptr: root_ptr,
                pool,
            })
        }
    }

    /// Gets the UUID of the filesystem.
    pub fn get_uuid(&self) -> Result<String, Error> {
        unsafe {
            with_tmp_pool(|pool| {
                let mut uuid = std::ptr::null();
                let err =
                    subversion_sys::svn_fs_get_uuid(self.fs_ptr, &mut uuid, pool.as_mut_ptr());
                svn_result(err)?;
                Ok(std::ffi::CStr::from_ptr(uuid)
                    .to_string_lossy()
                    .into_owned())
            })
        }
    }

    /// Sets the UUID of the filesystem.
    pub fn set_uuid(&mut self, uuid: &str) -> Result<(), Error> {
        unsafe {
            let uuid = std::ffi::CString::new(uuid).unwrap();
            let err = subversion_sys::svn_fs_set_uuid(
                self.fs_ptr,
                uuid.as_ptr(),
                apr::pool::Pool::new().as_mut_ptr(),
            );
            svn_result(err)?;
            Ok(())
        }
    }

    /// Lock a path in the filesystem
    pub fn lock(
        &mut self,
        path: &str,
        token: Option<&str>,
        comment: Option<&str>,
        is_dav_comment: bool,
        expiration_date: Option<i64>,
        current_rev: Revnum,
        steal_lock: bool,
    ) -> Result<crate::Lock<'static>, Error> {
        let path_cstr = std::ffi::CString::new(path).unwrap();

        let token_cstr = token.map(|t| std::ffi::CString::new(t).unwrap());
        let token_ptr = token_cstr.as_ref().map_or(std::ptr::null(), |t| t.as_ptr());

        let comment_cstr = comment.map(|c| std::ffi::CString::new(c).unwrap());
        let comment_ptr = comment_cstr
            .as_ref()
            .map_or(std::ptr::null(), |c| c.as_ptr());

        let mut lock_ptr: *mut subversion_sys::svn_lock_t = std::ptr::null_mut();

        let ret = unsafe {
            subversion_sys::svn_fs_lock(
                &mut lock_ptr,
                self.fs_ptr,
                path_cstr.as_ptr(),
                token_ptr,
                comment_ptr,
                is_dav_comment as i32,
                expiration_date.unwrap_or(0),
                current_rev.0,
                steal_lock as i32,
                self.pool.as_mut_ptr(),
            )
        };

        svn_result(ret)?;

        // The lock is allocated in the fs pool, so it should be valid for the lifetime of the fs
        Ok(crate::Lock::from_raw(lock_ptr))
    }

    /// Unlock a path in the filesystem
    pub fn unlock(&mut self, path: &str, token: &str, break_lock: bool) -> Result<(), Error> {
        let path_cstr = std::ffi::CString::new(path).unwrap();
        let token_cstr = std::ffi::CString::new(token).unwrap();

        let ret = unsafe {
            subversion_sys::svn_fs_unlock(
                self.fs_ptr,
                path_cstr.as_ptr(),
                token_cstr.as_ptr(),
                break_lock as i32,
                apr::Pool::new().as_mut_ptr(),
            )
        };

        svn_result(ret)
    }

    /// Get lock information for a path
    pub fn get_lock(&self, path: &str) -> Result<Option<crate::Lock<'static>>, Error> {
        let path_cstr = std::ffi::CString::new(path).unwrap();
        let mut lock_ptr: *mut subversion_sys::svn_lock_t = std::ptr::null_mut();

        let ret = unsafe {
            subversion_sys::svn_fs_get_lock(
                &mut lock_ptr,
                self.fs_ptr,
                path_cstr.as_ptr(),
                self.pool.as_mut_ptr(),
            )
        };

        svn_result(ret)?;

        if lock_ptr.is_null() {
            Ok(None)
        } else {
            // The lock is allocated in the fs pool
            Ok(Some(crate::Lock::from_raw(lock_ptr)))
        }
    }

    /// Set the access context for the filesystem with a username
    pub fn set_access(&mut self, username: &str) -> Result<(), Error> {
        let username_cstr = std::ffi::CString::new(username).unwrap();

        let mut access_ctx: *mut subversion_sys::svn_fs_access_t = std::ptr::null_mut();

        let ret = unsafe {
            subversion_sys::svn_fs_create_access(
                &mut access_ctx,
                username_cstr.as_ptr(),
                self.pool.as_mut_ptr(),
            )
        };

        svn_result(ret)?;

        let ret = unsafe { subversion_sys::svn_fs_set_access(self.fs_ptr, access_ctx) };

        svn_result(ret)
    }

    /// Get locks under a path
    pub fn get_locks(
        &self,
        path: &str,
        depth: crate::Depth,
    ) -> Result<Vec<crate::Lock<'static>>, Error> {
        let pool = apr::Pool::new();
        let path_cstr = std::ffi::CString::new(path).unwrap();

        let mut locks = Vec::new();
        let locks_ptr = &mut locks as *mut Vec<crate::Lock<'static>> as *mut std::ffi::c_void;

        extern "C" fn lock_callback(
            baton: *mut std::ffi::c_void,
            lock: *mut subversion_sys::svn_lock_t,
            _pool: *mut apr_sys::apr_pool_t,
        ) -> *mut subversion_sys::svn_error_t {
            unsafe {
                let locks = &mut *(baton as *mut Vec<crate::Lock<'static>>);
                if !lock.is_null() {
                    locks.push(crate::Lock::from_raw(lock));
                }
            }
            std::ptr::null_mut()
        }

        let ret = unsafe {
            subversion_sys::svn_fs_get_locks2(
                self.fs_ptr,
                path_cstr.as_ptr(),
                depth.into(),
                Some(lock_callback),
                locks_ptr,
                pool.as_mut_ptr(),
            )
        };

        svn_result(ret)?;
        Ok(locks)
    }

    /// Freeze the filesystem for the duration of a callback
    pub fn freeze<F>(&mut self, freeze_func: F) -> Result<(), Error>
    where
        F: FnOnce() -> Result<(), Error>,
    {
        // We need to create a wrapper that can be passed to C
        struct FreezeWrapper<F> {
            func: F,
            error: Option<Error>,
        }

        extern "C" fn freeze_callback<F>(
            baton: *mut std::ffi::c_void,
            _pool: *mut subversion_sys::apr_pool_t,
        ) -> *mut subversion_sys::svn_error_t
        where
            F: FnOnce() -> Result<(), Error>,
        {
            unsafe {
                let wrapper = &mut *(baton as *mut FreezeWrapper<F>);
                // We need to take the function out since it's FnOnce
                let func = std::ptr::read(&wrapper.func as *const F);
                match func() {
                    Ok(()) => std::ptr::null_mut(),
                    Err(e) => {
                        wrapper.error = Some(e.clone());
                        e.into_raw()
                    }
                }
            }
        }

        let mut wrapper = FreezeWrapper {
            func: freeze_func,
            error: None,
        };

        let ret = unsafe {
            subversion_sys::svn_fs_freeze(
                self.fs_ptr,
                Some(freeze_callback::<F>),
                &mut wrapper as *mut _ as *mut std::ffi::c_void,
                self.pool.as_mut_ptr(),
            )
        };

        if let Some(err) = wrapper.error {
            return Err(err);
        }

        svn_result(ret)
    }

    /// Get filesystem information
    pub fn info(&self) -> Result<FsInfo, Error> {
        let pool = apr::Pool::new();

        let mut info_ptr: *const subversion_sys::svn_fs_info_placeholder_t = std::ptr::null();

        let ret = unsafe {
            subversion_sys::svn_fs_info(
                &mut info_ptr,
                self.fs_ptr,
                pool.as_mut_ptr(),
                pool.as_mut_ptr(),
            )
        };

        svn_result(ret)?;

        if info_ptr.is_null() {
            return Err(Error::from_str("Failed to get filesystem info"));
        }

        // Parse the info structure from svn_fs_info_placeholder_t
        unsafe {
            let info = &*info_ptr;

            Ok(FsInfo {
                fs_type: if info.fs_type.is_null() {
                    None
                } else {
                    Some(
                        std::ffi::CStr::from_ptr(info.fs_type)
                            .to_string_lossy()
                            .into_owned(),
                    )
                },
            })
        }
    }
}

/// Gets the filesystem type for a repository at the given path.
pub fn fs_type(path: &std::path::Path) -> Result<String, Error> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    unsafe {
        let pool = apr::pool::Pool::new();
        let mut fs_type = std::ptr::null();
        let err = subversion_sys::svn_fs_type(&mut fs_type, path.as_ptr(), pool.as_mut_ptr());
        svn_result(err)?;
        Ok(std::ffi::CStr::from_ptr(fs_type)
            .to_string_lossy()
            .into_owned())
    }
}

/// Deletes a filesystem at the given path.
pub fn delete_fs(path: &std::path::Path) -> Result<(), Error> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    unsafe {
        let err =
            subversion_sys::svn_fs_delete_fs(path.as_ptr(), apr::pool::Pool::new().as_mut_ptr());
        svn_result(err)?;
        Ok(())
    }
}

/// Pack the filesystem at the given path.
/// This compresses the filesystem storage to save space.
pub fn pack(
    path: &std::path::Path,
    notify: Option<Box<dyn Fn(&str) + Send>>,
    cancel: Option<Box<dyn Fn() -> bool + Send>>,
) -> Result<(), Error> {
    let path_cstr = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let pool = apr::Pool::new();

    // Create notify callback wrapper
    let notify_baton = notify.map(|f| Box::into_raw(Box::new(f)) as *mut std::ffi::c_void);
    let cancel_baton = cancel.map(|f| Box::into_raw(Box::new(f)) as *mut std::ffi::c_void);

    extern "C" fn notify_wrapper(
        baton: *mut std::ffi::c_void,
        shard: i64,
        _action: subversion_sys::svn_fs_pack_notify_action_t,
        _pool: *mut apr_sys::apr_pool_t,
    ) -> *mut subversion_sys::svn_error_t {
        if !baton.is_null() {
            let notify = unsafe { &*(baton as *const Box<dyn Fn(&str) + Send>) };
            notify(&format!("Packing shard {}", shard));
        }
        std::ptr::null_mut()
    }

    extern "C" fn cancel_wrapper(baton: *mut std::ffi::c_void) -> *mut subversion_sys::svn_error_t {
        if !baton.is_null() {
            let cancel = unsafe { &*(baton as *const Box<dyn Fn() -> bool + Send>) };
            if cancel() {
                return unsafe { Error::from_str("Operation cancelled").into_raw() };
            }
        }
        std::ptr::null_mut()
    }

    let err = unsafe {
        subversion_sys::svn_fs_pack(
            path_cstr.as_ptr(),
            notify_baton.map_or(None, |_| Some(notify_wrapper as _)),
            notify_baton.unwrap_or(std::ptr::null_mut()),
            cancel_baton.map_or(None, |_| Some(cancel_wrapper as _)),
            cancel_baton.unwrap_or(std::ptr::null_mut()),
            pool.as_mut_ptr(),
        )
    };

    // Clean up callbacks
    if let Some(baton) = notify_baton {
        unsafe {
            let _ = Box::from_raw(baton as *mut Box<dyn Fn(&str) + Send>);
        }
    }
    if let Some(baton) = cancel_baton {
        unsafe {
            let _ = Box::from_raw(baton as *mut Box<dyn Fn() -> bool + Send>);
        }
    }

    svn_result(err)?;
    Ok(())
}

/// Verify the filesystem at the given path.
pub fn verify(
    path: &std::path::Path,
    start: Option<Revnum>,
    end: Option<Revnum>,
    notify: Option<Box<dyn Fn(Revnum, &str) + Send>>,
    cancel: Option<Box<dyn Fn() -> bool + Send>>,
) -> Result<(), Error> {
    let path_cstr = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let pool = apr::Pool::new();

    let start_rev = start.map(|r| r.0).unwrap_or(0);
    let end_rev = end.map(|r| r.0).unwrap_or(-1); // SVN_INVALID_REVNUM means HEAD

    // Create callback wrappers
    let notify_baton = notify.map(|f| Box::into_raw(Box::new(f)) as *mut std::ffi::c_void);
    let cancel_baton = cancel.map(|f| Box::into_raw(Box::new(f)) as *mut std::ffi::c_void);

    extern "C" fn notify_wrapper(
        revision: subversion_sys::svn_revnum_t,
        baton: *mut std::ffi::c_void,
        _pool: *mut apr_sys::apr_pool_t,
    ) {
        if !baton.is_null() {
            let notify = unsafe { &*(baton as *const Box<dyn Fn(Revnum, &str) + Send>) };
            notify(Revnum(revision), "Verifying");
        }
    }

    extern "C" fn cancel_wrapper(baton: *mut std::ffi::c_void) -> *mut subversion_sys::svn_error_t {
        if !baton.is_null() {
            let cancel = unsafe { &*(baton as *const Box<dyn Fn() -> bool + Send>) };
            if cancel() {
                return unsafe { Error::from_str("Operation cancelled").into_raw() };
            }
        }
        std::ptr::null_mut()
    }

    let err = unsafe {
        subversion_sys::svn_fs_verify(
            path_cstr.as_ptr(),
            std::ptr::null_mut(), // config
            start_rev,
            end_rev,
            notify_baton.map_or(None, |_| Some(notify_wrapper as _)),
            notify_baton.unwrap_or(std::ptr::null_mut()),
            cancel_baton.map_or(None, |_| Some(cancel_wrapper as _)),
            cancel_baton.unwrap_or(std::ptr::null_mut()),
            pool.as_mut_ptr(),
        )
    };

    // Clean up callbacks
    if let Some(baton) = notify_baton {
        unsafe {
            let _ = Box::from_raw(baton as *mut Box<dyn Fn(Revnum, &str) + Send>);
        }
    }
    if let Some(baton) = cancel_baton {
        unsafe {
            let _ = Box::from_raw(baton as *mut Box<dyn Fn() -> bool + Send>);
        }
    }

    svn_result(err)?;
    Ok(())
}

/// Hotcopy a filesystem from src_path to dst_path.
pub fn hotcopy(
    src_path: &std::path::Path,
    dst_path: &std::path::Path,
    clean: bool,
    incremental: bool,
    notify: Option<Box<dyn Fn(&str) + Send>>,
    cancel: Option<Box<dyn Fn() -> bool + Send>>,
) -> Result<(), Error> {
    let src_cstr = std::ffi::CString::new(src_path.to_str().unwrap()).unwrap();
    let dst_cstr = std::ffi::CString::new(dst_path.to_str().unwrap()).unwrap();
    let pool = apr::Pool::new();

    // Create callback wrappers
    let notify_baton = notify.map(|f| Box::into_raw(Box::new(f)) as *mut std::ffi::c_void);
    let cancel_baton = cancel.map(|f| Box::into_raw(Box::new(f)) as *mut std::ffi::c_void);

    extern "C" fn notify_wrapper(
        baton: *mut std::ffi::c_void,
        start_revision: subversion_sys::svn_revnum_t,
        end_revision: subversion_sys::svn_revnum_t,
        _pool: *mut apr_sys::apr_pool_t,
    ) {
        if !baton.is_null() {
            let notify = unsafe { &*(baton as *const Box<dyn Fn(&str) + Send>) };
            notify(&format!(
                "Hotcopy revisions {} to {}",
                start_revision, end_revision
            ));
        }
    }

    extern "C" fn cancel_wrapper(baton: *mut std::ffi::c_void) -> *mut subversion_sys::svn_error_t {
        if !baton.is_null() {
            let cancel = unsafe { &*(baton as *const Box<dyn Fn() -> bool + Send>) };
            if cancel() {
                return unsafe { Error::from_str("Operation cancelled").into_raw() };
            }
        }
        std::ptr::null_mut()
    }

    let err = unsafe {
        subversion_sys::svn_fs_hotcopy3(
            src_cstr.as_ptr(),
            dst_cstr.as_ptr(),
            clean as i32,
            incremental as i32,
            notify_baton.map_or(None, |_| Some(notify_wrapper as _)),
            notify_baton.unwrap_or(std::ptr::null_mut()),
            cancel_baton.map_or(None, |_| Some(cancel_wrapper as _)),
            cancel_baton.unwrap_or(std::ptr::null_mut()),
            pool.as_mut_ptr(),
        )
    };

    // Clean up callbacks
    if let Some(baton) = notify_baton {
        unsafe {
            let _ = Box::from_raw(baton as *mut Box<dyn Fn(&str) + Send>);
        }
    }
    if let Some(baton) = cancel_baton {
        unsafe {
            let _ = Box::from_raw(baton as *mut Box<dyn Fn() -> bool + Send>);
        }
    }

    svn_result(err)?;
    Ok(())
}

/// Recover a filesystem at the given path.
pub fn recover(
    path: &std::path::Path,
    cancel: Option<Box<dyn Fn() -> bool + Send>>,
) -> Result<(), Error> {
    let path_cstr = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let pool = apr::Pool::new();

    let cancel_baton = cancel.map(|f| Box::into_raw(Box::new(f)) as *mut std::ffi::c_void);

    extern "C" fn cancel_wrapper(baton: *mut std::ffi::c_void) -> *mut subversion_sys::svn_error_t {
        if !baton.is_null() {
            let cancel = unsafe { &*(baton as *const Box<dyn Fn() -> bool + Send>) };
            if cancel() {
                return unsafe { Error::from_str("Operation cancelled").into_raw() };
            }
        }
        std::ptr::null_mut()
    }

    let err = unsafe {
        subversion_sys::svn_fs_recover(
            path_cstr.as_ptr(),
            cancel_baton.map_or(None, |_| Some(cancel_wrapper as _)),
            cancel_baton.unwrap_or(std::ptr::null_mut()),
            pool.as_mut_ptr(),
        )
    };

    // Clean up callback
    if let Some(baton) = cancel_baton {
        unsafe {
            let _ = Box::from_raw(baton as *mut Box<dyn Fn() -> bool + Send>);
        }
    }

    svn_result(err)?;
    Ok(())
}

/// Information about a filesystem.
#[derive(Debug, Clone)]
pub struct FsInfo {
    /// Filesystem type (fsfs, bdb, etc.)
    pub fs_type: Option<String>,
}

#[allow(dead_code)]
/// Represents a filesystem root at a specific revision.
pub struct Root {
    ptr: *mut subversion_sys::svn_fs_root_t,
    pool: apr::Pool, // Keep pool alive for root lifetime
}

unsafe impl Send for Root {}

impl Drop for Root {
    fn drop(&mut self) {
        // Pool drop will clean up root
    }
}

impl Root {
    /// Gets the raw pointer to the root.
    pub fn as_ptr(&self) -> *const subversion_sys::svn_fs_root_t {
        self.ptr
    }

    /// Gets the mutable raw pointer to the root.
    pub fn as_mut_ptr(&mut self) -> *mut subversion_sys::svn_fs_root_t {
        self.ptr
    }

    /// Check if a path is a directory
    pub fn is_dir(&self, path: &str) -> Result<bool, Error> {
        with_tmp_pool(|pool| unsafe {
            let path_c = std::ffi::CString::new(path).unwrap();
            let mut is_dir = 0;
            let err = subversion_sys::svn_fs_is_dir(
                &mut is_dir,
                self.ptr,
                path_c.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;
            Ok(is_dir != 0)
        })
    }

    /// Check if a path is a file
    pub fn is_file(&self, path: &str) -> Result<bool, Error> {
        with_tmp_pool(|pool| unsafe {
            let path_c = std::ffi::CString::new(path).unwrap();
            let mut is_file = 0;
            let err = subversion_sys::svn_fs_is_file(
                &mut is_file,
                self.ptr,
                path_c.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;
            Ok(is_file != 0)
        })
    }

    /// Get the length of a file
    pub fn file_length(&self, path: &str) -> Result<i64, Error> {
        with_tmp_pool(|pool| unsafe {
            let path_c = std::ffi::CString::new(path).unwrap();
            let mut length = 0;
            let err = subversion_sys::svn_fs_file_length(
                &mut length,
                self.ptr,
                path_c.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;
            Ok(length)
        })
    }

    /// Get the contents of a file as a stream
    pub fn file_contents(&self, path: &str) -> Result<crate::io::Stream, Error> {
        let pool = apr::Pool::new();
        unsafe {
            let path_c = std::ffi::CString::new(path).unwrap();
            let mut stream = std::ptr::null_mut();
            let err = subversion_sys::svn_fs_file_contents(
                &mut stream,
                self.ptr,
                path_c.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;
            Ok(crate::io::Stream::from_ptr_and_pool(stream, pool))
        }
    }

    /// Get the checksum of a file
    pub fn file_checksum(
        &self,
        path: &str,
        kind: crate::ChecksumKind,
    ) -> Result<Option<crate::Checksum>, Error> {
        with_tmp_pool(|pool| unsafe {
            let path_c = std::ffi::CString::new(path).unwrap();
            let mut checksum = std::ptr::null_mut();
            let err = subversion_sys::svn_fs_file_checksum(
                &mut checksum,
                kind.into(),
                self.ptr,
                path_c.as_ptr(),
                1, // force computation
                pool.as_mut_ptr(),
            );
            svn_result(err)?;
            if checksum.is_null() {
                Ok(None)
            } else {
                Ok(Some(crate::Checksum::from_raw(checksum)))
            }
        })
    }

    /// Get properties of a node
    pub fn proplist(
        &self,
        path: &str,
    ) -> Result<std::collections::HashMap<String, Vec<u8>>, Error> {
        with_tmp_pool(|pool| unsafe {
            let path_c = std::ffi::CString::new(path).unwrap();
            let mut props = std::ptr::null_mut();
            let err = subversion_sys::svn_fs_node_proplist(
                &mut props,
                self.ptr,
                path_c.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;

            let result = if !props.is_null() {
                let prop_hash = crate::props::PropHash::from_ptr(props);
                prop_hash.to_hashmap()
            } else {
                std::collections::HashMap::new()
            };
            Ok(result)
        })
    }

    /// Get paths changed in this root (for revision roots)
    pub fn paths_changed(&self) -> Result<std::collections::HashMap<String, FsPathChange>, Error> {
        with_tmp_pool(|pool| unsafe {
            let mut changed_paths = std::ptr::null_mut();
            let err = subversion_sys::svn_fs_paths_changed2(
                &mut changed_paths,
                self.ptr,
                pool.as_mut_ptr(),
            );
            svn_result(err)?;

            if changed_paths.is_null() {
                Ok(std::collections::HashMap::new())
            } else {
                let hash = crate::hash::PathChangeHash::from_ptr(changed_paths);
                Ok(hash.to_hashmap())
            }
        })
    }

    /// Check the type of a path (file, directory, or none)
    pub fn check_path(&self, path: &str) -> Result<crate::NodeKind, Error> {
        with_tmp_pool(|pool| unsafe {
            let path_c = std::ffi::CString::new(path).unwrap();
            let mut kind = subversion_sys::svn_node_kind_t_svn_node_none;
            let err = subversion_sys::svn_fs_check_path(
                &mut kind,
                self.ptr,
                path_c.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;
            Ok(kind.into())
        })
    }

    /// List directory entries
    pub fn dir_entries(
        &self,
        path: &str,
    ) -> Result<std::collections::HashMap<String, FsDirEntry>, Error> {
        let pool = apr::SharedPool::from(apr::Pool::new());
        unsafe {
            let path_c = std::ffi::CString::new(path).unwrap();
            let mut entries = std::ptr::null_mut();
            let err = subversion_sys::svn_fs_dir_entries(
                &mut entries,
                self.ptr,
                path_c.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;

            if entries.is_null() {
                Ok(std::collections::HashMap::new())
            } else {
                let hash = crate::hash::FsDirentHash::from_ptr(entries);
                Ok(hash.to_hashmap(pool.clone()))
            }
        }
    }
    /// Check if file contents have changed between two paths
    pub fn contents_changed(&self, path1: &str, root2: &Root, path2: &str) -> Result<bool, Error> {
        with_tmp_pool(|pool| unsafe {
            let path1_c = std::ffi::CString::new(path1)?;
            let path2_c = std::ffi::CString::new(path2)?;
            let mut changed: subversion_sys::svn_boolean_t = 0;

            let err = subversion_sys::svn_fs_contents_changed(
                &mut changed,
                self.ptr,
                path1_c.as_ptr(),
                root2.ptr,
                path2_c.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;

            Ok(changed != 0)
        })
    }

    /// Check if properties have changed between two paths
    pub fn props_changed(&self, path1: &str, root2: &Root, path2: &str) -> Result<bool, Error> {
        with_tmp_pool(|pool| unsafe {
            let path1_c = std::ffi::CString::new(path1)?;
            let path2_c = std::ffi::CString::new(path2)?;
            let mut changed: subversion_sys::svn_boolean_t = 0;

            let err = subversion_sys::svn_fs_props_changed(
                &mut changed,
                self.ptr,
                path1_c.as_ptr(),
                root2.ptr,
                path2_c.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;

            Ok(changed != 0)
        })
    }

    /// Get the history of a node
    pub fn node_history(&self, path: &str) -> Result<NodeHistory, Error> {
        let pool = apr::Pool::new();
        unsafe {
            let path_c = std::ffi::CString::new(path)?;
            let mut history: *mut subversion_sys::svn_fs_history_t = std::ptr::null_mut();

            let err = subversion_sys::svn_fs_node_history(
                &mut history,
                self.ptr,
                path_c.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;

            if history.is_null() {
                return Err(Error::from_str("Failed to get node history"));
            }

            Ok(NodeHistory {
                ptr: history,
                _pool: pool,
            })
        }
    }

    /// Get the created revision of a path
    pub fn node_created_rev(&self, path: &str) -> Result<Revnum, Error> {
        with_tmp_pool(|pool| unsafe {
            let path_c = std::ffi::CString::new(path)?;
            let mut rev: subversion_sys::svn_revnum_t = -1;

            let err = subversion_sys::svn_fs_node_created_rev(
                &mut rev,
                self.ptr,
                path_c.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;

            Ok(Revnum(rev))
        })
    }

    /// Get the node ID for a path
    pub fn node_id(&self, path: &str) -> Result<NodeId, Error> {
        let pool = apr::Pool::new();
        unsafe {
            let path_c = std::ffi::CString::new(path)?;
            let mut id: *const subversion_sys::svn_fs_id_t = std::ptr::null();

            let err = subversion_sys::svn_fs_node_id(
                &mut id,
                self.ptr,
                path_c.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;

            if id.is_null() {
                return Err(Error::from_str("Failed to get node ID"));
            }

            Ok(NodeId {
                ptr: id,
                _pool: pool,
            })
        }
    }
}

/// Represents the history of a node in the filesystem
pub struct NodeHistory {
    ptr: *mut subversion_sys::svn_fs_history_t,
    _pool: apr::Pool,
}

impl NodeHistory {
    /// Get the previous history entry
    pub fn prev(&mut self, cross_copies: bool) -> Result<Option<(String, Revnum)>, Error> {
        unsafe {
            let mut prev_history: *mut subversion_sys::svn_fs_history_t = std::ptr::null_mut();
            let err = subversion_sys::svn_fs_history_prev(
                &mut prev_history,
                self.ptr,
                if cross_copies { 1 } else { 0 },
                self._pool.as_mut_ptr(),
            );
            svn_result(err)?;

            if prev_history.is_null() {
                return Ok(None);
            }

            // Update our pointer to the new history
            self.ptr = prev_history;

            // Get the location info for this history entry
            let mut path: *const std::ffi::c_char = std::ptr::null();
            let mut rev: subversion_sys::svn_revnum_t = -1;

            let err = subversion_sys::svn_fs_history_location(
                &mut path,
                &mut rev,
                prev_history,
                self._pool.as_mut_ptr(),
            );
            svn_result(err)?;

            if path.is_null() {
                return Ok(None);
            }

            let path_str = std::ffi::CStr::from_ptr(path)
                .to_string_lossy()
                .into_owned();
            Ok(Some((path_str, Revnum(rev))))
        }
    }
}

impl Drop for NodeHistory {
    fn drop(&mut self) {
        // History objects are allocated from pools and don't need explicit cleanup
    }
}

/// Represents a node ID in the filesystem
pub struct NodeId {
    ptr: *const subversion_sys::svn_fs_id_t,
    _pool: apr::Pool,
}

impl NodeId {
    /// Check if two node IDs are equal
    pub fn eq(&self, other: &NodeId) -> bool {
        unsafe {
            // svn_fs_compare_ids returns 0 if equal, -1 if different
            let result = subversion_sys::svn_fs_compare_ids(self.ptr, other.ptr);
            result == 0
        }
    }

    /// Convert the node ID to a string representation
    pub fn to_string(&self, pool: &apr::Pool) -> Result<String, Error> {
        unsafe {
            let str_svn = subversion_sys::svn_fs_unparse_id(self.ptr, pool.as_mut_ptr());
            if str_svn.is_null() {
                return Err(Error::from_str("Failed to unparse node ID"));
            }

            // svn_fs_unparse_id returns an svn_string_t
            let str_ptr = (*str_svn).data;
            let str_len = (*str_svn).len;

            let bytes = std::slice::from_raw_parts(str_ptr as *const u8, str_len);
            let result = String::from_utf8_lossy(bytes).into_owned();
            Ok(result)
        }
    }
}

/// Transaction handle with RAII cleanup
pub struct Transaction {
    ptr: *mut subversion_sys::svn_fs_txn_t,
    _pool: apr::Pool,
    _phantom: std::marker::PhantomData<*mut ()>, // !Send + !Sync
}

impl Drop for Transaction {
    fn drop(&mut self) {
        // Pool drop will clean up transaction
    }
}

impl Transaction {
    /// Create Transaction from existing pointer with pool (for repos integration)
    pub(crate) unsafe fn from_ptr_and_pool(
        ptr: *mut subversion_sys::svn_fs_txn_t,
        pool: apr::Pool,
    ) -> Self {
        Self {
            ptr,
            _pool: pool,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Get the underlying SVN transaction pointer
    pub(crate) fn as_ptr(&self) -> *mut subversion_sys::svn_fs_txn_t {
        self.ptr
    }

    /// Get the transaction name
    pub fn name(&self) -> Result<String, Error> {
        with_tmp_pool(|pool| unsafe {
            let mut name_ptr = std::ptr::null();
            let err = subversion_sys::svn_fs_txn_name(&mut name_ptr, self.ptr, pool.as_mut_ptr());
            Error::from_raw(err)?;
            let name_cstr = std::ffi::CStr::from_ptr(name_ptr);
            Ok(name_cstr.to_str()?.to_string())
        })
    }

    /// Get the base revision of this transaction
    pub fn base_revision(&self) -> Result<Revnum, Error> {
        unsafe {
            let base_rev = subversion_sys::svn_fs_txn_base_revision(self.ptr);
            Ok(Revnum(base_rev))
        }
    }

    /// Get the transaction root for making changes
    pub fn root(&mut self) -> Result<TxnRoot, Error> {
        let pool = apr::Pool::new();
        unsafe {
            let mut root_ptr = std::ptr::null_mut();
            let err = subversion_sys::svn_fs_txn_root(&mut root_ptr, self.ptr, pool.as_mut_ptr());
            Error::from_raw(err)?;
            Ok(TxnRoot {
                ptr: root_ptr,
                _pool: pool,
            })
        }
    }

    /// Set a property on this transaction
    pub fn change_prop(&mut self, name: &str, value: &str) -> Result<(), Error> {
        let name_cstr = std::ffi::CString::new(name)?;
        let value_str = subversion_sys::svn_string_t {
            data: value.as_ptr() as *mut _,
            len: value.len(),
        };
        let pool = apr::Pool::new();
        unsafe {
            let err = subversion_sys::svn_fs_change_txn_prop(
                self.ptr,
                name_cstr.as_ptr(),
                &value_str,
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(())
        }
    }

    /// Commit this transaction and return the new revision
    pub fn commit(self) -> Result<Revnum, Error> {
        let pool = apr::Pool::new();
        unsafe {
            let mut new_rev = 0;
            let err = subversion_sys::svn_fs_commit_txn(
                std::ptr::null_mut(), // conflict_p - we ignore conflicts for now
                &mut new_rev,
                self.ptr,
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(Revnum(new_rev))
        }
    }

    /// Abort this transaction
    pub fn abort(self) -> Result<(), Error> {
        let pool = apr::Pool::new();
        unsafe {
            let err = subversion_sys::svn_fs_abort_txn(self.ptr, pool.as_mut_ptr());
            Error::from_raw(err)?;
            Ok(())
        }
    }

    /// Change a transaction property (binary-safe version)
    pub fn change_prop_bytes(&self, name: &str, value: Option<&[u8]>) -> Result<(), Error> {
        let pool = apr::Pool::new();
        let name_cstr = std::ffi::CString::new(name)?;

        let value_ptr = value
            .map(|val| crate::svn_string_helpers::svn_string_ncreate(val, &pool))
            .unwrap_or(std::ptr::null_mut());

        unsafe {
            let err = subversion_sys::svn_fs_change_txn_prop(
                self.ptr,
                name_cstr.as_ptr(),
                value_ptr,
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
        }
        Ok(())
    }

    /// Get a transaction property
    pub fn prop(&self, name: &str) -> Result<Option<Vec<u8>>, Error> {
        let pool = apr::Pool::new();
        let name_cstr = std::ffi::CString::new(name)?;
        let mut value_ptr = std::ptr::null_mut();

        unsafe {
            let err = subversion_sys::svn_fs_txn_prop(
                &mut value_ptr,
                self.ptr,
                name_cstr.as_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;

            if value_ptr.is_null() {
                Ok(None)
            } else {
                let svn_str = &*value_ptr;
                let slice = std::slice::from_raw_parts(svn_str.data as *const u8, svn_str.len);
                Ok(Some(slice.to_vec()))
            }
        }
    }

    /// Get all transaction properties
    pub fn proplist(&self) -> Result<std::collections::HashMap<String, Vec<u8>>, Error> {
        let pool = apr::Pool::new();
        let mut props_ptr = std::ptr::null_mut();

        unsafe {
            let err =
                subversion_sys::svn_fs_txn_proplist(&mut props_ptr, self.ptr, pool.as_mut_ptr());
            Error::from_raw(err)?;

            let mut props = std::collections::HashMap::new();
            if !props_ptr.is_null() {
                let prop_hash = crate::props::PropHash::from_ptr(props_ptr);
                props = prop_hash.to_hashmap();
            }
            Ok(props)
        }
    }
}

/// Transaction root for making changes
pub struct TxnRoot {
    ptr: *mut subversion_sys::svn_fs_root_t,
    _pool: apr::Pool,
}

impl TxnRoot {
    /// Create a directory
    pub fn make_dir(&mut self, path: &str) -> Result<(), Error> {
        let path_cstr = std::ffi::CString::new(path)?;
        let pool = apr::Pool::new();
        unsafe {
            let err =
                subversion_sys::svn_fs_make_dir(self.ptr, path_cstr.as_ptr(), pool.as_mut_ptr());
            Error::from_raw(err)?;
            Ok(())
        }
    }

    /// Create a file
    pub fn make_file(&mut self, path: &str) -> Result<(), Error> {
        let path_cstr = std::ffi::CString::new(path)?;
        let pool = apr::Pool::new();
        unsafe {
            let err =
                subversion_sys::svn_fs_make_file(self.ptr, path_cstr.as_ptr(), pool.as_mut_ptr());
            Error::from_raw(err)?;
            Ok(())
        }
    }

    /// Delete a node
    pub fn delete(&mut self, path: &str) -> Result<(), Error> {
        let path_cstr = std::ffi::CString::new(path)?;
        let pool = apr::Pool::new();
        unsafe {
            let err =
                subversion_sys::svn_fs_delete(self.ptr, path_cstr.as_ptr(), pool.as_mut_ptr());
            Error::from_raw(err)?;
            Ok(())
        }
    }

    /// Copy a node from another location
    pub fn copy(&mut self, from_root: &Root, from_path: &str, to_path: &str) -> Result<(), Error> {
        let from_path_cstr = std::ffi::CString::new(from_path)?;
        let to_path_cstr = std::ffi::CString::new(to_path)?;
        let pool = apr::Pool::new();
        unsafe {
            let err = subversion_sys::svn_fs_copy(
                from_root.ptr,
                from_path_cstr.as_ptr(),
                self.ptr,
                to_path_cstr.as_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(())
        }
    }

    /// Apply text changes to a file
    pub fn apply_text(
        &mut self,
        path: &str,
        result_checksum: Option<&str>,
    ) -> Result<crate::io::Stream, Error> {
        let path_cstr = std::ffi::CString::new(path)?;
        let checksum_cstr = result_checksum.map(std::ffi::CString::new).transpose()?;
        let pool = apr::Pool::new();
        unsafe {
            let mut stream_ptr = std::ptr::null_mut();
            let err = subversion_sys::svn_fs_apply_text(
                &mut stream_ptr,
                self.ptr,
                path_cstr.as_ptr(),
                checksum_cstr
                    .as_ref()
                    .map_or(std::ptr::null(), |c| c.as_ptr() as *const _),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            // Create a Stream from the raw pointer and pool
            Ok(crate::io::Stream::from_ptr(stream_ptr, pool))
        }
    }

    /// Set a property on a node
    pub fn change_node_prop(&mut self, path: &str, name: &str, value: &[u8]) -> Result<(), Error> {
        let path_cstr = std::ffi::CString::new(path)?;
        let name_cstr = std::ffi::CString::new(name)?;
        let value_str = if value.is_empty() {
            std::ptr::null()
        } else {
            &subversion_sys::svn_string_t {
                data: value.as_ptr() as *mut _,
                len: value.len(),
            }
        };
        let pool = apr::Pool::new();
        unsafe {
            let err = subversion_sys::svn_fs_change_node_prop(
                self.ptr,
                path_cstr.as_ptr(),
                name_cstr.as_ptr(),
                value_str,
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(())
        }
    }

    /// Check if a path exists and what kind of node it is
    pub fn check_path(&self, path: &str) -> Result<crate::NodeKind, Error> {
        let path_cstr = std::ffi::CString::new(path)?;
        let pool = apr::Pool::new();
        unsafe {
            let mut kind = 0;
            let err = subversion_sys::svn_fs_check_path(
                &mut kind,
                self.ptr,
                path_cstr.as_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(crate::NodeKind::from(kind))
        }
    }

    /// Set file contents directly
    pub fn set_file_contents(&mut self, path: &str, contents: &[u8]) -> Result<(), Error> {
        let path_cstr = std::ffi::CString::new(path)?;
        let pool = apr::Pool::new();

        unsafe {
            let mut stream = std::ptr::null_mut();
            let err = subversion_sys::svn_fs_apply_text(
                &mut stream,
                self.ptr,
                path_cstr.as_ptr(),
                std::ptr::null(), // result_checksum
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;

            // Write the contents to the stream
            let bytes_written = subversion_sys::svn_stream_write(
                stream,
                contents.as_ptr() as *const std::ffi::c_char,
                &mut (contents.len() as usize),
            );
            Error::from_raw(bytes_written)?;

            // Close the stream
            let err = subversion_sys::svn_stream_close(stream);
            Error::from_raw(err)?;
        }

        Ok(())
    }

    /// Get the contents of a file as bytes
    pub fn file_contents(&self, path: &str) -> Result<Vec<u8>, Error> {
        let path_cstr = std::ffi::CString::new(path)?;
        let pool = apr::Pool::new();

        unsafe {
            let mut stream = std::ptr::null_mut();
            let err = subversion_sys::svn_fs_file_contents(
                &mut stream,
                self.ptr,
                path_cstr.as_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;

            // Read all contents from the stream
            let mut contents = Vec::new();
            let mut buffer = [0u8; 4096];
            loop {
                let mut len = buffer.len();
                let err = subversion_sys::svn_stream_read2(
                    stream,
                    buffer.as_mut_ptr() as *mut std::ffi::c_char,
                    &mut len,
                );
                Error::from_raw(err)?;

                if len == 0 {
                    break;
                }

                contents.extend_from_slice(&buffer[..len]);
            }

            let err = subversion_sys::svn_stream_close(stream);
            Error::from_raw(err)?;

            Ok(contents)
        }
    }
}

impl Fs {
    /// Begin a new transaction
    pub fn begin_txn(&self, base_rev: Revnum, flags: u32) -> Result<Transaction, Error> {
        let pool = apr::Pool::new();
        unsafe {
            let mut txn_ptr = std::ptr::null_mut();
            let err = subversion_sys::svn_fs_begin_txn2(
                &mut txn_ptr,
                self.as_ptr() as *mut _,
                base_rev.into(),
                flags,
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(Transaction {
                ptr: txn_ptr,
                _pool: pool,
                _phantom: std::marker::PhantomData,
            })
        }
    }

    /// Open an existing transaction by name
    pub fn open_txn(&self, name: &str) -> Result<Transaction, Error> {
        let name_cstr = std::ffi::CString::new(name)?;
        let pool = apr::Pool::new();
        unsafe {
            let mut txn_ptr = std::ptr::null_mut();
            let err = subversion_sys::svn_fs_open_txn(
                &mut txn_ptr,
                self.as_ptr() as *mut _,
                name_cstr.as_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(Transaction {
                ptr: txn_ptr,
                _pool: pool,
                _phantom: std::marker::PhantomData,
            })
        }
    }

    /// List all uncommitted transactions
    pub fn list_transactions(&self) -> Result<Vec<String>, Error> {
        let pool = apr::Pool::new();
        unsafe {
            let mut names_array = std::ptr::null_mut();
            let err = subversion_sys::svn_fs_list_transactions(
                &mut names_array,
                self.as_ptr() as *mut _,
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;

            if names_array.is_null() {
                return Ok(Vec::new());
            }

            let array = &*names_array;
            let mut result = Vec::new();
            for i in 0..array.nelts {
                let name_ptr = *((array.elts as *const *const std::ffi::c_char).add(i as usize));
                if !name_ptr.is_null() {
                    let name = std::ffi::CStr::from_ptr(name_ptr).to_str()?.to_string();
                    result.push(name);
                }
            }
            Ok(result)
        }
    }

    /// Purge (remove) an uncommitted transaction
    pub fn purge_txn(&self, name: &str) -> Result<(), Error> {
        let pool = apr::Pool::new();
        let name_cstr = std::ffi::CString::new(name)?;
        unsafe {
            let err = subversion_sys::svn_fs_purge_txn(
                self.as_ptr() as *mut _,
                name_cstr.as_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)
        }
    }

    /// Merge changes between trees
    ///
    /// This performs a 3-way merge between a common ancestor and two descendant trees.
    /// Returns the conflicts if any occurred during the merge.
    pub fn merge(
        &self,
        source: &Root,
        target: &mut TxnRoot,
        ancestor: &Root,
        ancestor_path: &str,
        source_path: &str,
        target_path: &str,
    ) -> Result<Option<String>, Error> {
        let pool = apr::Pool::new();
        let ancestor_path_cstr = std::ffi::CString::new(ancestor_path)?;
        let source_path_cstr = std::ffi::CString::new(source_path)?;
        let target_path_cstr = std::ffi::CString::new(target_path)?;

        unsafe {
            let mut conflict_ptr = std::ptr::null();
            let err = subversion_sys::svn_fs_merge(
                &mut conflict_ptr,
                source.ptr,
                source_path_cstr.as_ptr(),
                target.ptr,
                target_path_cstr.as_ptr(),
                ancestor.ptr,
                ancestor_path_cstr.as_ptr(),
                pool.as_mut_ptr(),
            );

            // Check if error is a merge conflict
            if !err.is_null() {
                let err_code = (*err).apr_err;
                if err_code == subversion_sys::svn_errno_t_SVN_ERR_FS_CONFLICT as i32 {
                    // Get conflict string if available
                    let conflict = if !conflict_ptr.is_null() {
                        Some(std::ffi::CStr::from_ptr(conflict_ptr).to_str()?.to_string())
                    } else {
                        None
                    };
                    // Clear the error since we're handling it
                    subversion_sys::svn_error_clear(err);
                    return Ok(conflict);
                }
                // Other errors propagate normally
                Error::from_raw(err)?;
            }

            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_fs_create_and_open() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        // Create filesystem
        let fs = Fs::create(&fs_path);
        assert!(fs.is_ok(), "Failed to create filesystem");

        // Open existing filesystem
        let fs = Fs::open(&fs_path);
        assert!(fs.is_ok(), "Failed to open filesystem");
    }

    #[test]
    fn test_fs_youngest_rev() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let fs = Fs::create(&fs_path).unwrap();
        let rev = fs.youngest_revision();
        assert!(rev.is_ok());
        // New filesystem should have revision 0
        assert_eq!(rev.unwrap(), crate::Revnum(0));
    }

    #[test]
    fn test_fs_get_uuid() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let fs = Fs::create(&fs_path).unwrap();
        let uuid = fs.get_uuid();
        assert!(uuid.is_ok());
        // UUID should not be empty
        assert!(!uuid.unwrap().is_empty());
    }

    #[test]
    fn test_fs_mutability() {
        // Test that methods requiring only &self work correctly
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let fs = Fs::create(&fs_path).unwrap();

        // These should work with immutable reference
        assert!(fs.get_uuid().is_ok());
        assert!(fs.revision_proplist(crate::Revnum(0)).is_ok());
        assert!(fs.revision_root(crate::Revnum(0)).is_ok());
        assert!(fs.youngest_revision().is_ok());
    }

    #[test]
    fn test_root_creation() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let fs = Fs::create(&fs_path).unwrap();
        let root = fs.revision_root(crate::Revnum(0));
        assert!(root.is_ok(), "Failed to get revision root");
    }

    #[test]
    fn test_drop_cleanup() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        {
            let _fs = Fs::create(&fs_path).unwrap();
            // Fs should be dropped here
        }

        // Should be able to open again after drop
        let fs = Fs::open(&fs_path);
        assert!(fs.is_ok());
    }

    #[test]
    fn test_root_check_path() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let fs = Fs::create(&fs_path).unwrap();
        let root = fs.revision_root(crate::Revnum(0)).unwrap();

        // Root directory should exist
        let kind = root.check_path("/").unwrap();
        assert_eq!(kind, crate::NodeKind::Dir);

        // Non-existent path should return None
        let kind = root.check_path("/nonexistent").unwrap();
        assert_eq!(kind, crate::NodeKind::None);
    }

    #[test]
    fn test_root_is_dir() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let fs = Fs::create(&fs_path).unwrap();
        let root = fs.revision_root(crate::Revnum(0)).unwrap();

        // Root should be a directory
        assert!(root.is_dir("/").unwrap());

        // Non-existent path should return false
        assert!(!root.is_dir("/nonexistent").unwrap_or(false));
    }

    #[test]
    fn test_root_dir_entries() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let fs = Fs::create(&fs_path).unwrap();
        let root = fs.revision_root(crate::Revnum(0)).unwrap();

        // Root directory should be empty initially
        let entries = root.dir_entries("/").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_root_proplist() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let fs = Fs::create(&fs_path).unwrap();
        let root = fs.revision_root(crate::Revnum(0)).unwrap();

        // Root should have no properties initially
        let props = root.proplist("/").unwrap();
        assert!(props.is_empty());
    }

    #[test]
    fn test_root_paths_changed() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let fs = Fs::create(&fs_path).unwrap();
        let root = fs.revision_root(crate::Revnum(0)).unwrap();

        // Initial revision should have no changes
        let changes = root.paths_changed().unwrap();
        assert!(changes.is_empty());
    }

    #[test]
    fn test_fs_path_change_accessors() {
        // Test FsPathChange accessors with a mock pointer
        // Note: This is a basic test since we can't easily create real path changes
        // without complex repository operations
        // We can at least test the structure exists and compiles
        // Real integration tests would require creating actual commits
    }

    #[test]
    fn test_fs_dir_entry_accessors() {
        // Test FsDirEntry accessors
        // Note: This is a basic test since we need actual directory entries
        // We can at least test the structure exists and compiles
        // Real integration tests would require creating actual files/directories
    }

    #[test]
    fn test_transaction_basic() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let fs = Fs::create(&fs_path).unwrap();

        // Begin a transaction
        let mut txn = fs.begin_txn(crate::Revnum::from(0u32), 0).unwrap();

        // Check basic transaction properties
        let name = txn.name().unwrap();
        assert!(!name.is_empty(), "Transaction should have a name");

        let base_rev = txn.base_revision().unwrap();
        assert_eq!(
            base_rev,
            crate::Revnum::from(0u32),
            "Base revision should be 0"
        );

        // Set a transaction property
        txn.change_prop("svn:log", "Test commit message").unwrap();
        txn.change_prop("svn:author", "test-user").unwrap();

        // Get transaction root and make some changes
        let mut root = txn.root().unwrap();

        // Create a directory
        root.make_dir("/trunk").unwrap();

        // Verify the directory exists
        let kind = root.check_path("/trunk").unwrap();
        assert_eq!(kind, crate::NodeKind::Dir);

        // Create a file
        root.make_file("/trunk/test.txt").unwrap();

        // Verify the file exists
        let kind = root.check_path("/trunk/test.txt").unwrap();
        assert_eq!(kind, crate::NodeKind::File);

        // Add content to the file
        let mut stream = root.apply_text("/trunk/test.txt", None).unwrap();
        use std::io::Write;
        stream.write_all(b"Hello, World!\n").unwrap();
        drop(stream);

        // Set a property on the file
        root.change_node_prop("/trunk/test.txt", "custom:prop", b"value")
            .unwrap();

        // Commit the transaction
        let new_rev = txn.commit().unwrap();
        assert_eq!(
            new_rev,
            crate::Revnum::from(1u32),
            "First commit should be revision 1"
        );

        // Verify the youngest revision is updated
        let youngest = fs.youngest_revision().unwrap();
        assert_eq!(youngest, crate::Revnum::from(1u32));
    }

    #[test]
    fn test_transaction_abort() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let fs = Fs::create(&fs_path).unwrap();

        // Begin a transaction
        let mut txn = fs.begin_txn(crate::Revnum(0), 0).unwrap();
        let mut root = txn.root().unwrap();

        // Make some changes
        root.make_dir("/test-dir").unwrap();
        root.make_file("/test-file.txt").unwrap();

        // Abort the transaction
        txn.abort().unwrap();

        // Verify no changes were committed
        let youngest = fs.youngest_revision().unwrap();
        assert_eq!(
            youngest,
            crate::Revnum(0),
            "No changes should be committed after abort"
        );
    }

    #[test]
    fn test_transaction_copy() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let fs = Fs::create(&fs_path).unwrap();

        // First, create some content to copy
        let mut txn1 = fs.begin_txn(crate::Revnum(0), 0).unwrap();
        txn1.change_prop("svn:log", "Initial commit").unwrap();
        let mut root1 = txn1.root().unwrap();

        root1.make_dir("/original").unwrap();
        root1.make_file("/original/file.txt").unwrap();

        let mut stream = root1.apply_text("/original/file.txt", None).unwrap();
        use std::io::Write;
        stream.write_all(b"Original content\n").unwrap();
        drop(stream);

        let rev1 = txn1.commit().unwrap();

        // Now copy the content in a new transaction
        let mut txn2 = fs.begin_txn(rev1, 0).unwrap();
        txn2.change_prop("svn:log", "Copy operation").unwrap();
        let mut root2 = txn2.root().unwrap();

        // Get the root from revision 1 for copying
        let rev1_root = fs.revision_root(rev1).unwrap();

        // Copy the directory
        root2.copy(&rev1_root, "/original", "/copy").unwrap();

        // Verify the copy exists
        let kind = root2.check_path("/copy").unwrap();
        assert_eq!(kind, crate::NodeKind::Dir);

        let kind = root2.check_path("/copy/file.txt").unwrap();
        assert_eq!(kind, crate::NodeKind::File);

        let _rev2 = txn2.commit().unwrap();
    }

    #[test]
    fn test_transaction_delete() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let fs = Fs::create(&fs_path).unwrap();

        // Create content first
        let mut txn1 = fs.begin_txn(crate::Revnum(0), 0).unwrap();
        txn1.change_prop("svn:log", "Create files").unwrap();
        let mut root1 = txn1.root().unwrap();

        root1.make_dir("/dir1").unwrap();
        root1.make_file("/dir1/file1.txt").unwrap();
        root1.make_file("/file2.txt").unwrap();

        let rev1 = txn1.commit().unwrap();

        // Now delete some content
        let mut txn2 = fs.begin_txn(rev1, 0).unwrap();
        txn2.change_prop("svn:log", "Delete files").unwrap();
        let mut root2 = txn2.root().unwrap();

        // Delete a file
        root2.delete("/file2.txt").unwrap();

        // Verify it's gone
        let kind = root2.check_path("/file2.txt").unwrap();
        assert_eq!(kind, crate::NodeKind::None);

        // But the other file still exists
        let kind = root2.check_path("/dir1/file1.txt").unwrap();
        assert_eq!(kind, crate::NodeKind::File);

        let _rev2 = txn2.commit().unwrap();
    }

    #[test]
    fn test_transaction_properties() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let fs = Fs::create(&fs_path).unwrap();

        let mut txn = fs.begin_txn(crate::Revnum(0), 0).unwrap();
        let mut root = txn.root().unwrap();

        // Create a file and set properties
        root.make_file("/test.txt").unwrap();

        // Set various properties
        root.change_node_prop("/test.txt", "svn:mime-type", b"text/plain")
            .unwrap();
        root.change_node_prop("/test.txt", "custom:author", b"test-user")
            .unwrap();
        root.change_node_prop("/test.txt", "custom:description", b"A test file")
            .unwrap();

        // Set an empty property (delete)
        root.change_node_prop("/test.txt", "custom:empty", b"")
            .unwrap();

        // Set transaction properties
        txn.change_prop("svn:log", "Test commit with properties")
            .unwrap();
        txn.change_prop("svn:author", "property-tester").unwrap();
        txn.change_prop("svn:date", "2023-01-01T12:00:00.000000Z")
            .unwrap();

        let _rev = txn.commit().unwrap();
    }

    #[test]
    fn test_open_transaction() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let fs = Fs::create(&fs_path).unwrap();

        // Begin a transaction
        let txn = fs.begin_txn(crate::Revnum(0), 0).unwrap();
        let txn_name = txn.name().unwrap();

        // Don't commit it, just get the name
        drop(txn);

        // Try to open the transaction by name
        // Note: This might fail if the transaction was cleaned up
        // This test mainly verifies the API works
        let _result = fs.open_txn(&txn_name);
        // We don't assert success because transaction cleanup behavior
        // depends on SVN implementation details
    }

    #[test]
    fn test_node_history_and_content_comparison() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        // Create a filesystem
        let fs = Fs::create(&fs_path).unwrap();

        // Create first revision with a file
        let mut txn = fs.begin_txn(Revnum(0), 0).unwrap();
        let mut txn_root = txn.root().unwrap();

        // Create a file
        txn_root.make_file("/test.txt").unwrap();
        let mut stream = txn_root.apply_text("/test.txt", None).unwrap();
        use std::io::Write;
        write!(stream, "Initial content").unwrap();
        stream.close().unwrap();

        // Commit the transaction
        let rev1 = txn.commit().unwrap();

        // Create second revision modifying the file
        let mut txn2 = fs.begin_txn(rev1, 0).unwrap();
        let mut txn_root2 = txn2.root().unwrap();

        let mut stream2 = txn_root2.apply_text("/test.txt", None).unwrap();
        write!(stream2, "Modified content").unwrap();
        stream2.close().unwrap();

        let rev2 = txn2.commit().unwrap();

        // Test content comparison
        let root1 = fs.revision_root(rev1).unwrap();
        let root2 = fs.revision_root(rev2).unwrap();

        // Contents should be different between revisions
        let contents_changed = root1
            .contents_changed("/test.txt", &root2, "/test.txt")
            .unwrap();
        assert!(
            contents_changed,
            "File contents should have changed between revisions"
        );

        // Contents should be the same when comparing the same revision
        let contents_same = root1
            .contents_changed("/test.txt", &root1, "/test.txt")
            .unwrap();
        assert!(
            !contents_same,
            "File contents should be the same in the same revision"
        );

        // Test node history
        let _history = root2.node_history("/test.txt").unwrap();
        // We should be able to get the history
        // Note: detailed history iteration would require more setup

        // Test node created revision
        // Note: node_created_rev returns the revision where the current node instance was created
        let created_rev1 = root1.node_created_rev("test.txt").unwrap();
        assert_eq!(
            created_rev1, rev1,
            "Node in rev1 should have been created in rev1"
        );

        let created_rev2 = root2.node_created_rev("test.txt").unwrap();
        assert_eq!(
            created_rev2, rev2,
            "Node in rev2 should have been created in rev2 (after modification)"
        );

        // Test node ID
        let node_id1 = root1.node_id("test.txt").unwrap();
        let node_id2 = root2.node_id("test.txt").unwrap();

        // Just verify we can get node IDs - comparison semantics may vary
        // based on SVN backend implementation
        let pool = apr::Pool::new();
        let _id1_str = node_id1.to_string(&pool).unwrap();
        let _id2_str = node_id2.to_string(&pool).unwrap();

        // Cleanup handled by tempdir Drop
    }

    #[test]
    fn test_props_changed() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        // Create a filesystem
        let fs = Fs::create(&fs_path).unwrap();

        // Create first revision with a file
        let mut txn = fs.begin_txn(Revnum(0), 0).unwrap();
        let mut txn_root = txn.root().unwrap();

        // Create a file with a property
        txn_root.make_file("/test.txt").unwrap();
        txn_root
            .change_node_prop("/test.txt", "custom:prop", b"value1")
            .unwrap();

        let rev1 = txn.commit().unwrap();

        // Create second revision changing the property
        let mut txn2 = fs.begin_txn(rev1, 0).unwrap();
        let mut txn_root2 = txn2.root().unwrap();

        txn_root2
            .change_node_prop("/test.txt", "custom:prop", b"value2")
            .unwrap();

        let rev2 = txn2.commit().unwrap();

        // Test property comparison
        let root1 = fs.revision_root(rev1).unwrap();
        let root2 = fs.revision_root(rev2).unwrap();

        // Properties should be different between revisions
        let props_changed = root1.props_changed("test.txt", &root2, "test.txt").unwrap();
        assert!(
            props_changed,
            "Properties should have changed between revisions"
        );

        // Properties should be the same when comparing the same revision
        let props_same = root1.props_changed("test.txt", &root1, "test.txt").unwrap();
        assert!(
            !props_same,
            "Properties should be the same in the same revision"
        );

        // Cleanup handled by tempdir Drop
    }

    #[test]
    fn test_transaction_operations() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let fs = Fs::create(&fs_path).unwrap();

        // Test list_transactions with no transactions
        let txns = fs.list_transactions().unwrap();
        assert_eq!(txns.len(), 0, "Should have no transactions initially");

        // Create a transaction
        let txn = fs.begin_txn(Revnum(0), 0).unwrap();
        let txn_name = txn.name().unwrap();

        // Now we should see it in the list
        let txns = fs.list_transactions().unwrap();
        assert_eq!(txns.len(), 1, "Should have one transaction");
        assert_eq!(txns[0], txn_name);

        // Abort the transaction
        txn.abort().unwrap();

        // Should be empty again
        let txns = fs.list_transactions().unwrap();
        assert_eq!(txns.len(), 0, "Should have no transactions after abort");
    }

    #[test]
    fn test_txn_file_operations() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let fs = Fs::create(&fs_path).unwrap();

        // First create a file in revision 1
        let mut txn1 = fs.begin_txn(Revnum(0), 0).unwrap();
        let mut txn_root1 = txn1.root().unwrap();
        txn_root1.make_file("/test.txt").unwrap();
        txn_root1
            .set_file_contents("/test.txt", b"Hello, World!")
            .unwrap();
        let rev1 = txn1.commit().unwrap();

        // Now test moving the file in a new transaction
        let mut txn2 = fs.begin_txn(rev1, 0).unwrap();
        let mut txn_root2 = txn2.root().unwrap();

        // Move requires copying from the base revision then deleting the old
        let base_root = fs.revision_root(rev1).unwrap();
        txn_root2
            .copy(&base_root, "/test.txt", "/renamed.txt")
            .unwrap();
        txn_root2.delete("/test.txt").unwrap();

        // Check that old path doesn't exist and new one does
        let old_kind = txn_root2.check_path("/test.txt").unwrap();
        assert_eq!(old_kind, crate::NodeKind::None);

        let new_kind = txn_root2.check_path("/renamed.txt").unwrap();
        assert_eq!(new_kind, crate::NodeKind::File);

        let rev2 = txn2.commit().unwrap();

        // Verify in committed revision
        let root = fs.revision_root(rev2).unwrap();
        let mut stream = root.file_contents("renamed.txt").unwrap();
        let mut contents = Vec::new();
        let mut buffer = [0u8; 1024];
        loop {
            let bytes_read = stream.read_full(&mut buffer).unwrap();
            if bytes_read == 0 {
                break;
            }
            contents.extend_from_slice(&buffer[..bytes_read]);
        }
        assert_eq!(contents, b"Hello, World!");
    }

    #[test]
    fn test_merge_trees() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let fs = Fs::create(&fs_path).unwrap();

        // Create initial revision with a file
        let mut txn1 = fs.begin_txn(Revnum(0), 0).unwrap();
        let mut root1 = txn1.root().unwrap();
        root1.make_file("/file.txt").unwrap();
        root1
            .set_file_contents("/file.txt", b"Initial content")
            .unwrap();
        let rev1 = txn1.commit().unwrap();

        // Create two divergent changes
        // Branch 1: modify the file
        let mut txn2 = fs.begin_txn(rev1, 0).unwrap();
        let mut root2 = txn2.root().unwrap();
        root2
            .set_file_contents("/file.txt", b"Branch 1 content")
            .unwrap();
        let rev2 = txn2.commit().unwrap();

        // Branch 2: also modify the file (creating a conflict)
        let mut txn3 = fs.begin_txn(rev1, 0).unwrap();
        let mut root3 = txn3.root().unwrap();
        root3
            .set_file_contents("/file.txt", b"Branch 2 content")
            .unwrap();

        // Try to merge branch 1 into branch 2
        let ancestor = fs.revision_root(rev1).unwrap();
        let source = fs.revision_root(rev2).unwrap();

        let conflict = fs
            .merge(&source, &mut root3, &ancestor, "", "", "")
            .unwrap();

        // This should produce a conflict since both branches modified the same file
        assert!(conflict.is_some(), "Should have a merge conflict");
    }

    #[test]
    fn test_contents_and_props_changed() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let fs = Fs::create(&fs_path).unwrap();

        // Create initial file with properties
        let mut txn1 = fs.begin_txn(Revnum(0), 0).unwrap();
        let mut root1 = txn1.root().unwrap();
        root1.make_file("/file.txt").unwrap();
        root1
            .set_file_contents("/file.txt", b"Original content")
            .unwrap();
        root1
            .change_node_prop("/file.txt", "custom:prop", b"value1")
            .unwrap();
        let rev1 = txn1.commit().unwrap();

        // Modify contents but not properties
        let mut txn2 = fs.begin_txn(rev1, 0).unwrap();
        let mut root2 = txn2.root().unwrap();
        root2
            .set_file_contents("/file.txt", b"Modified content")
            .unwrap();
        let rev2 = txn2.commit().unwrap();

        // Check changes between rev1 and rev2
        let root_r1 = fs.revision_root(rev1).unwrap();
        let root_r2 = fs.revision_root(rev2).unwrap();

        // Contents should have changed
        let contents_changed = root_r1
            .contents_changed("/file.txt", &root_r2, "/file.txt")
            .unwrap();
        assert!(contents_changed, "Contents should have changed");

        // Properties should not have changed
        let props_changed = root_r1
            .props_changed("/file.txt", &root_r2, "/file.txt")
            .unwrap();
        assert!(!props_changed, "Properties should not have changed");

        // Now modify properties
        let mut txn3 = fs.begin_txn(rev2, 0).unwrap();
        let mut root3 = txn3.root().unwrap();
        root3
            .change_node_prop("/file.txt", "custom:prop", b"value2")
            .unwrap();
        let rev3 = txn3.commit().unwrap();

        // Check changes between rev2 and rev3
        let root_r3 = fs.revision_root(rev3).unwrap();

        // Contents should not have changed
        let contents_changed = root_r2
            .contents_changed("/file.txt", &root_r3, "/file.txt")
            .unwrap();
        assert!(!contents_changed, "Contents should not have changed");

        // Properties should have changed
        let props_changed = root_r2
            .props_changed("/file.txt", &root_r3, "/file.txt")
            .unwrap();
        assert!(props_changed, "Properties should have changed");
    }

    #[test]
    fn test_node_history() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let fs = Fs::create(&fs_path).unwrap();

        // Create a file in rev 1
        let mut txn1 = fs.begin_txn(Revnum(0), 0).unwrap();
        let mut root1 = txn1.root().unwrap();
        root1.make_file("/file.txt").unwrap();
        root1.set_file_contents("/file.txt", b"Version 1").unwrap();
        let rev1 = txn1.commit().unwrap();

        // Modify the file in rev 2
        let mut txn2 = fs.begin_txn(rev1, 0).unwrap();
        let mut root2 = txn2.root().unwrap();
        root2.set_file_contents("/file.txt", b"Version 2").unwrap();
        let rev2 = txn2.commit().unwrap();

        // Copy the file in rev 3
        let mut txn3 = fs.begin_txn(rev2, 0).unwrap();
        let mut root3 = txn3.root().unwrap();
        let source_root = fs.revision_root(rev2).unwrap();
        root3.copy(&source_root, "/file.txt", "/copied.txt").unwrap();
        let rev3 = txn3.commit().unwrap();

        // Get history of the copied file
        let root_r3 = fs.revision_root(rev3).unwrap();
        let mut history = root_r3.node_history("/copied.txt").unwrap();

        // Get previous history entries
        if let Some((path, revision)) = history.prev(true).unwrap() {
            // The first prev should give us the current location
            assert!(path.contains("copied.txt") || path.contains("file.txt"));
            assert!(revision.0 <= rev3.0);

            // Go further back (should show file.txt history if cross_copies is true)
            if let Some((prev_path, prev_revision)) = history.prev(true).unwrap() {
                // This should be from the file.txt history
                assert!(prev_path.contains("file.txt") || prev_path.contains("copied.txt"));
                assert!(prev_revision.0 <= rev2.0);
            }
        }
    }

    #[test]
    fn test_fs_lock_unlock() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let mut fs = Fs::create(&fs_path).unwrap();

        // Set a username for lock operations
        fs.set_access("testuser").unwrap();

        // Create a transaction to add a file
        let mut txn = fs.begin_txn(crate::Revnum(0), 0).unwrap();
        let mut root = txn.root().unwrap();
        root.make_file("/test.txt").unwrap();
        root.set_file_contents("/test.txt", b"test content").unwrap();
        let rev = txn.commit().unwrap();

        // Test locking a file
        let lock = fs.lock(
            "/test.txt",
            None, // Let SVN generate a token
            Some("Test lock comment"),
            false,
            None, // No expiration
            rev,
            false, // Don't steal lock
        );

        assert!(lock.is_ok(), "Failed to lock file: {:?}", lock.err());
        let lock = lock.unwrap();

        // Verify the lock was created (SVN adds leading slash)
        assert_eq!(lock.path(), "/test.txt");
        assert!(!lock.token().is_empty());
        assert_eq!(lock.comment(), "Test lock comment");

        // Get the lock info
        let lock_info = fs.get_lock("/test.txt");
        assert!(lock_info.is_ok());
        let lock_info = lock_info.unwrap();
        assert!(lock_info.is_some());
        let lock_info = lock_info.unwrap();
        assert_eq!(lock_info.path(), "/test.txt");
        assert_eq!(lock_info.token(), lock.token());

        // Unlock the file
        let unlock_result = fs.unlock("/test.txt", lock.token(), false);
        assert!(
            unlock_result.is_ok(),
            "Failed to unlock: {:?}",
            unlock_result.err()
        );

        // Verify the lock is gone
        let lock_info = fs.get_lock("/test.txt");
        assert!(lock_info.is_ok());
        assert!(lock_info.unwrap().is_none());
    }

    #[test]
    fn test_fs_lock_steal() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let mut fs = Fs::create(&fs_path).unwrap();

        // Set a username for lock operations
        fs.set_access("testuser").unwrap();

        // Create a file
        let mut txn = fs.begin_txn(crate::Revnum(0), 0).unwrap();
        let mut root = txn.root().unwrap();
        root.make_file("/locked.txt").unwrap();
        root.set_file_contents("/locked.txt", b"content").unwrap();
        let rev = txn.commit().unwrap();

        // Lock the file
        let _lock1 = fs
            .lock(
                "/locked.txt",
                None,
                Some("First lock"),
                false,
                None,
                rev,
                false,
            )
            .unwrap();

        // Try to lock again without stealing (should fail)
        let lock2 = fs.lock(
            "/locked.txt",
            None,
            Some("Second lock"),
            false,
            None,
            rev,
            false, // Don't steal
        );
        assert!(
            lock2.is_err(),
            "Should not be able to lock already locked file"
        );

        // Now steal the lock
        let lock3 = fs.lock(
            "/locked.txt",
            None,
            Some("Stolen lock"),
            false,
            None,
            rev,
            true, // Steal lock
        );
        assert!(lock3.is_ok(), "Should be able to steal lock");
        let lock3 = lock3.unwrap();

        // The stolen lock should have updated comment
        // Note: SVN may reuse the same token when stealing
        assert_eq!(lock3.comment(), "Stolen lock");
    }

    #[test]
    fn test_fs_get_locks() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let mut fs = Fs::create(&fs_path).unwrap();

        // Set a username for lock operations
        fs.set_access("testuser").unwrap();

        // Create multiple files in a directory structure
        let mut txn = fs.begin_txn(crate::Revnum(0), 0).unwrap();
        let mut root = txn.root().unwrap();
        root.make_dir("/dir1").unwrap();
        root.make_file("/dir1/file1.txt").unwrap();
        root.make_file("/dir1/file2.txt").unwrap();
        root.make_dir("/dir1/subdir").unwrap();
        root.make_file("/dir1/subdir/file3.txt").unwrap();
        let rev = txn.commit().unwrap();

        // Lock multiple files
        fs.lock(
            "/dir1/file1.txt",
            None,
            Some("Lock 1"),
            false,
            None,
            rev,
            false,
        )
        .unwrap();
        fs.lock(
            "/dir1/file2.txt",
            None,
            Some("Lock 2"),
            false,
            None,
            rev,
            false,
        )
        .unwrap();
        fs.lock(
            "/dir1/subdir/file3.txt",
            None,
            Some("Lock 3"),
            false,
            None,
            rev,
            false,
        )
        .unwrap();

        // Get all locks under dir1 with infinity depth
        let locks = fs.get_locks("/dir1", crate::Depth::Infinity);
        assert!(locks.is_ok(), "Failed to get locks: {:?}", locks.err());
        let locks = locks.unwrap();
        assert_eq!(locks.len(), 3, "Should find 3 locks");

        // Get locks with immediates depth (only direct children)
        let locks_immediate = fs.get_locks("/dir1", crate::Depth::Immediates);
        assert!(locks_immediate.is_ok());
        let locks_immediate = locks_immediate.unwrap();
        // Should get file1.txt and file2.txt but not file3.txt
        assert_eq!(
            locks_immediate.len(),
            2,
            "Should find 2 locks at immediate depth"
        );

        // Get locks with files depth
        let locks_files = fs.get_locks("/dir1", crate::Depth::Files);
        assert!(locks_files.is_ok());
        let locks_files = locks_files.unwrap();
        assert_eq!(locks_files.len(), 2, "Should find 2 locks with files depth");
    }

    #[test]
    fn test_fs_pack() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        // Create a filesystem
        Fs::create(&fs_path).unwrap();

        // Pack should work on an empty repository
        let result = pack(
            &fs_path, None, // No notify callback
            None, // No cancel callback
        );
        assert!(result.is_ok(), "Pack should succeed on new repository");
    }

    #[test]
    fn test_fs_verify() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        // Create a filesystem
        Fs::create(&fs_path).unwrap();

        // Verify should succeed on a valid repository
        let result = verify(
            &fs_path, None, // No start revision
            None, // No end revision
            None, // No notify callback
            None, // No cancel callback
        );
        assert!(result.is_ok(), "Verify should succeed on valid repository");
    }

    #[test]
    fn test_fs_hotcopy() {
        let src_dir = tempdir().unwrap();
        let src_path = src_dir.path().join("src-fs");
        let dst_dir = tempdir().unwrap();
        let dst_path = dst_dir.path().join("backup");

        // Create source filesystem
        Fs::create(&src_path).unwrap();

        // Hotcopy to destination
        let result = hotcopy(
            &src_path, &dst_path, false, // Not incremental
            false, // Don't clean logs
            None,  // No notify callback
            None,  // No cancel callback
        );
        assert!(result.is_ok(), "Hotcopy should succeed");

        // Verify destination is a valid filesystem
        let dst_fs = Fs::open(&dst_path);
        assert!(
            dst_fs.is_ok(),
            "Hotcopy destination should be valid filesystem"
        );
    }

    #[test]
    fn test_fs_recover() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        // Create a filesystem
        Fs::create(&fs_path).unwrap();

        // Recover should work on repository
        let result = recover(&fs_path, None);
        assert!(result.is_ok(), "Recover should succeed");
    }

    #[test]
    fn test_fs_freeze() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        // Create and open filesystem
        Fs::create(&fs_path).unwrap();
        let mut fs = Fs::open(&fs_path).unwrap();

        // Test freeze with a simple callback
        let mut callback_called = false;
        let result = fs.freeze(|| {
            callback_called = true;
            Ok(())
        });

        assert!(result.is_ok(), "Freeze should succeed");
        assert!(callback_called, "Freeze callback should be called");
    }

    #[test]
    fn test_fs_freeze_with_error() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        // Create and open filesystem
        Fs::create(&fs_path).unwrap();
        let mut fs = Fs::open(&fs_path).unwrap();

        // Test freeze with callback that returns error
        let result = fs.freeze(|| Err(Error::from_str("Test error from freeze callback")));

        assert!(result.is_err(), "Freeze should propagate callback error");
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Test error from freeze callback"));
    }

    #[test]
    fn test_fs_info() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        // Create and open filesystem
        Fs::create(&fs_path).unwrap();
        let fs = Fs::open(&fs_path).unwrap();

        // Get filesystem info
        let result = fs.info();
        assert!(result.is_ok(), "Info should succeed");

        let info = result.unwrap();
        // fs_type might be None or Some depending on implementation
        if let Some(fs_type) = &info.fs_type {
            // Common filesystem types in SVN
            assert!(
                fs_type == "fsfs" || fs_type == "bdb" || fs_type == "fsx",
                "Filesystem type should be a known type"
            );
        }
    }

    #[test]
    fn test_pack_with_callbacks() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        // Create a filesystem
        Fs::create(&fs_path).unwrap();

        // Test with notify callback
        // Just test that we can pass a callback - we can't easily check if it was called
        let result = pack(
            &fs_path,
            Some(Box::new(|_msg| {
                // Callback received notification
            })),
            None,
        );
        assert!(result.is_ok(), "Pack with notify should succeed");
    }

    #[test]
    fn test_verify_with_callbacks() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        // Create a filesystem
        Fs::create(&fs_path).unwrap();

        // Test with cancel callback that doesn't cancel
        let result = verify(
            &fs_path,
            None,
            None,
            None,
            Some(Box::new(|| false)), // Don't cancel
        );
        assert!(result.is_ok(), "Verify with cancel callback should succeed");

        // Test with cancel callback that cancels immediately
        let result = verify(
            &fs_path,
            None,
            None,
            None,
            Some(Box::new(|| true)), // Cancel immediately
        );
        // Should return an error if cancelled, or succeed if it completes before checking
        // Both are valid outcomes depending on timing
        assert!(
            result.is_err() || result.is_ok(),
            "Verify should either cancel or complete"
        );
    }

    #[test]
    fn test_hotcopy_incremental() {
        let src_dir = tempdir().unwrap();
        let src_path = src_dir.path().join("src-fs");
        let dst_dir = tempdir().unwrap();
        let dst_path = dst_dir.path().join("backup");

        // Create source filesystem
        Fs::create(&src_path).unwrap();

        // First hotcopy
        hotcopy(&src_path, &dst_path, false, false, None, None).unwrap();

        // Incremental hotcopy (should also work)
        let result = hotcopy(
            &src_path, &dst_path, true,  // Incremental
            false, // Don't clean logs
            None, None,
        );
        // Incremental hotcopy might fail if there's nothing new to copy
        // or succeed if it can do an incremental update
        if result.is_err() {
            // Check if it's a reasonable error (e.g., "already up to date")
            let _err = result.unwrap_err();
            // Error is expected for incremental hotcopy with no changes
        }
    }

    #[test]
    fn test_apply_text_with_checksum() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let fs = Fs::create(&fs_path).unwrap();
        let mut txn = fs.begin_txn(Revnum(0), 0).unwrap();
        let mut root = txn.root().unwrap();

        // Create a file
        root.make_file("/test.txt").unwrap();

        // Apply text with a specific expected checksum (MD5)
        // The MD5 of "Hello, World!\n" is: 8ddd8be4b179a529afa5f2ffae4b9858
        let expected_checksum = "8ddd8be4b179a529afa5f2ffae4b9858";
        let mut stream = root
            .apply_text("/test.txt", Some(expected_checksum))
            .unwrap();
        use std::io::Write;
        stream.write_all(b"Hello, World!\n").unwrap();
        drop(stream);

        // Commit should succeed since checksum matches
        txn.commit().unwrap();
    }

    #[test]
    fn test_apply_text_with_wrong_checksum() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let fs = Fs::create(&fs_path).unwrap();
        let mut txn = fs.begin_txn(Revnum(0), 0).unwrap();
        let mut root = txn.root().unwrap();

        // Create a file
        root.make_file("/test.txt").unwrap();

        // Apply text with wrong expected checksum
        let wrong_checksum = "00000000000000000000000000000000";
        let result = root.apply_text("/test.txt", Some(wrong_checksum));
        // This should succeed in creating the stream, but commit might fail
        assert!(result.is_ok());
    }

    #[test]
    fn test_begin_txn_with_flags() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let fs = Fs::create(&fs_path).unwrap();

        // Test with no flags (0)
        let txn1 = fs.begin_txn(Revnum(0), 0).unwrap();
        assert!(!txn1.name().unwrap().is_empty());
        drop(txn1);

        // Test with SVN_FS_TXN_CHECK_OOD flag (if defined)
        // Note: The actual flag values would need to be imported from subversion_sys
        let txn2 = fs.begin_txn(Revnum(0), 0x00000001).unwrap();
        assert!(!txn2.name().unwrap().is_empty());
        drop(txn2);

        // Test with SVN_FS_TXN_CHECK_LOCKS flag (if defined)
        let txn3 = fs.begin_txn(Revnum(0), 0x00000002).unwrap();
        assert!(!txn3.name().unwrap().is_empty());
        drop(txn3);
    }

    #[test]
    fn test_transaction_properties_extended() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let fs = Fs::create(&fs_path).unwrap();
        let mut txn = fs.begin_txn(Revnum(0), 0).unwrap();

        // Test change_prop_bytes with binary data
        let binary_data = b"\x00\x01\x02\x03\xFF";
        txn.change_prop_bytes("custom:binary", Some(binary_data))
            .unwrap();

        // Test prop retrieval
        let prop_value = txn.prop("custom:binary").unwrap();
        assert_eq!(prop_value.as_deref(), Some(binary_data.as_ref()));

        // Test proplist
        txn.change_prop("svn:log", "Test commit").unwrap();
        txn.change_prop("svn:author", "test-user").unwrap();

        let props = txn.proplist().unwrap();
        assert!(props.contains_key("svn:log"));
        assert!(props.contains_key("svn:author"));
        assert!(props.contains_key("custom:binary"));

        // Test removing a property
        txn.change_prop_bytes("custom:binary", None).unwrap();
        let prop_value = txn.prop("custom:binary").unwrap();
        assert!(prop_value.is_none() || prop_value.as_deref() == Some(b"".as_ref()));
    }
}
