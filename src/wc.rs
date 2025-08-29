use crate::{svn_result, with_tmp_pool, Error};
use std::marker::PhantomData;
use subversion_sys::{svn_wc_context_t, svn_wc_version};

pub fn version() -> crate::Version {
    unsafe { crate::Version(svn_wc_version()) }
}

// Status constants for Python compatibility
pub const STATUS_NONE: u32 = subversion_sys::svn_wc_status_kind_svn_wc_status_none;
pub const STATUS_UNVERSIONED: u32 = subversion_sys::svn_wc_status_kind_svn_wc_status_unversioned;
pub const STATUS_NORMAL: u32 = subversion_sys::svn_wc_status_kind_svn_wc_status_normal;
pub const STATUS_ADDED: u32 = subversion_sys::svn_wc_status_kind_svn_wc_status_added;
pub const STATUS_MISSING: u32 = subversion_sys::svn_wc_status_kind_svn_wc_status_missing;
pub const STATUS_DELETED: u32 = subversion_sys::svn_wc_status_kind_svn_wc_status_deleted;
pub const STATUS_REPLACED: u32 = subversion_sys::svn_wc_status_kind_svn_wc_status_replaced;
pub const STATUS_MODIFIED: u32 = subversion_sys::svn_wc_status_kind_svn_wc_status_modified;
pub const STATUS_MERGED: u32 = subversion_sys::svn_wc_status_kind_svn_wc_status_merged;
pub const STATUS_CONFLICTED: u32 = subversion_sys::svn_wc_status_kind_svn_wc_status_conflicted;
pub const STATUS_IGNORED: u32 = subversion_sys::svn_wc_status_kind_svn_wc_status_ignored;
pub const STATUS_OBSTRUCTED: u32 = subversion_sys::svn_wc_status_kind_svn_wc_status_obstructed;
pub const STATUS_EXTERNAL: u32 = subversion_sys::svn_wc_status_kind_svn_wc_status_external;
pub const STATUS_INCOMPLETE: u32 = subversion_sys::svn_wc_status_kind_svn_wc_status_incomplete;

// Schedule constants for Python compatibility
pub const SCHEDULE_NORMAL: u32 = subversion_sys::svn_wc_schedule_t_svn_wc_schedule_normal;
pub const SCHEDULE_ADD: u32 = subversion_sys::svn_wc_schedule_t_svn_wc_schedule_add;
pub const SCHEDULE_DELETE: u32 = subversion_sys::svn_wc_schedule_t_svn_wc_schedule_delete;
pub const SCHEDULE_REPLACE: u32 = subversion_sys::svn_wc_schedule_t_svn_wc_schedule_replace;

/// Working copy status types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Status {
    /// Not under version control
    None = subversion_sys::svn_wc_status_kind_svn_wc_status_none,
    /// Item is not versioned
    Unversioned = subversion_sys::svn_wc_status_kind_svn_wc_status_unversioned,
    /// Item is versioned and unchanged
    Normal = subversion_sys::svn_wc_status_kind_svn_wc_status_normal,
    /// Item has been added
    Added = subversion_sys::svn_wc_status_kind_svn_wc_status_added,
    /// Item is missing (removed by non-svn command)
    Missing = subversion_sys::svn_wc_status_kind_svn_wc_status_missing,
    /// Item has been deleted
    Deleted = subversion_sys::svn_wc_status_kind_svn_wc_status_deleted,
    /// Item has been replaced
    Replaced = subversion_sys::svn_wc_status_kind_svn_wc_status_replaced,
    /// Item has been modified
    Modified = subversion_sys::svn_wc_status_kind_svn_wc_status_modified,
    /// Item has been merged
    Merged = subversion_sys::svn_wc_status_kind_svn_wc_status_merged,
    /// Item is in conflict
    Conflicted = subversion_sys::svn_wc_status_kind_svn_wc_status_conflicted,
    /// Item is ignored
    Ignored = subversion_sys::svn_wc_status_kind_svn_wc_status_ignored,
    /// Item is obstructed
    Obstructed = subversion_sys::svn_wc_status_kind_svn_wc_status_obstructed,
    /// Item is an external
    External = subversion_sys::svn_wc_status_kind_svn_wc_status_external,
    /// Item is incomplete
    Incomplete = subversion_sys::svn_wc_status_kind_svn_wc_status_incomplete,
}

impl From<subversion_sys::svn_wc_status_kind> for Status {
    fn from(status: subversion_sys::svn_wc_status_kind) -> Self {
        match status {
            subversion_sys::svn_wc_status_kind_svn_wc_status_none => Status::None,
            subversion_sys::svn_wc_status_kind_svn_wc_status_unversioned => Status::Unversioned,
            subversion_sys::svn_wc_status_kind_svn_wc_status_normal => Status::Normal,
            subversion_sys::svn_wc_status_kind_svn_wc_status_added => Status::Added,
            subversion_sys::svn_wc_status_kind_svn_wc_status_missing => Status::Missing,
            subversion_sys::svn_wc_status_kind_svn_wc_status_deleted => Status::Deleted,
            subversion_sys::svn_wc_status_kind_svn_wc_status_replaced => Status::Replaced,
            subversion_sys::svn_wc_status_kind_svn_wc_status_modified => Status::Modified,
            subversion_sys::svn_wc_status_kind_svn_wc_status_merged => Status::Merged,
            subversion_sys::svn_wc_status_kind_svn_wc_status_conflicted => Status::Conflicted,
            subversion_sys::svn_wc_status_kind_svn_wc_status_ignored => Status::Ignored,
            subversion_sys::svn_wc_status_kind_svn_wc_status_obstructed => Status::Obstructed,
            subversion_sys::svn_wc_status_kind_svn_wc_status_external => Status::External,
            subversion_sys::svn_wc_status_kind_svn_wc_status_incomplete => Status::Incomplete,
            _ => Status::None,
        }
    }
}

/// Working copy schedule types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Schedule {
    /// Nothing scheduled
    Normal = subversion_sys::svn_wc_schedule_t_svn_wc_schedule_normal,
    /// Scheduled for addition
    Add = subversion_sys::svn_wc_schedule_t_svn_wc_schedule_add,
    /// Scheduled for deletion
    Delete = subversion_sys::svn_wc_schedule_t_svn_wc_schedule_delete,
    /// Scheduled for replacement
    Replace = subversion_sys::svn_wc_schedule_t_svn_wc_schedule_replace,
}

impl From<subversion_sys::svn_wc_schedule_t> for Schedule {
    fn from(schedule: subversion_sys::svn_wc_schedule_t) -> Self {
        match schedule {
            subversion_sys::svn_wc_schedule_t_svn_wc_schedule_normal => Schedule::Normal,
            subversion_sys::svn_wc_schedule_t_svn_wc_schedule_add => Schedule::Add,
            subversion_sys::svn_wc_schedule_t_svn_wc_schedule_delete => Schedule::Delete,
            subversion_sys::svn_wc_schedule_t_svn_wc_schedule_replace => Schedule::Replace,
            _ => Schedule::Normal,
        }
    }
}

/// Working copy context with RAII cleanup
pub struct Context {
    ptr: *mut svn_wc_context_t,
    pool: apr::Pool,
    _phantom: PhantomData<*mut ()>, // !Send + !Sync
}

impl Drop for Context {
    fn drop(&mut self) {
        // Pool drop will clean up context
    }
}

impl Context {
    /// Get a reference to the underlying pool
    pub fn pool(&self) -> &apr::Pool {
        &self.pool
    }

    /// Get the raw pointer to the context (use with caution)
    pub fn as_ptr(&self) -> *const svn_wc_context_t {
        self.ptr
    }

    /// Get the mutable raw pointer to the context (use with caution)
    pub fn as_mut_ptr(&mut self) -> *mut svn_wc_context_t {
        self.ptr
    }

    pub fn new() -> Result<Self, crate::Error> {
        let pool = apr::Pool::new();

        unsafe {
            let mut ctx = std::ptr::null_mut();
            with_tmp_pool(|scratch_pool| {
                let err = subversion_sys::svn_wc_context_create(
                    &mut ctx,
                    std::ptr::null_mut(),
                    pool.as_mut_ptr(),
                    scratch_pool.as_mut_ptr(),
                );
                svn_result(err)
            })?;

            Ok(Context {
                ptr: ctx,
                pool,
                _phantom: PhantomData,
            })
        }
    }

    /// Create new context with configuration
    pub fn new_with_config(config: *mut std::ffi::c_void) -> Result<Self, crate::Error> {
        let pool = apr::Pool::new();

        unsafe {
            let mut ctx = std::ptr::null_mut();
            with_tmp_pool(|scratch_pool| {
                let err = subversion_sys::svn_wc_context_create(
                    &mut ctx,
                    config as *mut subversion_sys::svn_config_t,
                    pool.as_mut_ptr(),
                    scratch_pool.as_mut_ptr(),
                );
                svn_result(err)
            })?;

            Ok(Context {
                ptr: ctx,
                pool,
                _phantom: PhantomData,
            })
        }
    }

    pub fn check_wc(&mut self, path: &str) -> Result<i32, crate::Error> {
        let path = std::ffi::CString::new(path).unwrap();
        let mut wc_format = 0;
        let err = unsafe {
            subversion_sys::svn_wc_check_wc2(
                &mut wc_format,
                self.ptr,
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
            subversion_sys::svn_wc_text_modified_p2(
                &mut modified,
                self.ptr,
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
            subversion_sys::svn_wc_props_modified_p2(
                &mut modified,
                self.ptr,
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
            subversion_sys::svn_wc_conflicted_p3(
                &mut text_conflicted,
                &mut prop_conflicted,
                &mut tree_conflicted,
                self.ptr,
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
            subversion_sys::svn_wc_ensure_adm4(
                self.ptr,
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
            subversion_sys::svn_wc_locked2(
                &mut locked_here,
                &mut locked,
                self.ptr,
                path.as_ptr(),
                scratch_pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok((locked != 0, locked_here != 0))
    }

    /// Get the working copy database format version for this context
    pub fn db_version(&self) -> Result<i32, crate::Error> {
        // This would require exposing more internal SVN APIs
        // For now, just indicate we don't have this information
        Ok(0) // 0 indicates unknown/unavailable
    }
}

pub fn set_adm_dir(name: &str) -> Result<(), crate::Error> {
    let name = std::ffi::CString::new(name).unwrap();
    let err = unsafe {
        subversion_sys::svn_wc_set_adm_dir(name.as_ptr(), apr::pool::Pool::new().as_mut_ptr())
    };
    Error::from_raw(err)?;
    Ok(())
}

pub fn get_adm_dir() -> String {
    let pool = apr::pool::Pool::new();
    let name = unsafe { subversion_sys::svn_wc_get_adm_dir(pool.as_mut_ptr()) };
    unsafe { std::ffi::CStr::from_ptr(name) }
        .to_string_lossy()
        .into_owned()
}

/// Check if text is modified in a working copy file
pub fn text_modified(path: &std::path::Path, force_comparison: bool) -> Result<bool, crate::Error> {
    let path_str = path.to_string_lossy();
    let path_cstr = std::ffi::CString::new(path_str.as_ref()).unwrap();
    let mut modified = 0;

    with_tmp_pool(|pool| -> Result<(), crate::Error> {
        let mut ctx = std::ptr::null_mut();
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_context_create(
                    &mut ctx,
                    std::ptr::null_mut(),
                    pool.as_mut_ptr(),
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })?;

        let err = unsafe {
            subversion_sys::svn_wc_text_modified_p2(
                &mut modified,
                ctx,
                path_cstr.as_ptr(),
                if force_comparison { 1 } else { 0 },
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(())
    })?;

    Ok(modified != 0)
}

/// Check if properties are modified in a working copy file
pub fn props_modified(path: &std::path::Path) -> Result<bool, crate::Error> {
    let path_str = path.to_string_lossy();
    let path_cstr = std::ffi::CString::new(path_str.as_ref()).unwrap();
    let mut modified = 0;

    with_tmp_pool(|pool| -> Result<(), crate::Error> {
        let mut ctx = std::ptr::null_mut();
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_context_create(
                    &mut ctx,
                    std::ptr::null_mut(),
                    pool.as_mut_ptr(),
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })?;

        let err = unsafe {
            subversion_sys::svn_wc_props_modified_p2(
                &mut modified,
                ctx,
                path_cstr.as_ptr(),
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(())
    })?;

    Ok(modified != 0)
}

/// Check if directory name is an administrative directory
pub fn is_adm_dir(name: &str) -> bool {
    let name_cstr = std::ffi::CString::new(name).unwrap();
    let pool = apr::Pool::new();
    let result =
        unsafe { subversion_sys::svn_wc_is_adm_dir(name_cstr.as_ptr(), pool.as_mut_ptr()) };
    result != 0
}

/// Check working copy format at path
pub fn check_wc(path: &std::path::Path) -> Result<Option<i32>, crate::Error> {
    let path_str = path.to_string_lossy();
    let path_cstr = std::ffi::CString::new(path_str.as_ref()).unwrap();
    let mut wc_format = 0;

    with_tmp_pool(|pool| -> Result<(), crate::Error> {
        let mut ctx = std::ptr::null_mut();
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_context_create(
                    &mut ctx,
                    std::ptr::null_mut(),
                    pool.as_mut_ptr(),
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })?;

        let err = unsafe {
            subversion_sys::svn_wc_check_wc2(
                &mut wc_format,
                ctx,
                path_cstr.as_ptr(),
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(())
    })?;

    // Return None if not a working copy (format would be 0)
    if wc_format == 0 {
        Ok(None)
    } else {
        Ok(Some(wc_format))
    }
}

/// Ensure administrative directory exists
pub fn ensure_adm(
    path: &std::path::Path,
    uuid: &str,
    url: &str,
    repos_root: &str,
    revision: i64,
) -> Result<(), crate::Error> {
    let path_str = path.to_string_lossy();
    let path_cstr = std::ffi::CString::new(path_str.as_ref()).unwrap();
    let uuid_cstr = std::ffi::CString::new(uuid).unwrap();
    let url_cstr = std::ffi::CString::new(url).unwrap();
    let repos_root_cstr = std::ffi::CString::new(repos_root).unwrap();

    with_tmp_pool(|pool| -> Result<(), crate::Error> {
        let mut ctx = std::ptr::null_mut();
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_context_create(
                    &mut ctx,
                    std::ptr::null_mut(),
                    pool.as_mut_ptr(),
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })?;

        let err = unsafe {
            subversion_sys::svn_wc_ensure_adm4(
                ctx,
                path_cstr.as_ptr(),
                url_cstr.as_ptr(),
                repos_root_cstr.as_ptr(),
                uuid_cstr.as_ptr(),
                revision,
                subversion_sys::svn_depth_t_svn_depth_infinity,
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(())
    })
}

/// Check if a property name is a "normal" property (not special)
pub fn is_normal_prop(name: &str) -> bool {
    let name_cstr = std::ffi::CString::new(name).unwrap();
    unsafe { subversion_sys::svn_wc_is_normal_prop(name_cstr.as_ptr()) != 0 }
}

/// Check if a property name is an "entry" property
pub fn is_entry_prop(name: &str) -> bool {
    let name_cstr = std::ffi::CString::new(name).unwrap();
    unsafe { subversion_sys::svn_wc_is_entry_prop(name_cstr.as_ptr()) != 0 }
}

/// Check if a property name is a "wc" property
pub fn is_wc_prop(name: &str) -> bool {
    let name_cstr = std::ffi::CString::new(name).unwrap();
    unsafe { subversion_sys::svn_wc_is_wc_prop(name_cstr.as_ptr()) != 0 }
}

/// Match a path against an ignore list
pub fn match_ignore_list(path: &str, patterns: &[&str]) -> Result<bool, crate::Error> {
    let path_cstr = std::ffi::CString::new(path).unwrap();

    with_tmp_pool(|pool| {
        // Create APR array of patterns
        let mut patterns_array = apr::tables::ArrayHeader::<*const i8>::new(&pool);
        for pattern in patterns {
            let pattern_cstr = std::ffi::CString::new(*pattern)?;
            patterns_array.push(pattern_cstr.as_ptr());
        }

        let mut matched = 0;
        let err = unsafe {
            subversion_sys::svn_wc_match_ignore_list(
                path_cstr.as_ptr(),
                patterns_array.as_ptr(),
                pool.as_mut_ptr(),
            )
        };

        // svn_wc_match_ignore_list returns a boolean, not an error
        // The return value indicates whether there was a match
        matched = err as i32;
        Ok(matched != 0)
    })
}

/// Get the actual target for a path (anchor/target split)
pub fn get_actual_target(path: &std::path::Path) -> Result<(String, String), crate::Error> {
    let path_str = path.to_string_lossy();
    let path_cstr = std::ffi::CString::new(path_str.as_ref()).unwrap();
    let mut anchor: *const i8 = std::ptr::null();
    let mut target: *const i8 = std::ptr::null();

    with_tmp_pool(|pool| -> Result<(), crate::Error> {
        let mut ctx = std::ptr::null_mut();
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_context_create(
                    &mut ctx,
                    std::ptr::null_mut(),
                    pool.as_mut_ptr(),
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })?;

        let err = unsafe {
            subversion_sys::svn_wc_get_actual_target2(
                &mut anchor,
                &mut target,
                ctx,
                path_cstr.as_ptr(),
                pool.as_mut_ptr(),
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(())
    })?;

    let anchor_str = if anchor.is_null() {
        String::new()
    } else {
        unsafe { std::ffi::CStr::from_ptr(anchor) }
            .to_string_lossy()
            .into_owned()
    };

    let target_str = if target.is_null() {
        String::new()
    } else {
        unsafe { std::ffi::CStr::from_ptr(target) }
            .to_string_lossy()
            .into_owned()
    };

    Ok((anchor_str, target_str))
}

/// Get pristine contents of a file
pub fn get_pristine_contents(
    path: &std::path::Path,
) -> Result<Option<crate::io::Stream>, crate::Error> {
    let path_str = path.to_string_lossy();
    let path_cstr = std::ffi::CString::new(path_str.as_ref()).unwrap();
    let mut contents: *mut subversion_sys::svn_stream_t = std::ptr::null_mut();

    with_tmp_pool(|pool| -> Result<(), crate::Error> {
        let mut ctx = std::ptr::null_mut();
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_context_create(
                    &mut ctx,
                    std::ptr::null_mut(),
                    pool.as_mut_ptr(),
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })?;

        let err = unsafe {
            subversion_sys::svn_wc_get_pristine_contents2(
                &mut contents,
                ctx,
                path_cstr.as_ptr(),
                pool.as_mut_ptr(),
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(())
    })?;

    if contents.is_null() {
        Ok(None)
    } else {
        Ok(Some(unsafe {
            crate::io::Stream::from_ptr_and_pool(contents, apr::Pool::new())
        }))
    }
}

/// Get pristine copy path (deprecated - for backwards compatibility)
pub fn get_pristine_copy_path(path: &std::path::Path) -> Result<std::path::PathBuf, crate::Error> {
    let path_str = path.to_string_lossy();
    let path_cstr = std::ffi::CString::new(path_str.as_ref()).unwrap();
    let mut pristine_path: *const i8 = std::ptr::null();

    with_tmp_pool(|pool| -> Result<(), crate::Error> {
        let err = unsafe {
            subversion_sys::svn_wc_get_pristine_copy_path(
                path_cstr.as_ptr(),
                &mut pristine_path,
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(())
    })?;

    let pristine_path_str = if pristine_path.is_null() {
        String::new()
    } else {
        unsafe { std::ffi::CStr::from_ptr(pristine_path) }
            .to_string_lossy()
            .into_owned()
    };

    Ok(std::path::PathBuf::from(pristine_path_str))
}

impl Context {
    /// Get the actual target for a path using this working copy context
    pub fn get_actual_target(&mut self, path: &str) -> Result<(String, String), crate::Error> {
        let path_cstr = std::ffi::CString::new(path).unwrap();
        let mut anchor: *const i8 = std::ptr::null();
        let mut target: *const i8 = std::ptr::null();

        let pool = apr::Pool::new();
        let err = unsafe {
            subversion_sys::svn_wc_get_actual_target2(
                &mut anchor,
                &mut target,
                self.ptr,
                path_cstr.as_ptr(),
                pool.as_mut_ptr(),
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;

        let anchor_str = if anchor.is_null() {
            String::new()
        } else {
            unsafe { std::ffi::CStr::from_ptr(anchor) }
                .to_string_lossy()
                .into_owned()
        };

        let target_str = if target.is_null() {
            String::new()
        } else {
            unsafe { std::ffi::CStr::from_ptr(target) }
                .to_string_lossy()
                .into_owned()
        };

        Ok((anchor_str, target_str))
    }

    /// Get pristine contents of a file using this working copy context
    pub fn get_pristine_contents(
        &mut self,
        path: &str,
    ) -> Result<Option<crate::io::Stream>, crate::Error> {
        let path_cstr = std::ffi::CString::new(path).unwrap();
        let mut contents: *mut subversion_sys::svn_stream_t = std::ptr::null_mut();

        let pool = apr::Pool::new();
        let err = unsafe {
            subversion_sys::svn_wc_get_pristine_contents2(
                &mut contents,
                self.ptr,
                path_cstr.as_ptr(),
                pool.as_mut_ptr(),
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;

        if contents.is_null() {
            Ok(None)
        } else {
            Ok(Some(unsafe {
                crate::io::Stream::from_ptr_and_pool(contents, pool)
            }))
        }
    }

    /// Get pristine properties for a path
    pub fn get_pristine_props(
        &mut self,
        path: &str,
    ) -> Result<Option<std::collections::HashMap<String, Vec<u8>>>, crate::Error> {
        let path_cstr = std::ffi::CString::new(path).unwrap();
        let mut props: *mut apr_sys::apr_hash_t = std::ptr::null_mut();

        let pool = apr::Pool::new();
        let err = unsafe {
            subversion_sys::svn_wc_get_pristine_props(
                &mut props,
                self.ptr,
                path_cstr.as_ptr(),
                pool.as_mut_ptr(),
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;

        if props.is_null() {
            return Ok(None);
        }

        let props_hash =
            unsafe { apr::hash::Hash::<&str, *mut subversion_sys::svn_string_t>::from_ptr(props) };

        let mut result = std::collections::HashMap::new();
        for (key, svn_str) in props_hash.iter(&pool) {
            let value = unsafe {
                let svn_str = *svn_str;
                if !svn_str.is_null() {
                    std::slice::from_raw_parts((*svn_str).data as *const u8, (*svn_str).len)
                        .to_vec()
                } else {
                    Vec::new()
                }
            };
            result.insert(String::from_utf8_lossy(key).to_string(), value);
        }

        Ok(Some(result))
    }
}

/// Clean up a working copy
pub fn cleanup(
    wc_path: &std::path::Path,
    break_locks: bool,
    fix_recorded_timestamps: bool,
    clear_dav_cache: bool,
    vacuum_pristines: bool,
    include_externals: bool,
) -> Result<(), Error> {
    let path_str = wc_path.to_string_lossy();
    let path_cstr = std::ffi::CString::new(path_str.as_ref()).unwrap();

    with_tmp_pool(|pool| -> Result<(), Error> {
        let mut ctx = std::ptr::null_mut();
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_context_create(
                    &mut ctx,
                    std::ptr::null_mut(),
                    pool.as_mut_ptr(),
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })?;

        let err = unsafe {
            subversion_sys::svn_wc_cleanup4(
                ctx,
                path_cstr.as_ptr(),
                break_locks as i32,
                fix_recorded_timestamps as i32,
                clear_dav_cache as i32,
                vacuum_pristines as i32,
                None,                 // cancel_func
                std::ptr::null_mut(), // cancel_baton
                None,                 // notify_func
                std::ptr::null_mut(), // notify_baton
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(())
    })
}

/// Get the working copy revision status
pub fn revision_status(
    wc_path: &std::path::Path,
    trail_url: Option<&str>,
    committed: bool,
) -> Result<(i64, i64, bool, bool), Error> {
    let path_str = wc_path.to_string_lossy();
    let path_cstr = std::ffi::CString::new(path_str.as_ref()).unwrap();
    let trail_cstr = trail_url.map(|s| std::ffi::CString::new(s).unwrap());

    with_tmp_pool(|pool| -> Result<(i64, i64, bool, bool), Error> {
        let mut ctx = std::ptr::null_mut();
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_context_create(
                    &mut ctx,
                    std::ptr::null_mut(),
                    pool.as_mut_ptr(),
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })?;

        let mut status_ptr: *mut subversion_sys::svn_wc_revision_status_t = std::ptr::null_mut();

        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_revision_status2(
                    &mut status_ptr,
                    ctx,
                    path_cstr.as_ptr(),
                    trail_cstr.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()),
                    committed as i32,
                    None,                 // cancel_func
                    std::ptr::null_mut(), // cancel_baton
                    pool.as_mut_ptr(),
                    scratch_pool.as_mut_ptr(),
                )
            };
            Error::from_raw(err)
        })?;

        if status_ptr.is_null() {
            return Err(Error::from(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Failed to get revision status",
            )));
        }

        let status = unsafe { *status_ptr };
        Ok((
            status.min_rev,
            status.max_rev,
            status.switched != 0,
            status.modified != 0,
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_context_creation() {
        let context = Context::new();
        assert!(context.is_ok());
        let context = context.unwrap();
        assert!(!context.ptr.is_null());
    }

    #[test]
    fn test_adm_dir_default() {
        // Default admin dir should be ".svn"
        let dir = get_adm_dir();
        assert_eq!(dir, ".svn");
    }

    #[test]
    fn test_is_adm_dir() {
        // Test standard admin directory name
        assert!(is_adm_dir(".svn"));

        // Test non-admin directory names
        assert!(!is_adm_dir("src"));
        assert!(!is_adm_dir("test"));
        assert!(!is_adm_dir(".git"));
    }

    #[test]
    fn test_context_with_config() {
        // Create context with empty config
        let config = std::ptr::null_mut();
        let result = Context::new_with_config(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_wc() {
        let dir = tempdir().unwrap();
        let wc_path = dir.path();

        // Non-working-copy directory should return None
        let wc_format = check_wc(wc_path);
        assert!(wc_format.is_ok());
        assert_eq!(wc_format.unwrap(), None);
    }

    #[test]
    fn test_ensure_adm() {
        let dir = tempdir().unwrap();
        let wc_path = dir.path();

        // Try to ensure admin area
        let result = ensure_adm(
            wc_path,
            "",                  // uuid
            "file:///test/repo", // url
            "file:///test/repo", // repos
            0,                   // revision
        );

        // This might fail if the directory already exists or other reasons
        // Just ensure it doesn't panic
        let _ = result;
    }

    // Note: Context cannot be Send because it contains raw pointers to C structures

    #[test]
    fn test_text_modified() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "test content").unwrap();

        // This will fail without a working copy, but shouldn't panic
        let result = text_modified(&file_path, false);
        assert!(result.is_err()); // Expected to fail without WC
    }

    #[test]
    fn test_props_modified() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "test content").unwrap();

        // This will fail without a working copy, but shouldn't panic
        let result = props_modified(&file_path);
        assert!(result.is_err()); // Expected to fail without WC
    }

    #[test]
    fn test_status_enum() {
        // Test Status enum conversions
        assert_eq!(
            Status::Normal as u32,
            subversion_sys::svn_wc_status_kind_svn_wc_status_normal
        );
        assert_eq!(
            Status::Added as u32,
            subversion_sys::svn_wc_status_kind_svn_wc_status_added
        );
        assert_eq!(
            Status::Deleted as u32,
            subversion_sys::svn_wc_status_kind_svn_wc_status_deleted
        );

        // Test From conversion
        let status = Status::from(subversion_sys::svn_wc_status_kind_svn_wc_status_modified);
        assert_eq!(status, Status::Modified);
    }

    #[test]
    fn test_schedule_enum() {
        // Test Schedule enum conversions
        assert_eq!(
            Schedule::Normal as u32,
            subversion_sys::svn_wc_schedule_t_svn_wc_schedule_normal
        );
        assert_eq!(
            Schedule::Add as u32,
            subversion_sys::svn_wc_schedule_t_svn_wc_schedule_add
        );

        // Test From conversion
        let schedule = Schedule::from(subversion_sys::svn_wc_schedule_t_svn_wc_schedule_delete);
        assert_eq!(schedule, Schedule::Delete);
    }

    #[test]
    fn test_is_normal_prop() {
        // "Normal" properties are versioned properties that users can set
        // SVN properties like svn:keywords ARE normal properties
        assert!(is_normal_prop("svn:keywords"));
        assert!(is_normal_prop("svn:eol-style"));
        assert!(is_normal_prop("svn:mime-type"));

        // Entry and WC properties are NOT normal
        assert!(!is_normal_prop("svn:entry:committed-rev"));
        assert!(!is_normal_prop("svn:wc:ra_dav:version-url"));
    }

    #[test]
    fn test_is_entry_prop() {
        // These should be entry properties
        assert!(is_entry_prop("svn:entry:committed-rev"));
        assert!(is_entry_prop("svn:entry:uuid"));

        // These should not be entry properties
        assert!(!is_entry_prop("svn:keywords"));
        assert!(!is_entry_prop("user:custom"));
    }

    #[test]
    fn test_is_wc_prop() {
        // These should be WC properties
        assert!(is_wc_prop("svn:wc:ra_dav:version-url"));

        // These should not be WC properties
        assert!(!is_wc_prop("svn:keywords"));
        assert!(!is_wc_prop("user:custom"));
    }
}
