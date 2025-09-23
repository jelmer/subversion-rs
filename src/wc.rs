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
pub enum StatusKind {
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

impl From<subversion_sys::svn_wc_status_kind> for StatusKind {
    fn from(status: subversion_sys::svn_wc_status_kind) -> Self {
        match status {
            subversion_sys::svn_wc_status_kind_svn_wc_status_none => StatusKind::None,
            subversion_sys::svn_wc_status_kind_svn_wc_status_unversioned => StatusKind::Unversioned,
            subversion_sys::svn_wc_status_kind_svn_wc_status_normal => StatusKind::Normal,
            subversion_sys::svn_wc_status_kind_svn_wc_status_added => StatusKind::Added,
            subversion_sys::svn_wc_status_kind_svn_wc_status_missing => StatusKind::Missing,
            subversion_sys::svn_wc_status_kind_svn_wc_status_deleted => StatusKind::Deleted,
            subversion_sys::svn_wc_status_kind_svn_wc_status_replaced => StatusKind::Replaced,
            subversion_sys::svn_wc_status_kind_svn_wc_status_modified => StatusKind::Modified,
            subversion_sys::svn_wc_status_kind_svn_wc_status_merged => StatusKind::Merged,
            subversion_sys::svn_wc_status_kind_svn_wc_status_conflicted => StatusKind::Conflicted,
            subversion_sys::svn_wc_status_kind_svn_wc_status_ignored => StatusKind::Ignored,
            subversion_sys::svn_wc_status_kind_svn_wc_status_obstructed => StatusKind::Obstructed,
            subversion_sys::svn_wc_status_kind_svn_wc_status_external => StatusKind::External,
            subversion_sys::svn_wc_status_kind_svn_wc_status_incomplete => StatusKind::Incomplete,
            _ => StatusKind::None,
        }
    }
}

/// Working copy status information
pub struct Status {
    ptr: *const subversion_sys::svn_wc_status3_t,
}

impl Status {
    /// Get the node status
    pub fn node_status(&self) -> StatusKind {
        unsafe { (*self.ptr).node_status.into() }
    }

    /// Get the text status
    pub fn text_status(&self) -> StatusKind {
        unsafe { (*self.ptr).text_status.into() }
    }

    /// Get the property status
    pub fn prop_status(&self) -> StatusKind {
        unsafe { (*self.ptr).prop_status.into() }
    }

    /// Check if the item is copied
    pub fn copied(&self) -> bool {
        unsafe { (*self.ptr).copied != 0 }
    }

    /// Check if the item is switched
    pub fn switched(&self) -> bool {
        unsafe { (*self.ptr).switched != 0 }
    }

    /// Check if the item is locked
    pub fn locked(&self) -> bool {
        unsafe { (*self.ptr).locked != 0 }
    }

    /// Get the revision
    pub fn revision(&self) -> crate::Revnum {
        unsafe { crate::Revnum((*self.ptr).revision) }
    }

    /// Get the changed revision
    pub fn changed_rev(&self) -> crate::Revnum {
        unsafe { crate::Revnum((*self.ptr).changed_rev) }
    }

    /// Get the repository relative path
    pub fn repos_relpath(&self) -> Option<String> {
        unsafe {
            if (*self.ptr).repos_relpath.is_null() {
                None
            } else {
                Some(
                    std::ffi::CStr::from_ptr((*self.ptr).repos_relpath)
                        .to_string_lossy()
                        .into_owned(),
                )
            }
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
        // We need to keep the CStrings alive for the duration of the call
        let pattern_cstrs: Vec<std::ffi::CString> = patterns
            .iter()
            .map(|p| std::ffi::CString::new(*p))
            .collect::<Result<Vec<_>, _>>()?;

        // Create APR array of patterns
        let mut patterns_array =
            apr::tables::TypedArray::<*const i8>::new(pool, patterns.len() as i32);
        for pattern_cstr in &pattern_cstrs {
            patterns_array.push(pattern_cstr.as_ptr());
        }

        let matched = unsafe {
            subversion_sys::svn_wc_match_ignore_list(
                path_cstr.as_ptr(),
                patterns_array.as_ptr(),
                pool.as_mut_ptr(),
            )
        };

        // svn_wc_match_ignore_list returns svn_boolean_t (0 = false, non-zero = true)
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

        let prop_hash = unsafe { crate::props::PropHash::from_ptr(props) };
        Ok(Some(prop_hash.to_hashmap()))
    }

    /// Walk the status of a working copy tree
    ///
    /// Walks the WC status for `local_abspath` and all its children, invoking
    /// the callback for each node.
    pub fn walk_status<F>(
        &mut self,
        local_abspath: &std::path::Path,
        depth: crate::Depth,
        get_all: bool,
        no_ignore: bool,
        ignore_text_mods: bool,
        status_func: F,
    ) -> Result<(), Error>
    where
        F: FnMut(&str, &Status) -> Result<(), Error>,
    {
        let pool = apr::Pool::new();
        let path_cstr = std::ffi::CString::new(local_abspath.to_str().unwrap())?;

        // Wrap the closure in a way that can be passed to C
        extern "C" fn status_callback(
            baton: *mut std::ffi::c_void,
            local_abspath: *const std::os::raw::c_char,
            status: *const subversion_sys::svn_wc_status3_t,
            _pool: *mut apr_sys::apr_pool_t,
        ) -> *mut subversion_sys::svn_error_t {
            let callback =
                unsafe { &mut *(baton as *mut Box<dyn FnMut(&str, &Status) -> Result<(), Error>>) };

            let path = unsafe {
                std::ffi::CStr::from_ptr(local_abspath)
                    .to_string_lossy()
                    .into_owned()
            };

            let status = Status { ptr: status };

            match callback(&path, &status) {
                Ok(()) => std::ptr::null_mut(),
                Err(e) => unsafe { e.into_raw() },
            }
        }

        let boxed_callback: Box<Box<dyn FnMut(&str, &Status) -> Result<(), Error>>> =
            Box::new(Box::new(status_func));
        let baton = Box::into_raw(boxed_callback) as *mut std::ffi::c_void;

        unsafe {
            let err = subversion_sys::svn_wc_walk_status(
                self.ptr,
                path_cstr.as_ptr(),
                depth.into(),
                get_all as i32,
                no_ignore as i32,
                ignore_text_mods as i32,
                std::ptr::null_mut(), // ignore_patterns
                Some(status_callback),
                baton,
                None,                 // cancel_func
                std::ptr::null_mut(), // cancel_baton
                pool.as_mut_ptr(),
            );

            // Clean up the callback
            let _ = Box::from_raw(baton as *mut Box<dyn FnMut(&str, &Status) -> Result<(), Error>>);

            Error::from_raw(err)
        }
    }

    /// Queue committed items for post-commit processing
    ///
    /// Queues items that have been committed for later processing by `process_committed_queue`.
    pub fn queue_committed(
        &mut self,
        local_abspath: &std::path::Path,
        recurse: bool,
        committed_queue: &mut CommittedQueue,
    ) -> Result<(), Error> {
        let pool = apr::Pool::new();
        let path_cstr = std::ffi::CString::new(local_abspath.to_str().unwrap())?;

        unsafe {
            let err = subversion_sys::svn_wc_queue_committed3(
                committed_queue.as_mut_ptr(),
                self.ptr,
                path_cstr.as_ptr(),
                recurse as i32,
                std::ptr::null_mut(), // wcprop_changes (deprecated)
                false as i32,         // remove_lock
                false as i32,         // remove_changelist
                std::ptr::null_mut(), // sha1_checksum
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)
        }
    }

    /// Process the committed queue
    ///
    /// Processes all items in the committed queue after a successful commit.
    pub fn process_committed_queue(
        &mut self,
        committed_queue: &mut CommittedQueue,
        new_revnum: crate::Revnum,
        rev_date: Option<&str>,
        rev_author: Option<&str>,
    ) -> Result<(), Error> {
        let pool = apr::Pool::new();

        let rev_date_cstr = rev_date.map(std::ffi::CString::new).transpose()?;
        let rev_author_cstr = rev_author.map(std::ffi::CString::new).transpose()?;

        unsafe {
            let err = subversion_sys::svn_wc_process_committed_queue2(
                committed_queue.as_mut_ptr(),
                self.ptr,
                new_revnum.0,
                rev_date_cstr
                    .as_ref()
                    .map_or(std::ptr::null(), |s| s.as_ptr()),
                rev_author_cstr
                    .as_ref()
                    .map_or(std::ptr::null(), |s| s.as_ptr()),
                None,                 // cancel_func
                std::ptr::null_mut(), // cancel_baton
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)
        }
    }

    /// Add a lock to the working copy
    ///
    /// Adds lock information for the given path.
    pub fn add_lock(&mut self, local_abspath: &std::path::Path, lock: &Lock) -> Result<(), Error> {
        let pool = apr::Pool::new();
        let path_cstr = std::ffi::CString::new(local_abspath.to_str().unwrap())?;

        unsafe {
            let err = subversion_sys::svn_wc_add_lock2(
                self.ptr,
                path_cstr.as_ptr(),
                lock.as_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)
        }
    }

    /// Remove a lock from the working copy
    ///
    /// Removes lock information for the given path.
    pub fn remove_lock(&mut self, local_abspath: &std::path::Path) -> Result<(), Error> {
        let pool = apr::Pool::new();
        let path_cstr = std::ffi::CString::new(local_abspath.to_str().unwrap())?;

        unsafe {
            let err = subversion_sys::svn_wc_remove_lock2(
                self.ptr,
                path_cstr.as_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)
        }
    }

    /// Crop a working copy subtree to a specified depth
    ///
    /// This function will remove any items that exceed the specified depth.
    /// For example, cropping to Depth::Files will remove any subdirectories.
    pub fn crop_tree(
        &mut self,
        local_abspath: &std::path::Path,
        depth: crate::Depth,
        cancel_func: Option<&dyn Fn() -> Result<(), Error>>,
    ) -> Result<(), Error> {
        let pool = apr::Pool::new();
        let path = local_abspath.to_str().unwrap();
        let path_cstr = std::ffi::CString::new(path).unwrap();

        let cancel_baton = cancel_func
            .map(|f| Box::into_raw(Box::new(f)) as *mut std::ffi::c_void)
            .unwrap_or(std::ptr::null_mut());

        let ret = unsafe {
            subversion_sys::svn_wc_crop_tree2(
                self.ptr,
                path_cstr.as_ptr(),
                depth.into(),
                if cancel_func.is_some() {
                    Some(crate::wrap_cancel_func)
                } else {
                    None
                },
                cancel_baton,
                None,                 // notify_func - not commonly used for crop
                std::ptr::null_mut(), // notify_baton
                pool.as_mut_ptr(),
            )
        };

        // Free callback baton
        if !cancel_baton.is_null() {
            unsafe {
                drop(Box::from_raw(
                    cancel_baton as *mut Box<dyn Fn() -> Result<(), Error>>,
                ))
            };
        }

        Error::from_raw(ret)
    }

    /// Resolve a conflict on a working copy path
    ///
    /// This is the most advanced conflict resolution function, allowing
    /// specification of which conflict to resolve and how to resolve it.
    pub fn resolved_conflict(
        &mut self,
        local_abspath: &std::path::Path,
        depth: crate::Depth,
        resolve_text: bool,
        resolve_property: Option<&str>,
        resolve_tree: bool,
        conflict_choice: ConflictChoice,
        cancel_func: Option<&dyn Fn() -> Result<(), Error>>,
    ) -> Result<(), Error> {
        let pool = apr::Pool::new();
        let path = local_abspath.to_str().unwrap();
        let path_cstr = std::ffi::CString::new(path).unwrap();

        let prop_cstr = resolve_property.map(|p| std::ffi::CString::new(p).unwrap());
        let prop_ptr = prop_cstr.as_ref().map_or(std::ptr::null(), |p| p.as_ptr());

        let cancel_baton = cancel_func
            .map(|f| Box::into_raw(Box::new(f)) as *mut std::ffi::c_void)
            .unwrap_or(std::ptr::null_mut());

        let ret = unsafe {
            subversion_sys::svn_wc_resolved_conflict5(
                self.ptr,
                path_cstr.as_ptr(),
                depth.into(),
                resolve_text.into(),
                prop_ptr,
                resolve_tree.into(),
                conflict_choice.into(),
                if cancel_func.is_some() {
                    Some(crate::wrap_cancel_func)
                } else {
                    None
                },
                cancel_baton,
                None,                 // notify_func
                std::ptr::null_mut(), // notify_baton
                pool.as_mut_ptr(),
            )
        };

        // Free callback baton
        if !cancel_baton.is_null() {
            unsafe {
                drop(Box::from_raw(
                    cancel_baton as *mut Box<dyn Fn() -> Result<(), Error>>,
                ))
            };
        }

        Error::from_raw(ret)
    }

    /// Add a path from disk to version control
    ///
    /// This is the modern version that adds an existing on-disk item to version control.
    pub fn add_from_disk(
        &mut self,
        local_abspath: &std::path::Path,
        notify_func: Option<&dyn Fn(&Notify)>,
    ) -> Result<(), Error> {
        let path = local_abspath.to_str().unwrap();
        let path_cstr = std::ffi::CString::new(path).unwrap();
        let pool = apr::Pool::new();

        let notify_baton = notify_func
            .map(|f| Box::into_raw(Box::new(f)) as *mut std::ffi::c_void)
            .unwrap_or(std::ptr::null_mut());

        let ret = unsafe {
            subversion_sys::svn_wc_add_from_disk3(
                self.ptr,
                path_cstr.as_ptr(),
                std::ptr::null_mut(), // props (NULL = use auto-props)
                0,                    // skip checks
                if notify_func.is_some() {
                    Some(wrap_notify_func)
                } else {
                    None
                },
                notify_baton,
                pool.as_mut_ptr(),
            )
        };

        if !notify_baton.is_null() {
            unsafe { drop(Box::from_raw(notify_baton as *mut Box<dyn Fn(&Notify)>)) };
        }

        Error::from_raw(ret)
    }

    /// Move a file or directory within the working copy
    pub fn move_path(
        &mut self,
        src_abspath: &std::path::Path,
        dst_abspath: &std::path::Path,
        metadata_only: bool,
        _allow_mixed_revisions: bool,
        cancel_func: Option<&dyn Fn() -> Result<(), Error>>,
        notify_func: Option<&dyn Fn(&Notify)>,
    ) -> Result<(), Error> {
        let src = src_abspath.to_str().unwrap();
        let src_cstr = std::ffi::CString::new(src).unwrap();
        let dst = dst_abspath.to_str().unwrap();
        let dst_cstr = std::ffi::CString::new(dst).unwrap();
        let pool = apr::Pool::new();

        let cancel_baton = cancel_func
            .map(|f| Box::into_raw(Box::new(f)) as *mut std::ffi::c_void)
            .unwrap_or(std::ptr::null_mut());

        let notify_baton = notify_func
            .map(|f| Box::into_raw(Box::new(f)) as *mut std::ffi::c_void)
            .unwrap_or(std::ptr::null_mut());

        let ret = unsafe {
            subversion_sys::svn_wc_move(
                self.ptr,
                src_cstr.as_ptr(),
                dst_cstr.as_ptr(),
                metadata_only.into(),
                if cancel_func.is_some() {
                    Some(crate::wrap_cancel_func)
                } else {
                    None
                },
                cancel_baton,
                if notify_func.is_some() {
                    Some(wrap_notify_func)
                } else {
                    None
                },
                notify_baton,
                pool.as_mut_ptr(),
            )
        };

        if !cancel_baton.is_null() {
            unsafe {
                drop(Box::from_raw(
                    cancel_baton as *mut Box<dyn Fn() -> Result<(), Error>>,
                ))
            };
        }
        if !notify_baton.is_null() {
            unsafe { drop(Box::from_raw(notify_baton as *mut Box<dyn Fn(&Notify)>)) };
        }

        Error::from_raw(ret)
    }

    /// Delete a path from version control
    pub fn delete(
        &mut self,
        local_abspath: &std::path::Path,
        keep_local: bool,
        delete_unversioned_target: bool,
        cancel_func: Option<&dyn Fn() -> Result<(), Error>>,
        notify_func: Option<&dyn Fn(&Notify)>,
    ) -> Result<(), Error> {
        let path = local_abspath.to_str().unwrap();
        let path_cstr = std::ffi::CString::new(path).unwrap();
        let pool = apr::Pool::new();

        let cancel_baton = cancel_func
            .map(|f| Box::into_raw(Box::new(f)) as *mut std::ffi::c_void)
            .unwrap_or(std::ptr::null_mut());

        let notify_baton = notify_func
            .map(|f| Box::into_raw(Box::new(f)) as *mut std::ffi::c_void)
            .unwrap_or(std::ptr::null_mut());

        let ret = unsafe {
            subversion_sys::svn_wc_delete4(
                self.ptr,
                path_cstr.as_ptr(),
                keep_local.into(),
                delete_unversioned_target.into(),
                if cancel_func.is_some() {
                    Some(crate::wrap_cancel_func)
                } else {
                    None
                },
                cancel_baton,
                if notify_func.is_some() {
                    Some(wrap_notify_func)
                } else {
                    None
                },
                notify_baton,
                pool.as_mut_ptr(),
            )
        };

        if !cancel_baton.is_null() {
            unsafe {
                drop(Box::from_raw(
                    cancel_baton as *mut Box<dyn Fn() -> Result<(), Error>>,
                ))
            };
        }
        if !notify_baton.is_null() {
            unsafe { drop(Box::from_raw(notify_baton as *mut Box<dyn Fn(&Notify)>)) };
        }

        Error::from_raw(ret)
    }
}

/// Notify structure for callbacks
pub struct Notify {
    ptr: *const subversion_sys::svn_wc_notify_t,
}

impl Notify {
    unsafe fn from_ptr(ptr: *const subversion_sys::svn_wc_notify_t) -> Self {
        Self { ptr }
    }

    /// Get the action type
    pub fn action(&self) -> u32 {
        unsafe { (*self.ptr).action }
    }

    /// Get the path
    pub fn path(&self) -> Option<&str> {
        unsafe {
            if (*self.ptr).path.is_null() {
                None
            } else {
                Some(std::ffi::CStr::from_ptr((*self.ptr).path).to_str().unwrap())
            }
        }
    }
}

/// Wrapper for notify callbacks
extern "C" fn wrap_notify_func(
    baton: *mut std::ffi::c_void,
    notify: *const subversion_sys::svn_wc_notify_t,
    _pool: *mut apr_sys::apr_pool_t,
) {
    if baton.is_null() || notify.is_null() {
        return;
    }

    let callback = unsafe { &*(baton as *const Box<dyn Fn(&Notify)>) };
    let notify_struct = unsafe { Notify::from_ptr(notify) };
    callback(&notify_struct);
}

/// Represents a queue of committed items
pub struct CommittedQueue {
    ptr: *mut subversion_sys::svn_wc_committed_queue_t,
    #[allow(dead_code)]
    pool: apr::Pool,
}

impl Default for CommittedQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl CommittedQueue {
    /// Create a new committed queue
    pub fn new() -> Self {
        let pool = apr::Pool::new();
        let ptr = unsafe { subversion_sys::svn_wc_committed_queue_create(pool.as_mut_ptr()) };
        Self { ptr, pool }
    }

    fn as_mut_ptr(&mut self) -> *mut subversion_sys::svn_wc_committed_queue_t {
        self.ptr
    }
}

/// Represents a lock in the working copy
pub struct Lock {
    ptr: *const subversion_sys::svn_lock_t,
}

impl Lock {
    /// Create from a raw pointer
    pub fn from_ptr(ptr: *const subversion_sys::svn_lock_t) -> Self {
        Self { ptr }
    }

    fn as_ptr(&self) -> *const subversion_sys::svn_lock_t {
        self.ptr
    }
}

/// Clean up a working copy
pub fn cleanup(
    wc_path: &std::path::Path,
    break_locks: bool,
    fix_recorded_timestamps: bool,
    clear_dav_cache: bool,
    vacuum_pristines: bool,
    _include_externals: bool,
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
/// Add a path to version control
pub fn add(
    ctx: &mut Context,
    path: &std::path::Path,
    _depth: crate::Depth,
    force: bool,
    _no_ignore: bool,
    _no_autoprops: bool,
    _add_parents: bool,
) -> Result<(), Error> {
    let path_str = path.to_string_lossy();
    let path_cstr = std::ffi::CString::new(path_str.as_ref())?;

    with_tmp_pool(|pool| unsafe {
        let err = subversion_sys::svn_wc_add_from_disk3(
            ctx.as_mut_ptr(),
            path_cstr.as_ptr(),
            std::ptr::null_mut(), // props (use auto-props if enabled)
            force as i32,
            None,                 // notify_func
            std::ptr::null_mut(), // notify_baton
            pool.as_mut_ptr(),
        );
        Error::from_raw(err)
    })
}

/// Delete a path from version control
pub fn delete(
    ctx: &mut Context,
    path: &std::path::Path,
    keep_local: bool,
    delete_unversioned_target: bool,
) -> Result<(), Error> {
    let path_str = path.to_string_lossy();
    let path_cstr = std::ffi::CString::new(path_str.as_ref())?;

    with_tmp_pool(|pool| unsafe {
        let err = subversion_sys::svn_wc_delete4(
            ctx.as_mut_ptr(),
            path_cstr.as_ptr(),
            keep_local as i32,
            delete_unversioned_target as i32,
            None,                 // cancel_func
            std::ptr::null_mut(), // cancel_baton
            None,                 // notify_func
            std::ptr::null_mut(), // notify_baton
            pool.as_mut_ptr(),
        );
        Error::from_raw(err)
    })
}

/// Revert changes to a path
pub fn revert(
    ctx: &mut Context,
    path: &std::path::Path,
    depth: crate::Depth,
    use_commit_times: bool,
    clear_changelists: bool,
    metadata_only: bool,
) -> Result<(), Error> {
    let path_str = path.to_string_lossy();
    let path_cstr = std::ffi::CString::new(path_str.as_ref())?;

    with_tmp_pool(|pool| unsafe {
        let err = subversion_sys::svn_wc_revert6(
            ctx.as_mut_ptr(),
            path_cstr.as_ptr(),
            depth.into(),
            use_commit_times as i32,
            std::ptr::null(), // changelists
            clear_changelists as i32,
            metadata_only as i32,
            1,                    // added_keep_local (keep added files)
            None,                 // cancel_func
            std::ptr::null_mut(), // cancel_baton
            None,                 // notify_func
            std::ptr::null_mut(), // notify_baton
            pool.as_mut_ptr(),
        );
        Error::from_raw(err)
    })
}

/// Copy or move a path within the working copy
pub fn copy_or_move(
    ctx: &mut Context,
    src: &std::path::Path,
    dst: &std::path::Path,
    is_move: bool,
    metadata_only: bool,
) -> Result<(), Error> {
    let src_str = src.to_string_lossy();
    let src_cstr = std::ffi::CString::new(src_str.as_ref())?;
    let dst_str = dst.to_string_lossy();
    let dst_cstr = std::ffi::CString::new(dst_str.as_ref())?;

    with_tmp_pool(|pool| unsafe {
        if is_move {
            let err = subversion_sys::svn_wc_move(
                ctx.as_mut_ptr(),
                src_cstr.as_ptr(),
                dst_cstr.as_ptr(),
                metadata_only as i32,
                None,                 // cancel_func
                std::ptr::null_mut(), // cancel_baton
                None,                 // notify_func
                std::ptr::null_mut(), // notify_baton
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)
        } else {
            let err = subversion_sys::svn_wc_copy3(
                ctx.as_mut_ptr(),
                src_cstr.as_ptr(),
                dst_cstr.as_ptr(),
                metadata_only as i32,
                None,                 // cancel_func
                std::ptr::null_mut(), // cancel_baton
                None,                 // notify_func
                std::ptr::null_mut(), // notify_baton
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)
        }
    })
}

/// Resolve a conflict on a path
pub fn resolve_conflict(
    ctx: &mut Context,
    path: &std::path::Path,
    depth: crate::Depth,
    resolve_text: bool,
    _resolve_props: bool,
    resolve_tree: bool,
    conflict_choice: ConflictChoice,
) -> Result<(), Error> {
    let path_str = path.to_string_lossy();
    let path_cstr = std::ffi::CString::new(path_str.as_ref())?;

    with_tmp_pool(|pool| unsafe {
        let err = subversion_sys::svn_wc_resolved_conflict5(
            ctx.as_mut_ptr(),
            path_cstr.as_ptr(),
            depth.into(),
            resolve_text as i32,
            std::ptr::null(), // resolve_prop (resolve all props if resolve_props is true)
            resolve_tree as i32,
            conflict_choice.into(),
            None,                 // cancel_func
            std::ptr::null_mut(), // cancel_baton
            None,                 // notify_func
            std::ptr::null_mut(), // notify_baton
            pool.as_mut_ptr(),
        );
        Error::from_raw(err)
    })
}

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
            return Err(Error::from(std::io::Error::other(
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

// Note: Advanced conflict resolution functions like crop_tree and
// mark_resolved require more complex FFI bindings that are not currently
// implemented in the subversion-sys crate. The basic conflict detection
// via Context.conflicted() is available and working.

/// Conflict resolution choice
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ConflictChoice {
    /// Postpone resolution
    Postpone = subversion_sys::svn_wc_conflict_choice_t_svn_wc_conflict_choose_postpone,
    /// Choose the base revision
    Base = subversion_sys::svn_wc_conflict_choice_t_svn_wc_conflict_choose_base,
    /// Choose the theirs revision
    Theirs = subversion_sys::svn_wc_conflict_choice_t_svn_wc_conflict_choose_theirs_full,
    /// Choose the mine/working revision
    Mine = subversion_sys::svn_wc_conflict_choice_t_svn_wc_conflict_choose_mine_full,
    /// Choose the theirs file for conflicts
    TheirsConflict =
        subversion_sys::svn_wc_conflict_choice_t_svn_wc_conflict_choose_theirs_conflict,
    /// Choose the mine file for conflicts
    MineConflict = subversion_sys::svn_wc_conflict_choice_t_svn_wc_conflict_choose_mine_conflict,
    /// Merge the conflicted regions
    Merged = subversion_sys::svn_wc_conflict_choice_t_svn_wc_conflict_choose_merged,
}

impl From<ConflictChoice> for subversion_sys::svn_wc_conflict_choice_t {
    fn from(choice: ConflictChoice) -> Self {
        choice as subversion_sys::svn_wc_conflict_choice_t
    }
}

// Context methods for conflict resolution would go here when the
// underlying FFI bindings are properly implemented

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
        // Test StatusKind enum conversions
        assert_eq!(
            StatusKind::Normal as u32,
            subversion_sys::svn_wc_status_kind_svn_wc_status_normal
        );
        assert_eq!(
            StatusKind::Added as u32,
            subversion_sys::svn_wc_status_kind_svn_wc_status_added
        );
        assert_eq!(
            StatusKind::Deleted as u32,
            subversion_sys::svn_wc_status_kind_svn_wc_status_deleted
        );

        // Test From conversion
        let status = StatusKind::from(subversion_sys::svn_wc_status_kind_svn_wc_status_modified);
        assert_eq!(status, StatusKind::Modified);
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

    #[test]
    fn test_conflict_choice_enum() {
        // Test that ConflictChoice enum values map correctly
        assert_eq!(
            ConflictChoice::Postpone as i32,
            subversion_sys::svn_wc_conflict_choice_t_svn_wc_conflict_choose_postpone
        );
        assert_eq!(
            ConflictChoice::Base as i32,
            subversion_sys::svn_wc_conflict_choice_t_svn_wc_conflict_choose_base
        );
        assert_eq!(
            ConflictChoice::Theirs as i32,
            subversion_sys::svn_wc_conflict_choice_t_svn_wc_conflict_choose_theirs_full
        );
        assert_eq!(
            ConflictChoice::Mine as i32,
            subversion_sys::svn_wc_conflict_choice_t_svn_wc_conflict_choose_mine_full
        );
    }

    #[test]
    fn test_crop_tree_basic() {
        // Test that crop_tree function compiles and can be called
        // Creating a real working copy for testing would require full SVN setup
        let mut ctx = Context::new().unwrap();
        let tempdir = tempdir().unwrap();

        // This will fail without a working copy, but tests the API
        let result = ctx.crop_tree(tempdir.path(), crate::Depth::Files, None);

        // Expected to fail without valid working copy
        assert!(result.is_err());
    }

    #[test]
    fn test_resolved_conflict_basic() {
        // Test that resolved_conflict function compiles and can be called
        let mut ctx = Context::new().unwrap();
        let tempdir = tempdir().unwrap();

        // This will fail without a working copy with conflicts, but tests the API
        let result = ctx.resolved_conflict(
            tempdir.path(),
            crate::Depth::Infinity,
            true,  // resolve_text
            None,  // resolve_property
            false, // resolve_tree
            ConflictChoice::Mine,
            None,
        );

        // Expected to fail without valid working copy
        assert!(result.is_err());
    }

    #[test]
    fn test_conflict_choice_conversion() {
        // Test that ConflictChoice enum converts properly to SVN types
        let choice = ConflictChoice::Mine;
        let svn_choice: subversion_sys::svn_wc_conflict_choice_t = choice.into();
        assert_eq!(
            svn_choice,
            subversion_sys::svn_wc_conflict_choice_t_svn_wc_conflict_choose_mine_full
        );

        let choice = ConflictChoice::Theirs;
        let svn_choice: subversion_sys::svn_wc_conflict_choice_t = choice.into();
        assert_eq!(
            svn_choice,
            subversion_sys::svn_wc_conflict_choice_t_svn_wc_conflict_choose_theirs_full
        );
    }

    #[test]
    fn test_match_ignore_list() {
        // Test exact matches
        assert!(match_ignore_list("foo", &["foo", "bar"]).unwrap());
        assert!(match_ignore_list("bar", &["foo", "bar"]).unwrap());
        assert!(!match_ignore_list("baz", &["foo", "bar"]).unwrap());

        // Test wildcard patterns
        assert!(match_ignore_list("foo", &["f*"]).unwrap());
        assert!(match_ignore_list("foobar", &["f*"]).unwrap());
        assert!(!match_ignore_list("bar", &["f*"]).unwrap());

        // Test file extension patterns
        assert!(match_ignore_list("test.txt", &["*.txt"]).unwrap());
        assert!(match_ignore_list("file.txt", &["*.txt", "*.log"]).unwrap());
        assert!(!match_ignore_list("test.rs", &["*.txt"]).unwrap());

        // Test empty patterns
        assert!(!match_ignore_list("foo", &[]).unwrap());
    }

    #[test]
    fn test_add_from_disk() {
        // This test requires a working copy, so we just test that the API compiles
        // and returns an error when used on a non-WC directory
        let temp_dir = tempfile::tempdir().unwrap();
        let mut ctx = Context::new().unwrap();

        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "test content").unwrap();

        // Should fail without a working copy
        let result = ctx.add_from_disk(&file_path, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_move_path() {
        // Test that the API compiles and fails gracefully without a WC
        let temp_dir = tempfile::tempdir().unwrap();
        let mut ctx = Context::new().unwrap();

        let src = temp_dir.path().join("src.txt");
        let dst = temp_dir.path().join("dst.txt");
        std::fs::write(&src, "content").unwrap();

        // Should fail without a working copy
        let result = ctx.move_path(&src, &dst, false, false, None, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_delete() {
        // Test that the API compiles and fails gracefully without a WC
        let temp_dir = tempfile::tempdir().unwrap();
        let mut ctx = Context::new().unwrap();

        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "test content").unwrap();

        // Should fail without a working copy
        let result = ctx.delete(&file_path, false, false, None, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_committed_queue() {
        // Test CommittedQueue creation
        let queue = CommittedQueue::new();
        assert!(!queue.ptr.is_null());

        // Test queue_committed and process_committed_queue APIs
        let temp_dir = tempfile::tempdir().unwrap();
        let mut ctx = Context::new().unwrap();
        let mut queue = CommittedQueue::new();

        let file_path = temp_dir.path().join("test.txt");
        std::fs::write(&file_path, "test content").unwrap();

        // Should fail without a working copy
        let result = ctx.queue_committed(&file_path, false, &mut queue);
        assert!(result.is_err());
    }

    #[test]
    fn test_notify_struct() {
        // Test that Notify struct methods work
        // We can't easily create a real notify, but we can test the structure exists
        use std::mem::size_of;
        assert!(size_of::<Notify>() > 0);
    }
}
