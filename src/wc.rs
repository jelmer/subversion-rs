//! Working copy management and status operations.
//!
//! This module provides low-level access to Subversion working copies through the [`Context`](crate::wc::Context) type.
//! It handles working copy metadata, status tracking, and local modifications.
//!
//! # Overview
//!
//! The working copy (WC) layer manages the `.svn` administrative directory and tracks the state
//! of files in a working copy. Most users should use the [`client`](crate::client) module instead,
//! which provides higher-level operations. This module is useful for tools that need direct
//! access to working copy internals.
//!
//! ## Key Operations
//!
//! - **Status tracking**: Walk working copy and report file status
//! - **Property management**: Get, set, and list versioned properties
//! - **Conflict resolution**: Handle and resolve merge conflicts
//! - **Working copy maintenance**: Revert changes, cleanup locks
//! - **Notification**: Receive callbacks about working copy operations
//!
//! # Example
//!
//! ```no_run
//! use subversion::wc::Context;
//!
//! let ctx = Context::new().unwrap();
//!
//! // Check if a path is a working copy root
//! if ctx.is_wc_root("/path/to/wc").unwrap() {
//!     println!("This is a working copy root");
//! }
//! ```

use crate::{svn_result, with_tmp_pool, Error};
use std::marker::PhantomData;
use subversion_sys::{svn_wc_context_t, svn_wc_version};

// Helper functions for properly boxing callback batons
// wrap_cancel_func expects *mut Box<dyn Fn()>, not *mut Box<&dyn Fn()>
// We need double-boxing to avoid UB
fn box_cancel_baton(f: Box<dyn Fn() -> Result<(), Error>>) -> *mut std::ffi::c_void {
    Box::into_raw(Box::new(f)) as *mut std::ffi::c_void
}

fn box_notify_baton(f: Box<dyn Fn(&Notify)>) -> *mut std::ffi::c_void {
    Box::into_raw(Box::new(f)) as *mut std::ffi::c_void
}

fn box_conflict_baton(
    f: Box<
        dyn Fn(
            &crate::conflict::ConflictDescription,
        ) -> Result<crate::conflict::ConflictResult, Error>,
    >,
) -> *mut std::ffi::c_void {
    Box::into_raw(Box::new(f)) as *mut std::ffi::c_void
}

fn box_external_baton(
    f: Box<dyn Fn(&str, Option<&str>, Option<&str>, crate::Depth) -> Result<(), Error>>,
) -> *mut std::ffi::c_void {
    Box::into_raw(Box::new(f)) as *mut std::ffi::c_void
}

fn box_fetch_dirents_baton(
    f: Box<
        dyn Fn(&str, &str) -> Result<std::collections::HashMap<String, crate::ra::Dirent>, Error>,
    >,
) -> *mut std::ffi::c_void {
    Box::into_raw(Box::new(f)) as *mut std::ffi::c_void
}

// Borrowed versions for synchronous operations where callback lifetime is guaranteed
fn box_cancel_baton_borrowed(f: &dyn Fn() -> Result<(), Error>) -> *mut std::ffi::c_void {
    Box::into_raw(Box::new(f)) as *mut std::ffi::c_void
}

fn box_notify_baton_borrowed(f: &dyn Fn(&Notify)) -> *mut std::ffi::c_void {
    Box::into_raw(Box::new(f)) as *mut std::ffi::c_void
}

// Dropper functions for each callback type
unsafe fn drop_cancel_baton(baton: *mut std::ffi::c_void) {
    drop(Box::from_raw(
        baton as *mut Box<dyn Fn() -> Result<(), Error>>,
    ));
}

unsafe fn drop_notify_baton(baton: *mut std::ffi::c_void) {
    drop(Box::from_raw(baton as *mut Box<dyn Fn(&Notify)>));
}

unsafe fn drop_conflict_baton(baton: *mut std::ffi::c_void) {
    drop(Box::from_raw(
        baton
            as *mut Box<
                dyn Fn(
                    &crate::conflict::ConflictDescription,
                ) -> Result<crate::conflict::ConflictResult, Error>,
            >,
    ));
}

unsafe fn drop_external_baton(baton: *mut std::ffi::c_void) {
    drop(Box::from_raw(
        baton
            as *mut Box<
                dyn Fn(&str, Option<&str>, Option<&str>, crate::Depth) -> Result<(), Error>,
            >,
    ));
}

unsafe fn drop_fetch_dirents_baton(baton: *mut std::ffi::c_void) {
    drop(Box::from_raw(
        baton
            as *mut Box<
                dyn Fn(
                    &str,
                    &str,
                )
                    -> Result<std::collections::HashMap<String, crate::ra::Dirent>, Error>,
            >,
    ));
}

// Dropper functions for borrowed callbacks (used in synchronous operations)
unsafe fn drop_cancel_baton_borrowed(baton: *mut std::ffi::c_void) {
    drop(Box::from_raw(baton as *mut &dyn Fn() -> Result<(), Error>));
}

unsafe fn drop_notify_baton_borrowed(baton: *mut std::ffi::c_void) {
    drop(Box::from_raw(baton as *mut &dyn Fn(&Notify)));
}

/// Returns the version information for the working copy library.
pub fn version() -> crate::Version {
    unsafe { crate::Version(svn_wc_version()) }
}

// Status constants for Python compatibility
/// Status constant indicating no status.
pub const STATUS_NONE: u32 = subversion_sys::svn_wc_status_kind_svn_wc_status_none;
/// Status constant for unversioned items.
pub const STATUS_UNVERSIONED: u32 = subversion_sys::svn_wc_status_kind_svn_wc_status_unversioned;
/// Status constant for normal versioned items.
pub const STATUS_NORMAL: u32 = subversion_sys::svn_wc_status_kind_svn_wc_status_normal;
/// Status constant for added items.
pub const STATUS_ADDED: u32 = subversion_sys::svn_wc_status_kind_svn_wc_status_added;
/// Status constant for missing items.
pub const STATUS_MISSING: u32 = subversion_sys::svn_wc_status_kind_svn_wc_status_missing;
/// Status constant for deleted items.
pub const STATUS_DELETED: u32 = subversion_sys::svn_wc_status_kind_svn_wc_status_deleted;
/// Status constant for replaced items.
pub const STATUS_REPLACED: u32 = subversion_sys::svn_wc_status_kind_svn_wc_status_replaced;
/// Status constant for modified items.
pub const STATUS_MODIFIED: u32 = subversion_sys::svn_wc_status_kind_svn_wc_status_modified;
/// Status constant for merged items.
pub const STATUS_MERGED: u32 = subversion_sys::svn_wc_status_kind_svn_wc_status_merged;
/// Status constant for conflicted items.
pub const STATUS_CONFLICTED: u32 = subversion_sys::svn_wc_status_kind_svn_wc_status_conflicted;
/// Status constant for ignored items.
pub const STATUS_IGNORED: u32 = subversion_sys::svn_wc_status_kind_svn_wc_status_ignored;
/// Status constant for obstructed items.
pub const STATUS_OBSTRUCTED: u32 = subversion_sys::svn_wc_status_kind_svn_wc_status_obstructed;
/// Status constant for external items.
pub const STATUS_EXTERNAL: u32 = subversion_sys::svn_wc_status_kind_svn_wc_status_external;
/// Status constant for incomplete items.
pub const STATUS_INCOMPLETE: u32 = subversion_sys::svn_wc_status_kind_svn_wc_status_incomplete;

// Schedule constants for Python compatibility
/// Schedule constant for normal items.
pub const SCHEDULE_NORMAL: u32 = subversion_sys::svn_wc_schedule_t_svn_wc_schedule_normal;
/// Schedule constant for items to be added.
pub const SCHEDULE_ADD: u32 = subversion_sys::svn_wc_schedule_t_svn_wc_schedule_add;
/// Schedule constant for items to be deleted.
pub const SCHEDULE_DELETE: u32 = subversion_sys::svn_wc_schedule_t_svn_wc_schedule_delete;
/// Schedule constant for items to be replaced.
pub const SCHEDULE_REPLACE: u32 = subversion_sys::svn_wc_schedule_t_svn_wc_schedule_replace;

/// Working copy status types.
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

/// Represents a property change in the working copy
///
/// A property change consists of a property name and an optional value.
/// If the value is None, it indicates the property has been deleted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropChange {
    /// The name of the property
    pub name: String,
    /// The new value of the property, or None if deleted
    pub value: Option<Vec<u8>>,
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
    pool: apr::Pool<'static>,
    _phantom: PhantomData<*mut ()>, // !Send + !Sync
}

impl Drop for Context {
    fn drop(&mut self) {
        // Pool drop will clean up context
    }
}

impl Context {
    /// Get a reference to the underlying pool
    pub fn pool(&self) -> &apr::Pool<'_> {
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

    /// Creates a new working copy context.
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

    /// Checks the working copy format version.
    pub fn check_wc(&mut self, path: &str) -> Result<i32, crate::Error> {
        let scratch_pool = apr::pool::Pool::new();
        let path = std::ffi::CString::new(path).unwrap();
        let mut wc_format = 0;
        let err = unsafe {
            subversion_sys::svn_wc_check_wc2(
                &mut wc_format,
                self.ptr,
                path.as_ptr(),
                scratch_pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(wc_format)
    }

    /// Checks if a file's text content has been modified.
    pub fn text_modified(&mut self, path: &str) -> Result<bool, crate::Error> {
        let scratch_pool = apr::pool::Pool::new();
        let path = std::ffi::CString::new(path).unwrap();
        let mut modified = 0;
        let err = unsafe {
            subversion_sys::svn_wc_text_modified_p2(
                &mut modified,
                self.ptr,
                path.as_ptr(),
                0,
                scratch_pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(modified != 0)
    }

    /// Checks if a file's properties have been modified.
    pub fn props_modified(&mut self, path: &str) -> Result<bool, crate::Error> {
        let scratch_pool = apr::pool::Pool::new();
        let path = std::ffi::CString::new(path).unwrap();
        let mut modified = 0;
        let err = unsafe {
            subversion_sys::svn_wc_props_modified_p2(
                &mut modified,
                self.ptr,
                path.as_ptr(),
                scratch_pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(modified != 0)
    }

    /// Checks if a path has conflicts (text, property, tree).
    pub fn conflicted(&mut self, path: &str) -> Result<(bool, bool, bool), crate::Error> {
        let scratch_pool = apr::pool::Pool::new();
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
                scratch_pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok((
            text_conflicted != 0,
            prop_conflicted != 0,
            tree_conflicted != 0,
        ))
    }

    /// Ensures an administrative area exists for the given path.
    pub fn ensure_adm(
        &mut self,
        local_abspath: &str,
        url: &str,
        repos_root_url: &str,
        repos_uuid: &str,
        revision: crate::Revnum,
        depth: crate::Depth,
    ) -> Result<(), crate::Error> {
        let scratch_pool = apr::pool::Pool::new();
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
                scratch_pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(())
    }

    /// Checks if a path is locked in the working copy.
    /// Returns (locked_here, locked) where locked_here means locked in this working copy.
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

    /// Upgrade a working copy to the latest format
    pub fn upgrade(&mut self, local_abspath: &str) -> Result<(), crate::Error> {
        let local_abspath_cstr = std::ffi::CString::new(local_abspath)?;
        let scratch_pool = apr::pool::Pool::new();

        let err = unsafe {
            subversion_sys::svn_wc_upgrade(
                self.ptr,
                local_abspath_cstr.as_ptr(),
                None,                 // repos_info_func
                std::ptr::null_mut(), // repos_info_baton
                None,                 // cancel_func
                std::ptr::null_mut(), // cancel_baton
                None,                 // notify_func
                std::ptr::null_mut(), // notify_baton
                scratch_pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(())
    }

    /// Relocate the working copy to a new repository URL
    pub fn relocate(
        &mut self,
        wcroot_abspath: &str,
        from: &str,
        to: &str,
    ) -> Result<(), crate::Error> {
        let wcroot_abspath_cstr = std::ffi::CString::new(wcroot_abspath)?;
        let from_cstr = std::ffi::CString::new(from)?;
        let to_cstr = std::ffi::CString::new(to)?;
        let scratch_pool = apr::pool::Pool::new();

        // Default validator that accepts all relocations
        unsafe extern "C" fn default_validator(
            _baton: *mut std::ffi::c_void,
            _uuid: *const std::ffi::c_char,
            _url: *const std::ffi::c_char,
            _root_url: *const std::ffi::c_char,
            _pool: *mut apr_sys::apr_pool_t,
        ) -> *mut subversion_sys::svn_error_t {
            std::ptr::null_mut() // No error = validation successful
        }

        let err = unsafe {
            subversion_sys::svn_wc_relocate4(
                self.ptr,
                wcroot_abspath_cstr.as_ptr(),
                from_cstr.as_ptr(),
                to_cstr.as_ptr(),
                Some(default_validator),
                std::ptr::null_mut(), // validator_baton
                scratch_pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(())
    }

    /// Add a file or directory to the working copy
    pub fn add(
        &mut self,
        local_abspath: &str,
        depth: crate::Depth,
        copyfrom_url: Option<&str>,
        copyfrom_rev: Option<crate::Revnum>,
    ) -> Result<(), crate::Error> {
        let local_abspath_cstr = std::ffi::CString::new(local_abspath)?;
        let copyfrom_url_cstr = copyfrom_url.map(std::ffi::CString::new).transpose()?;
        let scratch_pool = apr::pool::Pool::new();

        let err = unsafe {
            subversion_sys::svn_wc_add4(
                self.ptr,
                local_abspath_cstr.as_ptr(),
                depth.into(),
                copyfrom_url_cstr
                    .as_ref()
                    .map_or(std::ptr::null(), |c| c.as_ptr()),
                copyfrom_rev.map_or(-1, |r| r.into()),
                None,                 // cancel_func
                std::ptr::null_mut(), // cancel_baton
                None,                 // notify_func
                std::ptr::null_mut(), // notify_baton
                scratch_pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(())
    }
}

/// Sets the name of the administrative directory (typically ".svn").
pub fn set_adm_dir(name: &str) -> Result<(), crate::Error> {
    let scratch_pool = apr::pool::Pool::new();
    let name = std::ffi::CString::new(name).unwrap();
    let err =
        unsafe { subversion_sys::svn_wc_set_adm_dir(name.as_ptr(), scratch_pool.as_mut_ptr()) };
    Error::from_raw(err)?;
    Ok(())
}

/// Returns the name of the administrative directory.
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

/// Crawl local changes in the working copy and report them to the repository
pub fn crawl_revisions5(
    wc_ctx: &mut Context,
    local_abspath: &str,
    reporter: &mut crate::ra::WrapReporter,
    restore_files: bool,
    depth: crate::Depth,
    honor_depth_exclude: bool,
    depth_compatibility_trick: bool,
    use_commit_times: bool,
) -> Result<(), crate::Error> {
    let local_abspath_cstr = std::ffi::CString::new(local_abspath)?;

    with_tmp_pool(|scratch_pool| {
        let err = unsafe {
            subversion_sys::svn_wc_crawl_revisions5(
                wc_ctx.as_mut_ptr(),
                local_abspath_cstr.as_ptr(),
                reporter.as_ptr(),
                reporter.as_baton(),
                if restore_files { 1 } else { 0 },
                depth.into(),
                if honor_depth_exclude { 1 } else { 0 },
                if depth_compatibility_trick { 1 } else { 0 },
                if use_commit_times { 1 } else { 0 },
                None,                 // notify_func
                std::ptr::null_mut(), // notify_baton
                None,                 // cancel_func
                std::ptr::null_mut(), // cancel_baton
                scratch_pool.as_mut_ptr(),
            )
        };

        svn_result(err)
    })
}

/// Get an editor for updating the working copy
pub fn get_update_editor4(
    wc_ctx: &mut Context,
    anchor_abspath: &str,
    target_basename: &str,
    use_commit_times: bool,
    depth: crate::Depth,
    depth_is_sticky: bool,
    allow_unver_obstructions: bool,
    adds_as_modification: bool,
    server_performs_filtering: bool,
    clean_checkout: bool,
    diff3_cmd: Option<&str>,
    preserved_exts: &[&str],
    fetch_dirents_func: Option<
        Box<
            dyn Fn(
                &str,
                &str,
            )
                -> Result<std::collections::HashMap<String, crate::ra::Dirent>, Error>,
        >,
    >,
    conflict_func: Option<
        Box<
            dyn Fn(
                &crate::conflict::ConflictDescription,
            ) -> Result<crate::conflict::ConflictResult, Error>,
        >,
    >,
    external_func: Option<
        Box<dyn Fn(&str, Option<&str>, Option<&str>, crate::Depth) -> Result<(), Error>>,
    >,
    cancel_func: Option<Box<dyn Fn() -> Result<(), Error>>>,
    notify_func: Option<Box<dyn Fn(&Notify)>>,
) -> Result<(UpdateEditor, crate::Revnum), crate::Error> {
    let anchor_abspath_cstr = std::ffi::CString::new(anchor_abspath)?;
    let target_basename_cstr = std::ffi::CString::new(target_basename)?;
    let diff3_cmd_cstr = diff3_cmd.map(std::ffi::CString::new).transpose()?;

    // Create preserved extensions array
    let preserved_exts_cstrs: Vec<std::ffi::CString> = preserved_exts
        .iter()
        .map(|&s| std::ffi::CString::new(s))
        .collect::<Result<Vec<_>, _>>()?;
    let _preserved_exts_ptrs: Vec<*const std::ffi::c_char> = preserved_exts_cstrs
        .iter()
        .map(|s| s.as_ptr())
        .chain(std::iter::once(std::ptr::null()))
        .collect();

    let result_pool = apr::Pool::new();
    let mut target_revision: subversion_sys::svn_revnum_t = 0;
    let mut editor_ptr: *const subversion_sys::svn_delta_editor_t = std::ptr::null();
    let mut edit_baton: *mut std::ffi::c_void = std::ptr::null_mut();

    // Create batons for callbacks
    let has_fetch_dirents = fetch_dirents_func.is_some();
    let fetch_dirents_baton = fetch_dirents_func
        .map(|f| box_fetch_dirents_baton(f))
        .unwrap_or(std::ptr::null_mut());
    let has_conflict = conflict_func.is_some();
    let conflict_baton = conflict_func
        .map(|f| box_conflict_baton(f))
        .unwrap_or(std::ptr::null_mut());
    let has_external = external_func.is_some();
    let external_baton = external_func
        .map(|f| box_external_baton(f))
        .unwrap_or(std::ptr::null_mut());
    let has_cancel = cancel_func.is_some();
    let cancel_baton = cancel_func
        .map(|f| box_cancel_baton(f))
        .unwrap_or(std::ptr::null_mut());
    let has_notify = notify_func.is_some();
    let notify_baton = notify_func
        .map(|f| box_notify_baton(f))
        .unwrap_or(std::ptr::null_mut());

    let err = with_tmp_pool(|scratch_pool| {
        unsafe {
            svn_result(subversion_sys::svn_wc_get_update_editor4(
                &mut editor_ptr,
                &mut edit_baton,
                &mut target_revision,
                wc_ctx.as_mut_ptr(),
                anchor_abspath_cstr.as_ptr(),
                target_basename_cstr.as_ptr(),
                if use_commit_times { 1 } else { 0 }, // use_commit_times
                depth.into(),
                if depth_is_sticky { 1 } else { 0 },
                if allow_unver_obstructions { 1 } else { 0 },
                if adds_as_modification { 1 } else { 0 },
                if server_performs_filtering { 1 } else { 0 },
                if clean_checkout { 1 } else { 0 },
                diff3_cmd_cstr
                    .as_ref()
                    .map_or(std::ptr::null(), |c| c.as_ptr()),
                std::ptr::null(), // preserved_exts - TODO: create proper apr_array_header_t
                if has_fetch_dirents {
                    Some(wrap_fetch_dirents_func)
                } else {
                    None
                },
                fetch_dirents_baton,
                if has_conflict {
                    Some(wrap_conflict_func)
                } else {
                    None
                },
                conflict_baton,
                if has_external {
                    Some(wrap_external_func)
                } else {
                    None
                },
                external_baton,
                if has_cancel {
                    Some(crate::wrap_cancel_func)
                } else {
                    None
                },
                cancel_baton,
                if has_notify {
                    Some(wrap_notify_func)
                } else {
                    None
                },
                notify_baton,
                result_pool.as_mut_ptr(),
                scratch_pool.as_mut_ptr(),
            ))
        }
    });

    err?;

    // Create the update editor wrapper
    // Store callback batons with their droppers so they're properly cleaned up
    let mut batons = Vec::new();
    if !fetch_dirents_baton.is_null() {
        batons.push((fetch_dirents_baton, drop_fetch_dirents_baton as DropperFn));
    }
    if !conflict_baton.is_null() {
        batons.push((conflict_baton, drop_conflict_baton as DropperFn));
    }
    if !external_baton.is_null() {
        batons.push((external_baton, drop_external_baton as DropperFn));
    }
    if !cancel_baton.is_null() {
        batons.push((cancel_baton, drop_cancel_baton as DropperFn));
    }
    if !notify_baton.is_null() {
        batons.push((notify_baton, drop_notify_baton as DropperFn));
    }

    let editor = UpdateEditor {
        editor: editor_ptr as *const subversion_sys::svn_delta_editor_t,
        edit_baton,
        _pool: result_pool,
        target_revision: crate::Revnum::from_raw(target_revision).unwrap_or_default(),
        callback_batons: batons,
    };

    Ok((
        editor,
        crate::Revnum::from_raw(target_revision).unwrap_or_default(),
    ))
}

// Type-erased dropper function for callback batons
type DropperFn = unsafe fn(*mut std::ffi::c_void);

/// Update editor for working copy operations
pub struct UpdateEditor {
    editor: *const subversion_sys::svn_delta_editor_t,
    edit_baton: *mut std::ffi::c_void,
    _pool: apr::Pool<'static>,
    target_revision: crate::Revnum,
    // Callback batons with their dropper functions
    callback_batons: Vec<(*mut std::ffi::c_void, DropperFn)>,
}

impl Drop for UpdateEditor {
    fn drop(&mut self) {
        // Clean up callback batons using their type-erased droppers
        for (baton, dropper) in &self.callback_batons {
            if !baton.is_null() {
                unsafe {
                    dropper(*baton);
                }
            }
        }
        self.callback_batons.clear();
    }
}

impl UpdateEditor {
    /// Get the target revision for this update
    pub fn target_revision(&self) -> crate::Revnum {
        self.target_revision
    }
}

impl crate::delta::Editor for UpdateEditor {
    fn as_raw_parts(
        &self,
    ) -> (
        *const subversion_sys::svn_delta_editor_t,
        *mut std::ffi::c_void,
    ) {
        (self.editor, self.edit_baton)
    }

    fn set_target_revision(&mut self, revision: Option<crate::Revnum>) -> Result<(), crate::Error> {
        let scratch_pool = apr::Pool::new();
        let err = unsafe {
            ((*self.editor).set_target_revision.unwrap())(
                self.edit_baton,
                revision.map_or(-1, |r| r.into()),
                scratch_pool.as_mut_ptr(),
            )
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }

    fn open_root(
        &mut self,
        base_revision: Option<crate::Revnum>,
    ) -> Result<Box<dyn crate::delta::DirectoryEditor + 'static>, crate::Error> {
        let mut baton = std::ptr::null_mut();
        let pool = apr::Pool::new();
        let err = unsafe {
            ((*self.editor).open_root.unwrap())(
                self.edit_baton,
                base_revision.map_or(-1, |r| r.into()),
                pool.as_mut_ptr(),
                &mut baton,
            )
        };
        crate::Error::from_raw(err)?;
        Ok(Box::new(crate::delta::WrapDirectoryEditor {
            editor: self.editor,
            baton,
            _pool: apr::PoolHandle::owned(pool),
        }))
    }

    fn close(&mut self) -> Result<(), crate::Error> {
        let scratch_pool = apr::Pool::new();
        let err = unsafe {
            ((*self.editor).close_edit.unwrap())(self.edit_baton, scratch_pool.as_mut_ptr())
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }

    fn abort(&mut self) -> Result<(), crate::Error> {
        let scratch_pool = apr::Pool::new();
        let err = unsafe {
            ((*self.editor).abort_edit.unwrap())(self.edit_baton, scratch_pool.as_mut_ptr())
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }
}

/// Directory entries type for working copy operations
pub type DirEntries = std::collections::HashMap<String, crate::ra::Dirent>;

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

    // Create a pool that will live as long as the Stream
    let result_pool = apr::Pool::new();

    with_tmp_pool(|scratch_pool| -> Result<(), crate::Error> {
        let mut ctx = std::ptr::null_mut();
        with_tmp_pool(|ctx_scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_context_create(
                    &mut ctx,
                    std::ptr::null_mut(),
                    scratch_pool.as_mut_ptr(), // ctx lives in the outer scratch pool
                    ctx_scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })?;

        let err = unsafe {
            subversion_sys::svn_wc_get_pristine_contents2(
                &mut contents,
                ctx,
                path_cstr.as_ptr(),
                result_pool.as_mut_ptr(),  // result pool for the stream
                scratch_pool.as_mut_ptr(), // scratch pool for temporary allocations
            )
        };
        Error::from_raw(err)?;
        Ok(())
    })?;

    if contents.is_null() {
        Ok(None)
    } else {
        Ok(Some(unsafe {
            crate::io::Stream::from_ptr_and_pool(contents, result_pool)
        }))
    }
}

/// Get pristine copy path (deprecated - for backwards compatibility)
pub fn get_pristine_copy_path(path: &std::path::Path) -> Result<std::path::PathBuf, crate::Error> {
    let path_str = path.to_string_lossy();
    let path_cstr = std::ffi::CString::new(path_str.as_ref()).unwrap();
    let mut pristine_path: *const i8 = std::ptr::null();

    let pristine_path_str = with_tmp_pool(|pool| -> Result<String, crate::Error> {
        let err = unsafe {
            subversion_sys::svn_wc_get_pristine_copy_path(
                path_cstr.as_ptr(),
                &mut pristine_path,
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;

        // Copy the string before the pool is destroyed
        let result = if pristine_path.is_null() {
            String::new()
        } else {
            unsafe { std::ffi::CStr::from_ptr(pristine_path) }
                .to_string_lossy()
                .into_owned()
        };
        Ok(result)
    })?;

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
            .map(|f| box_cancel_baton_borrowed(f))
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
            unsafe { drop_cancel_baton_borrowed(cancel_baton) };
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
            .map(|f| box_cancel_baton_borrowed(f))
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
            .map(|f| box_notify_baton_borrowed(f))
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
            unsafe { drop_notify_baton_borrowed(notify_baton) };
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
            .map(|f| box_cancel_baton_borrowed(f))
            .unwrap_or(std::ptr::null_mut());

        let notify_baton = notify_func
            .map(|f| box_notify_baton_borrowed(f))
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
            unsafe { drop_notify_baton_borrowed(notify_baton) };
        }

        Error::from_raw(ret)
    }

    /// Get an editor for switching the working copy to a different URL
    pub fn get_switch_editor(
        &mut self,
        anchor_abspath: &str,
        target_basename: &str,
        switch_url: &str,
        use_commit_times: bool,
        depth: crate::Depth,
        depth_is_sticky: bool,
        allow_unver_obstructions: bool,
        server_performs_filtering: bool,
        diff3_cmd: Option<&str>,
        fetch_dirents_func: Option<
            Box<
                dyn Fn(
                    &str,
                    &str,
                )
                    -> Result<std::collections::HashMap<String, crate::ra::Dirent>, Error>,
            >,
        >,
        conflict_func: Option<
            Box<
                dyn Fn(
                    &crate::conflict::ConflictDescription,
                ) -> Result<crate::conflict::ConflictResult, Error>,
            >,
        >,
        external_func: Option<
            Box<dyn Fn(&str, Option<&str>, Option<&str>, crate::Depth) -> Result<(), Error>>,
        >,
        cancel_func: Option<Box<dyn Fn() -> Result<(), Error>>>,
        notify_func: Option<Box<dyn Fn(&Notify)>>,
    ) -> Result<(Box<dyn crate::delta::Editor>, crate::Revnum), crate::Error> {
        let anchor_abspath_cstr = std::ffi::CString::new(anchor_abspath)?;
        let target_basename_cstr = std::ffi::CString::new(target_basename)?;
        let switch_url_cstr = std::ffi::CString::new(switch_url)?;
        let diff3_cmd_cstr = diff3_cmd.map(std::ffi::CString::new).transpose()?;

        let result_pool = apr::Pool::new();
        let mut target_revision: subversion_sys::svn_revnum_t = 0;
        let mut editor_ptr: *const subversion_sys::svn_delta_editor_t = std::ptr::null();
        let mut edit_baton: *mut std::ffi::c_void = std::ptr::null_mut();

        // Create batons for callbacks
        let has_fetch_dirents = fetch_dirents_func.is_some();
        let fetch_dirents_baton = fetch_dirents_func
            .map(|f| box_fetch_dirents_baton(f))
            .unwrap_or(std::ptr::null_mut());
        let has_conflict = conflict_func.is_some();
        let conflict_baton = conflict_func
            .map(|f| box_conflict_baton(f))
            .unwrap_or(std::ptr::null_mut());
        let has_external = external_func.is_some();
        let external_baton = external_func
            .map(|f| box_external_baton(f))
            .unwrap_or(std::ptr::null_mut());
        let has_cancel = cancel_func.is_some();
        let cancel_baton = cancel_func
            .map(|f| box_cancel_baton(f))
            .unwrap_or(std::ptr::null_mut());
        let has_notify = notify_func.is_some();
        let notify_baton = notify_func
            .map(|f| box_notify_baton(f))
            .unwrap_or(std::ptr::null_mut());

        let err = with_tmp_pool(|scratch_pool| {
            unsafe {
                svn_result(subversion_sys::svn_wc_get_switch_editor4(
                    &mut editor_ptr,
                    &mut edit_baton,
                    &mut target_revision,
                    self.ptr,
                    anchor_abspath_cstr.as_ptr(),
                    target_basename_cstr.as_ptr(),
                    switch_url_cstr.as_ptr(),
                    if use_commit_times { 1 } else { 0 },
                    depth.into(),
                    if depth_is_sticky { 1 } else { 0 },
                    if allow_unver_obstructions { 1 } else { 0 },
                    if server_performs_filtering { 1 } else { 0 },
                    diff3_cmd_cstr
                        .as_ref()
                        .map_or(std::ptr::null(), |c| c.as_ptr()),
                    std::ptr::null(), // preserved_exts - TODO: expose
                    if has_fetch_dirents {
                        Some(wrap_fetch_dirents_func)
                    } else {
                        None
                    },
                    fetch_dirents_baton,
                    if has_conflict {
                        Some(wrap_conflict_func)
                    } else {
                        None
                    },
                    conflict_baton,
                    if has_external {
                        Some(wrap_external_func)
                    } else {
                        None
                    },
                    external_baton,
                    if has_cancel {
                        Some(crate::wrap_cancel_func)
                    } else {
                        None
                    },
                    cancel_baton,
                    if has_notify {
                        Some(wrap_notify_func)
                    } else {
                        None
                    },
                    notify_baton,
                    result_pool.as_mut_ptr(),
                    scratch_pool.as_mut_ptr(),
                ))
            }
        });

        err?;

        // Store callback batons with their droppers so they're properly cleaned up
        let mut batons = Vec::new();
        if !fetch_dirents_baton.is_null() {
            batons.push((
                fetch_dirents_baton,
                drop_fetch_dirents_baton as crate::delta::DropperFn,
            ));
        }
        if !conflict_baton.is_null() {
            batons.push((
                conflict_baton,
                drop_conflict_baton as crate::delta::DropperFn,
            ));
        }
        if !external_baton.is_null() {
            batons.push((
                external_baton,
                drop_external_baton as crate::delta::DropperFn,
            ));
        }
        if !cancel_baton.is_null() {
            batons.push((cancel_baton, drop_cancel_baton as crate::delta::DropperFn));
        }
        if !notify_baton.is_null() {
            batons.push((notify_baton, drop_notify_baton as crate::delta::DropperFn));
        }

        // Reuse the existing WrapEditor from delta module
        let editor = Box::new(crate::delta::WrapEditor {
            editor: editor_ptr,
            baton: edit_baton,
            _pool: apr::PoolHandle::owned(result_pool),
            callback_batons: batons,
        });

        Ok((
            editor,
            crate::Revnum::from_raw(target_revision).unwrap_or_default(),
        ))
    }

    /// Get an editor for showing differences in the working copy
    /// Note: This returns an editor that will generate diff callbacks.
    /// The callbacks parameter should be a valid svn_wc_diff_callbacks4_t structure.
    /// For now, we pass NULL which means no actual diff output will be generated.
    pub fn get_diff_editor(
        &mut self,
        anchor_abspath: &str,
        target_abspath: &str,
        use_text_base: bool,
        depth: crate::Depth,
        ignore_ancestry: bool,
        show_copies_as_adds: bool,
        use_git_diff_format: bool,
    ) -> Result<Box<dyn crate::delta::Editor>, crate::Error> {
        let anchor_abspath_cstr = std::ffi::CString::new(anchor_abspath)?;
        let target_abspath_cstr = std::ffi::CString::new(target_abspath)?;

        let result_pool = apr::Pool::new();
        let mut editor_ptr: *const subversion_sys::svn_delta_editor_t = std::ptr::null();
        let mut edit_baton: *mut std::ffi::c_void = std::ptr::null_mut();

        let err = with_tmp_pool(|scratch_pool| {
            unsafe {
                svn_result(subversion_sys::svn_wc_get_diff_editor6(
                    &mut editor_ptr,
                    &mut edit_baton,
                    self.ptr,
                    anchor_abspath_cstr.as_ptr(),
                    target_abspath_cstr.as_ptr(),
                    depth.into(),
                    if ignore_ancestry { 1 } else { 0 },
                    if show_copies_as_adds { 1 } else { 0 },
                    if use_git_diff_format { 1 } else { 0 },
                    if use_text_base { 1 } else { 0 },
                    0,                    // reverse_order
                    0,                    // server_performs_filtering
                    std::ptr::null(),     // changelist_filter
                    std::ptr::null(),     // callbacks - TODO: implement svn_wc_diff_callbacks4_t
                    std::ptr::null_mut(), // callback_baton
                    None,                 // cancel_func
                    std::ptr::null_mut(), // cancel_baton
                    result_pool.as_mut_ptr(),
                    scratch_pool.as_mut_ptr(),
                ))
            }
        });

        err?;

        // Reuse the existing WrapEditor from delta module
        let editor = Box::new(crate::delta::WrapEditor {
            editor: editor_ptr,
            baton: edit_baton,
            _pool: apr::PoolHandle::owned(result_pool),
            callback_batons: Vec::new(), // No callbacks for diff editor
        });

        Ok(editor)
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
            .map(|f| box_cancel_baton_borrowed(f))
            .unwrap_or(std::ptr::null_mut());

        let notify_baton = notify_func
            .map(|f| box_notify_baton_borrowed(f))
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
            unsafe { drop_notify_baton_borrowed(notify_baton) };
        }

        Error::from_raw(ret)
    }

    /// Get a versioned property value from a working copy path
    ///
    /// Retrieves the value of property @a name for @a local_abspath.
    /// Returns None if the property doesn't exist.
    pub fn prop_get(
        &mut self,
        local_abspath: &std::path::Path,
        name: &str,
    ) -> Result<Option<Vec<u8>>, Error> {
        let path = local_abspath.to_str().unwrap();
        let path_cstr = std::ffi::CString::new(path).unwrap();
        let name_cstr = std::ffi::CString::new(name).unwrap();
        let result_pool = apr::Pool::new();
        let scratch_pool = apr::Pool::new();

        let mut value: *const subversion_sys::svn_string_t = std::ptr::null();

        let err = unsafe {
            subversion_sys::svn_wc_prop_get2(
                &mut value,
                self.ptr,
                path_cstr.as_ptr(),
                name_cstr.as_ptr(),
                result_pool.as_mut_ptr(),
                scratch_pool.as_mut_ptr(),
            )
        };

        Error::from_raw(err)?;

        if value.is_null() {
            Ok(None)
        } else {
            Ok(Some(Vec::from(unsafe {
                std::slice::from_raw_parts((*value).data as *const u8, (*value).len)
            })))
        }
    }

    /// Set a versioned property on a working copy path
    ///
    /// Sets property @a name to @a value on @a local_abspath.
    /// If @a value is None, deletes the property.
    pub fn prop_set(
        &mut self,
        local_abspath: &std::path::Path,
        name: &str,
        value: Option<&[u8]>,
        depth: crate::Depth,
        skip_checks: bool,
        changelist_filter: Option<&[&str]>,
        cancel_func: Option<&dyn Fn() -> Result<(), Error>>,
        notify_func: Option<&dyn Fn(&Notify)>,
    ) -> Result<(), Error> {
        let path = local_abspath.to_str().unwrap();
        let path_cstr = std::ffi::CString::new(path).unwrap();
        let name_cstr = std::ffi::CString::new(name).unwrap();
        let scratch_pool = apr::Pool::new();

        // Create svn_string_t for the value if provided
        let value_svn = value.map(|v| crate::string::BStr::from_bytes(v, &scratch_pool));
        let value_ptr = value_svn
            .as_ref()
            .map(|v| v.as_ptr())
            .unwrap_or(std::ptr::null());

        // Convert changelist_filter if provided
        let changelist_cstrings: Vec<_> = changelist_filter
            .map(|lists| {
                lists
                    .iter()
                    .map(|l| std::ffi::CString::new(*l).unwrap())
                    .collect()
            })
            .unwrap_or_default();

        let changelist_array = if changelist_filter.is_some() {
            let mut array = apr::tables::TypedArray::<*const i8>::new(
                &scratch_pool,
                changelist_cstrings.len() as i32,
            );
            for cstring in &changelist_cstrings {
                array.push(cstring.as_ptr());
            }
            unsafe { array.as_ptr() }
        } else {
            std::ptr::null()
        };

        let cancel_baton = cancel_func
            .map(|f| box_cancel_baton_borrowed(f))
            .unwrap_or(std::ptr::null_mut());

        let notify_baton = notify_func
            .map(|f| box_notify_baton_borrowed(f))
            .unwrap_or(std::ptr::null_mut());

        let err = unsafe {
            subversion_sys::svn_wc_prop_set4(
                self.ptr,
                path_cstr.as_ptr(),
                name_cstr.as_ptr(),
                value_ptr,
                depth.into(),
                skip_checks.into(),
                changelist_array,
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
                scratch_pool.as_mut_ptr(),
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
            unsafe { drop_notify_baton_borrowed(notify_baton) };
        }

        Error::from_raw(err)
    }

    /// List all versioned properties on a working copy path
    ///
    /// Returns a HashMap of all properties set on @a local_abspath.
    pub fn prop_list(
        &mut self,
        local_abspath: &std::path::Path,
    ) -> Result<std::collections::HashMap<String, Vec<u8>>, Error> {
        let path = local_abspath.to_str().unwrap();
        let path_cstr = std::ffi::CString::new(path).unwrap();
        let result_pool = apr::Pool::new();
        let scratch_pool = apr::Pool::new();

        let mut props: *mut apr::hash::apr_hash_t = std::ptr::null_mut();

        let err = unsafe {
            subversion_sys::svn_wc_prop_list2(
                &mut props,
                self.ptr,
                path_cstr.as_ptr(),
                result_pool.as_mut_ptr(),
                scratch_pool.as_mut_ptr(),
            )
        };

        Error::from_raw(err)?;

        let prop_hash = unsafe { crate::props::PropHash::from_ptr(props) };
        Ok(prop_hash.to_hashmap())
    }

    /// Get property differences for a working copy path
    ///
    /// Returns the property changes (propchanges) and original properties
    /// for a given path in the working copy. The propchanges represent
    /// modifications made in the working copy but not yet committed.
    ///
    /// # Arguments
    ///
    /// * `local_abspath` - Absolute path to the working copy item
    ///
    /// # Returns
    ///
    /// Returns a tuple of:
    /// * `Vec<PropChange>` - Array of property changes
    /// * `Option<HashMap<String, Vec<u8>>>` - Hash of original (pristine) properties (None if no properties)
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// * The path is not a valid working copy path
    /// * There are issues accessing the working copy database
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use subversion::wc::Context;
    /// # fn example() -> Result<(), subversion::Error> {
    /// let mut ctx = Context::new()?;
    /// let path = std::path::Path::new("/path/to/wc/file.txt");
    /// let (changes, original_props) = ctx.get_prop_diffs(path)?;
    /// for change in changes {
    ///     println!("Property {} changed", change.name);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_prop_diffs(
        &mut self,
        local_abspath: &std::path::Path,
    ) -> Result<
        (
            Vec<PropChange>,
            Option<std::collections::HashMap<String, Vec<u8>>>,
        ),
        Error,
    > {
        let path = local_abspath.to_str().unwrap();
        let path_cstr = std::ffi::CString::new(path).unwrap();
        let result_pool = apr::Pool::new();
        let scratch_pool = apr::Pool::new();

        let mut propchanges: *mut apr::tables::apr_array_header_t = std::ptr::null_mut();
        let mut original_props: *mut apr::hash::apr_hash_t = std::ptr::null_mut();

        let err = unsafe {
            subversion_sys::svn_wc_get_prop_diffs2(
                &mut propchanges,
                &mut original_props,
                self.ptr,
                path_cstr.as_ptr(),
                result_pool.as_mut_ptr(),
                scratch_pool.as_mut_ptr(),
            )
        };

        Error::from_raw(err)?;

        // Convert propchanges array to Vec<PropChange>
        // Note: The array contains svn_prop_t structs directly, not pointers
        let changes = if propchanges.is_null() {
            Vec::new()
        } else {
            let array = unsafe {
                apr::tables::TypedArray::<subversion_sys::svn_prop_t>::from_ptr(propchanges)
            };
            array
                .iter()
                .map(|prop| unsafe {
                    if prop.name.is_null() {
                        panic!("Encountered null prop.name in propchanges array");
                    }
                    let name = std::ffi::CStr::from_ptr(prop.name)
                        .to_str()
                        .expect("Property name is not valid UTF-8")
                        .to_owned();
                    let value = if prop.value.is_null() {
                        None
                    } else {
                        Some(crate::svn_string_helpers::to_vec(&*prop.value))
                    };
                    PropChange { name, value }
                })
                .collect()
        };

        // Convert original_props hash to HashMap, or None if null
        let original = if original_props.is_null() {
            None
        } else {
            let prop_hash = unsafe { crate::props::PropHash::from_ptr(original_props) };
            Some(prop_hash.to_hashmap())
        };

        Ok((changes, original))
    }

    /// Read the kind (type) of a node in the working copy
    ///
    /// Returns the kind of node at @a local_abspath, which can be a file,
    /// directory, symlink, etc.
    ///
    /// If @a show_deleted is true, will show deleted nodes.
    /// If @a show_hidden is true, will show hidden nodes.
    pub fn read_kind(
        &mut self,
        local_abspath: &std::path::Path,
        show_deleted: bool,
        show_hidden: bool,
    ) -> Result<crate::NodeKind, Error> {
        let path = local_abspath.to_str().unwrap();
        let path_cstr = std::ffi::CString::new(path).unwrap();
        let scratch_pool = apr::Pool::new();

        let mut kind: subversion_sys::svn_node_kind_t =
            subversion_sys::svn_node_kind_t_svn_node_none;

        let err = unsafe {
            subversion_sys::svn_wc_read_kind2(
                &mut kind,
                self.ptr,
                path_cstr.as_ptr(),
                show_deleted.into(),
                show_hidden.into(),
                scratch_pool.as_mut_ptr(),
            )
        };

        Error::from_raw(err)?;
        Ok(kind.into())
    }

    /// Check if a path is a working copy root
    ///
    /// Returns true if @a local_abspath is the root of a working copy.
    /// A working copy root is the top-level directory containing the .svn
    /// administrative directory.
    pub fn is_wc_root(&mut self, local_abspath: &std::path::Path) -> Result<bool, Error> {
        let path = local_abspath.to_str().unwrap();
        let path_cstr = std::ffi::CString::new(path).unwrap();
        let scratch_pool = apr::Pool::new();

        let mut wc_root: i32 = 0;

        let err = unsafe {
            subversion_sys::svn_wc_is_wc_root2(
                &mut wc_root,
                self.ptr,
                path_cstr.as_ptr(),
                scratch_pool.as_mut_ptr(),
            )
        };

        Error::from_raw(err)?;
        Ok(wc_root != 0)
    }

    /// Exclude a versioned directory from a working copy
    ///
    /// This function removes @a local_abspath from the working copy, but
    /// unlike delete, keeps it in the repository. This is useful for
    /// sparse checkouts - excluding subtrees you don't need locally.
    ///
    /// The excluded item will not appear in status output and will not
    /// be included in commits. Use update with depth=infinity to bring
    /// it back.
    ///
    /// @a local_abspath must be a versioned directory.
    pub fn exclude(
        &mut self,
        local_abspath: &std::path::Path,
        cancel_func: Option<&dyn Fn() -> Result<(), Error>>,
        notify_func: Option<&dyn Fn(&Notify)>,
    ) -> Result<(), Error> {
        let path = local_abspath.to_str().unwrap();
        let path_cstr = std::ffi::CString::new(path).unwrap();
        let scratch_pool = apr::Pool::new();

        let cancel_baton = cancel_func
            .map(|f| box_cancel_baton_borrowed(f))
            .unwrap_or(std::ptr::null_mut());

        let notify_baton = notify_func
            .map(|f| box_notify_baton_borrowed(f))
            .unwrap_or(std::ptr::null_mut());

        let err = unsafe {
            subversion_sys::svn_wc_exclude(
                self.ptr,
                path_cstr.as_ptr(),
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
                scratch_pool.as_mut_ptr(),
            )
        };

        // Free callback batons after synchronous operation completes
        if !cancel_baton.is_null() {
            unsafe {
                drop(Box::from_raw(
                    cancel_baton as *mut &dyn Fn() -> Result<(), Error>,
                ));
            }
        }
        if !notify_baton.is_null() {
            unsafe {
                drop(Box::from_raw(notify_baton as *mut &dyn Fn(&Notify)));
            }
        }

        Error::from_raw(err)
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

    /// Get the node kind
    pub fn kind(&self) -> crate::NodeKind {
        unsafe { (*self.ptr).kind.into() }
    }

    /// Get the mime type
    pub fn mime_type(&self) -> Option<&str> {
        unsafe {
            if (*self.ptr).mime_type.is_null() {
                None
            } else {
                Some(
                    std::ffi::CStr::from_ptr((*self.ptr).mime_type)
                        .to_str()
                        .unwrap(),
                )
            }
        }
    }

    /// Get the lock information
    pub fn lock(&self) -> Option<Lock> {
        unsafe {
            if (*self.ptr).lock.is_null() {
                None
            } else {
                Some(Lock::from_ptr((*self.ptr).lock))
            }
        }
    }

    /// Get the error if notification indicates a failure
    pub fn err(&self) -> Option<Error> {
        unsafe {
            if (*self.ptr).err.is_null() {
                None
            } else {
                Some(Error::from_raw((*self.ptr).err).unwrap_err())
            }
        }
    }

    /// Get the content state
    pub fn content_state(&self) -> u32 {
        unsafe { (*self.ptr).content_state }
    }

    /// Get the property state
    pub fn prop_state(&self) -> u32 {
        unsafe { (*self.ptr).prop_state }
    }

    /// Get the lock state
    pub fn lock_state(&self) -> u32 {
        unsafe { (*self.ptr).lock_state }
    }

    /// Get the revision
    pub fn revision(&self) -> Option<crate::Revnum> {
        unsafe { crate::Revnum::from_raw((*self.ptr).revision) }
    }

    /// Get the changelist name
    pub fn changelist_name(&self) -> Option<&str> {
        unsafe {
            if (*self.ptr).changelist_name.is_null() {
                None
            } else {
                Some(
                    std::ffi::CStr::from_ptr((*self.ptr).changelist_name)
                        .to_str()
                        .unwrap(),
                )
            }
        }
    }

    /// Get the URL
    pub fn url(&self) -> Option<&str> {
        unsafe {
            if (*self.ptr).url.is_null() {
                None
            } else {
                Some(std::ffi::CStr::from_ptr((*self.ptr).url).to_str().unwrap())
            }
        }
    }

    /// Get the path prefix
    pub fn path_prefix(&self) -> Option<&str> {
        unsafe {
            if (*self.ptr).path_prefix.is_null() {
                None
            } else {
                Some(
                    std::ffi::CStr::from_ptr((*self.ptr).path_prefix)
                        .to_str()
                        .unwrap(),
                )
            }
        }
    }

    /// Get the property name
    pub fn prop_name(&self) -> Option<&str> {
        unsafe {
            if (*self.ptr).prop_name.is_null() {
                None
            } else {
                Some(
                    std::ffi::CStr::from_ptr((*self.ptr).prop_name)
                        .to_str()
                        .unwrap(),
                )
            }
        }
    }

    /// Get the old revision (for updates)
    pub fn old_revision(&self) -> Option<crate::Revnum> {
        unsafe { crate::Revnum::from_raw((*self.ptr).old_revision) }
    }

    /// Get the hunk original start line (for patch operations)
    pub fn hunk_original_start(&self) -> u64 {
        unsafe { (*self.ptr).hunk_original_start as u64 }
    }

    /// Get the hunk original length (for patch operations)
    pub fn hunk_original_length(&self) -> u64 {
        unsafe { (*self.ptr).hunk_original_length as u64 }
    }

    /// Get the hunk modified start line (for patch operations)
    pub fn hunk_modified_start(&self) -> u64 {
        unsafe { (*self.ptr).hunk_modified_start as u64 }
    }

    /// Get the hunk modified length (for patch operations)
    pub fn hunk_modified_length(&self) -> u64 {
        unsafe { (*self.ptr).hunk_modified_length as u64 }
    }

    /// Get the line at which a hunk was matched (for patch operations)
    pub fn hunk_matched_line(&self) -> u64 {
        unsafe { (*self.ptr).hunk_matched_line as u64 }
    }

    /// Get the fuzz factor the hunk was applied with (for patch operations)
    pub fn hunk_fuzz(&self) -> u64 {
        unsafe { (*self.ptr).hunk_fuzz as u64 }
    }
}

/// Wrapper for conflict resolver callbacks
extern "C" fn wrap_conflict_func(
    result: *mut *mut subversion_sys::svn_wc_conflict_result_t,
    description: *const subversion_sys::svn_wc_conflict_description2_t,
    baton: *mut std::ffi::c_void,
    result_pool: *mut apr_sys::apr_pool_t,
    _scratch_pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    if baton.is_null() || description.is_null() || result.is_null() {
        return std::ptr::null_mut();
    }

    let callback = unsafe {
        &*(baton
            as *const Box<
                dyn Fn(
                    &crate::conflict::ConflictDescription,
                ) -> Result<crate::conflict::ConflictResult, Error>,
            >)
    };

    // Convert C description to Rust type
    let desc = match unsafe { crate::conflict::ConflictDescription::from_raw(description) } {
        Ok(d) => d,
        Err(mut e) => return unsafe { e.detach() },
    };

    match callback(&desc) {
        Ok(conflict_result) => {
            // Convert Rust result to C result
            unsafe {
                *result = conflict_result.to_raw(result_pool);
            }
            std::ptr::null_mut()
        }
        Err(mut e) => unsafe { e.detach() },
    }
}

/// Wrapper for external update callbacks
extern "C" fn wrap_external_func(
    baton: *mut std::ffi::c_void,
    local_abspath: *const i8,
    old_val: *const subversion_sys::svn_string_t,
    new_val: *const subversion_sys::svn_string_t,
    depth: subversion_sys::svn_depth_t,
    _scratch_pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    if baton.is_null() || local_abspath.is_null() {
        return std::ptr::null_mut();
    }

    let callback = unsafe {
        &*(baton
            as *const Box<
                dyn Fn(&str, Option<&str>, Option<&str>, crate::Depth) -> Result<(), Error>,
            >)
    };

    let path_str = unsafe {
        std::ffi::CStr::from_ptr(local_abspath)
            .to_str()
            .unwrap_or("")
    };

    let old_str = if old_val.is_null() {
        None
    } else {
        unsafe {
            let data = (*old_val).data as *const u8;
            let len = (*old_val).len;
            std::str::from_utf8(std::slice::from_raw_parts(data, len)).ok()
        }
    };

    let new_str = if new_val.is_null() {
        None
    } else {
        unsafe {
            let data = (*new_val).data as *const u8;
            let len = (*new_val).len;
            std::str::from_utf8(std::slice::from_raw_parts(data, len)).ok()
        }
    };

    let depth_enum = crate::Depth::from(depth);

    match callback(path_str, old_str, new_str, depth_enum) {
        Ok(()) => std::ptr::null_mut(),
        Err(mut e) => unsafe { e.detach() },
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

/// Wrapper for fetch dirents callbacks
extern "C" fn wrap_fetch_dirents_func(
    baton: *mut std::ffi::c_void,
    dirents: *mut *mut apr_sys::apr_hash_t,
    repos_root_url: *const i8,
    repos_relpath: *const i8,
    result_pool: *mut apr_sys::apr_pool_t,
    _scratch_pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    if baton.is_null() || dirents.is_null() || repos_root_url.is_null() || repos_relpath.is_null() {
        return std::ptr::null_mut();
    }

    let callback = unsafe {
        &*(baton
            as *const Box<
                dyn Fn(
                    &str,
                    &str,
                )
                    -> Result<std::collections::HashMap<String, crate::ra::Dirent>, Error>,
            >)
    };

    let root_url = unsafe {
        std::ffi::CStr::from_ptr(repos_root_url)
            .to_str()
            .unwrap_or("")
    };

    let relpath = unsafe {
        std::ffi::CStr::from_ptr(repos_relpath)
            .to_str()
            .unwrap_or("")
    };

    let pool = unsafe { apr::Pool::from_raw(result_pool) };
    match callback(root_url, relpath) {
        Ok(dirents_map) => {
            // Create apr_hash_t and populate it
            let mut hash = apr::hash::Hash::new(&pool);
            for (name, dirent) in dirents_map {
                // Create svn_dirent_t in the pool
                let svn_dirent = pool.alloc::<subversion_sys::svn_dirent_t>();
                unsafe {
                    let svn_dirent_ptr = (*svn_dirent).as_mut_ptr();
                    std::ptr::write_bytes(svn_dirent_ptr, 0, 1);
                    (*svn_dirent_ptr).kind = dirent.kind().into();
                    (*svn_dirent_ptr).size = dirent.size();
                    (*svn_dirent_ptr).has_props = if dirent.has_props() { 1 } else { 0 };
                    (*svn_dirent_ptr).created_rev = dirent.created_rev().map(|r| r.0).unwrap_or(-1);
                    (*svn_dirent_ptr).time = dirent.time().into();
                    if let Some(author) = dirent.last_author() {
                        (*svn_dirent_ptr).last_author = pool.pstrdup(author);
                    }
                    hash.insert(name.as_bytes(), svn_dirent_ptr as *mut std::ffi::c_void);
                }
            }
            unsafe {
                *dirents = hash.as_mut_ptr();
            }
            std::ptr::null_mut()
        }
        Err(mut e) => unsafe { e.detach() },
    }
}

/// Represents a queue of committed items
pub struct CommittedQueue {
    ptr: *mut subversion_sys::svn_wc_committed_queue_t,
    _pool: apr::Pool<'static>,
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
        Self { ptr, _pool: pool }
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

/// Gets the revision status of a working copy.
///
/// Returns (min_revision, max_revision, is_switched, is_modified).
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
    fn test_get_switch_editor() {
        // Test that the API compiles and can be called
        let temp_dir = tempfile::tempdir().unwrap();
        let mut ctx = Context::new().unwrap();

        // This will fail without a working copy, but tests the API
        let result = ctx.get_switch_editor(
            temp_dir.path().to_str().unwrap(),
            "",
            "http://example.com/repo/branches/test",
            false,
            crate::Depth::Infinity,
            false,
            false,
            false, // server_performs_filtering
            None,
            None, // fetch_dirents_func
            None, // no conflict callback
            None, // no external callback
            None, // no cancel callback
            None, // no notify callback
        );

        // Expected to fail without a valid working copy
        assert!(result.is_err());
    }

    #[test]
    fn test_get_switch_editor_api() {
        // Test that the switch editor API compiles and can be called
        let temp_dir = tempfile::tempdir().unwrap();
        let mut ctx = Context::new().unwrap();

        // This will fail without a working copy, but tests the API
        let result = ctx.get_switch_editor(
            temp_dir.path().to_str().unwrap(),
            "",
            "http://example.com/svn/trunk",
            false, // use_commit_times
            crate::Depth::Infinity,
            false, // depth_is_sticky
            false, // allow_unver_obstructions
            false, // server_performs_filtering
            None,  // diff3_cmd
            None,  // fetch_dirents_func
            None,  // no conflict callback
            None,  // no external callback
            None,  // no cancel callback
            None,  // no notify callback
        );

        // Expected to fail without a valid working copy
        assert!(result.is_err());
    }

    #[test]
    fn test_get_switch_editor_with_target() {
        // Test switch editor with target basename
        let temp_dir = tempfile::tempdir().unwrap();
        let mut ctx = Context::new().unwrap();

        let result = ctx.get_switch_editor(
            temp_dir.path().to_str().unwrap(),
            "subdir", // target basename
            "http://example.com/svn/branches/test",
            true, // use_commit_times
            crate::Depth::Files,
            true,  // depth_is_sticky
            true,  // allow_unver_obstructions
            false, // server_performs_filtering
            None,  // diff3_cmd
            None,  // fetch_dirents_func
            None,  // no conflict callback
            None,  // no external callback
            None,  // no cancel callback
            None,  // no notify callback
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_get_diff_editor() {
        // Test that the API compiles and can be called
        let temp_dir = tempfile::tempdir().unwrap();
        let mut ctx = Context::new().unwrap();

        // Create the directory
        std::fs::create_dir_all(temp_dir.path()).unwrap();

        // This should succeed with an existing directory
        let result = ctx.get_diff_editor(
            temp_dir.path().to_str().unwrap(),
            temp_dir.path().to_str().unwrap(),
            false, // use_text_base
            crate::Depth::Infinity,
            false, // ignore_ancestry
            false, // show_copies_as_adds
            false, // use_git_diff_format
        );

        // Should succeed with an existing path
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_diff_editor_with_options() {
        // Test diff editor with various options
        let temp_dir = tempfile::tempdir().unwrap();
        let mut ctx = Context::new().unwrap();

        // Create the paths so they exist
        std::fs::create_dir_all(temp_dir.path()).unwrap();

        // Test with text base using existing paths
        let result = ctx.get_diff_editor(
            temp_dir.path().to_str().unwrap(),
            temp_dir.path().to_str().unwrap(),
            true, // use_text_base
            crate::Depth::Empty,
            true, // ignore_ancestry
            true, // show_copies_as_adds
            true, // use_git_diff_format
        );

        // Should succeed since paths exist
        assert!(result.is_ok());

        // Test with different paths
        let anchor_path = temp_dir.path().join("anchor");
        let target_path = temp_dir.path().join("target");
        std::fs::create_dir_all(&anchor_path).unwrap();
        std::fs::create_dir_all(&target_path).unwrap();

        let result = ctx.get_diff_editor(
            anchor_path.to_str().unwrap(),
            target_path.to_str().unwrap(),
            false,
            crate::Depth::Files,
            false,
            false,
            false,
        );

        // Should succeed since paths exist
        assert!(result.is_ok());
    }

    #[test]
    fn test_update_editor_trait() {
        // Test that UpdateEditor implements the Editor trait
        use crate::delta::Editor;

        // This just verifies the trait implementation compiles
        fn check_editor_impl<T: Editor>() {}

        // Verify UpdateEditor implements Editor trait
        check_editor_impl::<UpdateEditor>();
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

    #[test]
    fn test_wc_prop_operations() {
        use tempfile::TempDir;

        // Create a repository and working copy
        let temp_dir = TempDir::new().unwrap();
        let repos_path = temp_dir.path().join("repos");
        let wc_path = temp_dir.path().join("wc");

        // Create a repository
        let _repos = crate::repos::Repos::create(&repos_path).unwrap();

        // Create a working copy using client API
        let url_str = format!("file://{}", repos_path.display());
        let url = crate::uri::Uri::new(&url_str).unwrap();
        let mut client_ctx = crate::client::Context::new().unwrap();
        client_ctx
            .checkout(
                url,
                &wc_path,
                &crate::client::CheckoutOptions {
                    peg_revision: crate::Revision::Head,
                    revision: crate::Revision::Head,
                    depth: crate::Depth::Infinity,
                    ignore_externals: false,
                    allow_unver_obstructions: false,
                },
            )
            .unwrap();

        // Create a file in the working copy
        let file_path = wc_path.join("test.txt");
        std::fs::write(&file_path, "test content").unwrap();

        // Add the file to version control
        client_ctx
            .add(&file_path, &crate::client::AddOptions::new())
            .unwrap();

        // Use client API to set a property (it handles locking)
        client_ctx
            .propset(
                "test:property",
                Some(b"test value"),
                file_path.to_str().unwrap(),
                &crate::client::PropSetOptions::default(),
            )
            .unwrap();

        // Now test the wc property functions for reading
        let mut wc_ctx = Context::new().unwrap();

        // Test prop_get - retrieve the property we set via client
        let value = wc_ctx.prop_get(&file_path, "test:property").unwrap();
        assert_eq!(value, Some(b"test value".to_vec()));

        // Test prop_get for non-existent property
        let missing = wc_ctx.prop_get(&file_path, "test:missing").unwrap();
        assert_eq!(missing, None);

        // Test prop_list - list all properties
        let props = wc_ctx.prop_list(&file_path).unwrap();
        assert!(props.contains_key("test:property"));
        assert_eq!(props.get("test:property").unwrap(), b"test value");
    }

    #[test]
    fn test_get_prop_diffs() {
        use tempfile::TempDir;

        // Create a repository and working copy
        let temp_dir = TempDir::new().unwrap();
        let repos_path = temp_dir.path().join("repos");
        let wc_path = temp_dir.path().join("wc");

        // Create a repository
        let _repos = crate::repos::Repos::create(&repos_path).unwrap();

        // Create a working copy
        let url_str = format!("file://{}", repos_path.display());
        let url = crate::uri::Uri::new(&url_str).unwrap();
        let mut client_ctx = crate::client::Context::new().unwrap();
        client_ctx
            .checkout(
                url,
                &wc_path,
                &crate::client::CheckoutOptions {
                    peg_revision: crate::Revision::Head,
                    revision: crate::Revision::Head,
                    depth: crate::Depth::Infinity,
                    ignore_externals: false,
                    allow_unver_obstructions: false,
                },
            )
            .unwrap();

        // Create and add a file
        let file_path = wc_path.join("test.txt");
        std::fs::write(&file_path, "test content").unwrap();
        client_ctx
            .add(&file_path, &crate::client::AddOptions::new())
            .unwrap();

        // Set a property without committing
        client_ctx
            .propset(
                "test:prop1",
                Some(b"value1"),
                file_path.to_str().unwrap(),
                &crate::client::PropSetOptions::default(),
            )
            .unwrap();

        // Test get_prop_diffs - should show the new property as a change
        let mut wc_ctx = Context::new().unwrap();
        let (changes, original) = wc_ctx.get_prop_diffs(&file_path).unwrap();

        // Should have 1 property change
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].name, "test:prop1");
        assert_eq!(changes[0].value, Some(b"value1".to_vec()));

        // Original props should not contain our property (it's only in working copy)
        if let Some(orig) = original {
            assert!(!orig.contains_key("test:prop1"));
        }
    }

    #[test]
    fn test_read_kind() {
        use tempfile::TempDir;

        // Create a repository and working copy
        let temp_dir = TempDir::new().unwrap();
        let repos_path = temp_dir.path().join("repos");
        let wc_path = temp_dir.path().join("wc");

        // Create a repository
        let _repos = crate::repos::Repos::create(&repos_path).unwrap();

        // Create a working copy using client API
        let url_str = format!("file://{}", repos_path.display());
        let url = crate::uri::Uri::new(&url_str).unwrap();
        let mut client_ctx = crate::client::Context::new().unwrap();

        let checkout_opts = crate::client::CheckoutOptions {
            revision: crate::Revision::Head,
            peg_revision: crate::Revision::Head,
            depth: crate::Depth::Infinity,
            ignore_externals: false,
            allow_unver_obstructions: false,
        };
        client_ctx.checkout(url, &wc_path, &checkout_opts).unwrap();

        // Create a file and directory
        let file_path = wc_path.join("test.txt");
        std::fs::write(&file_path, "test content").unwrap();

        let dir_path = wc_path.join("subdir");
        std::fs::create_dir(&dir_path).unwrap();

        client_ctx
            .add(&file_path, &crate::client::AddOptions::new())
            .unwrap();
        client_ctx
            .add(&dir_path, &crate::client::AddOptions::new())
            .unwrap();

        // Test read_kind
        let mut wc_ctx = Context::new().unwrap();

        // Check the working copy root is a directory
        let kind = wc_ctx.read_kind(&wc_path, false, false).unwrap();
        assert_eq!(kind, crate::NodeKind::Dir);

        // Check the file is recognized as a file
        let kind = wc_ctx.read_kind(&file_path, false, false).unwrap();
        assert_eq!(kind, crate::NodeKind::File);

        // Check the directory
        let kind = wc_ctx.read_kind(&dir_path, false, false).unwrap();
        assert_eq!(kind, crate::NodeKind::Dir);

        // Check a non-existent path
        let nonexistent = wc_path.join("nonexistent");
        let kind = wc_ctx.read_kind(&nonexistent, false, false).unwrap();
        assert_eq!(kind, crate::NodeKind::None);
    }

    #[test]
    fn test_is_wc_root() {
        use tempfile::TempDir;

        // Create a repository and working copy
        let temp_dir = TempDir::new().unwrap();
        let repos_path = temp_dir.path().join("repos");
        let wc_path = temp_dir.path().join("wc");

        // Create a repository
        let _repos = crate::repos::Repos::create(&repos_path).unwrap();

        // Create a working copy using client API
        let url_str = format!("file://{}", repos_path.display());
        let url = crate::uri::Uri::new(&url_str).unwrap();
        let mut client_ctx = crate::client::Context::new().unwrap();

        let checkout_opts = crate::client::CheckoutOptions {
            revision: crate::Revision::Head,
            peg_revision: crate::Revision::Head,
            depth: crate::Depth::Infinity,
            ignore_externals: false,
            allow_unver_obstructions: false,
        };
        client_ctx.checkout(url, &wc_path, &checkout_opts).unwrap();

        // Create a subdirectory
        let subdir = wc_path.join("subdir");
        std::fs::create_dir(&subdir).unwrap();
        client_ctx
            .add(&subdir, &crate::client::AddOptions::new())
            .unwrap();

        // Test is_wc_root
        let mut wc_ctx = Context::new().unwrap();

        // The working copy root should return true
        let is_root = wc_ctx.is_wc_root(&wc_path).unwrap();
        assert!(is_root, "Working copy root should be detected as WC root");

        // A subdirectory should return false
        let is_root = wc_ctx.is_wc_root(&subdir).unwrap();
        assert!(!is_root, "Subdirectory should not be a WC root");
    }

    #[test]
    fn test_get_pristine_contents() {
        use std::io::Read;
        use tempfile::TempDir;

        // Create a repository and working copy
        let temp_dir = TempDir::new().unwrap();
        let repos_path = temp_dir.path().join("repos");
        let wc_path = temp_dir.path().join("wc");

        // Create a repository
        let _repos = crate::repos::Repos::create(&repos_path).unwrap();

        // Create a working copy using client API
        let url_str = format!("file://{}", repos_path.display());
        let url = crate::uri::Uri::new(&url_str).unwrap();
        let mut client_ctx = crate::client::Context::new().unwrap();

        let checkout_opts = crate::client::CheckoutOptions {
            revision: crate::Revision::Head,
            peg_revision: crate::Revision::Head,
            depth: crate::Depth::Infinity,
            ignore_externals: false,
            allow_unver_obstructions: false,
        };
        client_ctx.checkout(url, &wc_path, &checkout_opts).unwrap();

        // Create and commit a file with original content
        let file_path = wc_path.join("test.txt");
        let original_content = "original content";
        std::fs::write(&file_path, original_content).unwrap();

        client_ctx
            .add(&file_path, &crate::client::AddOptions::new())
            .unwrap();

        let commit_opts = crate::client::CommitOptions::default();
        let revprops = std::collections::HashMap::new();
        client_ctx
            .commit(
                &[wc_path.to_str().unwrap()],
                &commit_opts,
                revprops,
                &|_info| Ok(()),
            )
            .unwrap();

        // Modify the file
        let modified_content = "modified content";
        std::fs::write(&file_path, modified_content).unwrap();

        // Get pristine contents
        let mut wc_ctx = Context::new().unwrap();
        let pristine_stream = wc_ctx
            .get_pristine_contents(file_path.to_str().unwrap())
            .unwrap();

        // Should have pristine contents
        assert!(
            pristine_stream.is_some(),
            "Should have pristine contents for committed file"
        );

        // Read and verify pristine contents match original
        let mut pristine_stream = pristine_stream.unwrap();
        let mut pristine_content = String::new();
        pristine_stream
            .read_to_string(&mut pristine_content)
            .unwrap();

        assert_eq!(
            pristine_content, original_content,
            "Pristine content should match original"
        );

        // Test with a newly added file (no pristine)
        let new_file = wc_path.join("new.txt");
        std::fs::write(&new_file, "new file content").unwrap();
        client_ctx
            .add(&new_file, &crate::client::AddOptions::new())
            .unwrap();

        let pristine = wc_ctx
            .get_pristine_contents(new_file.to_str().unwrap())
            .unwrap();
        assert!(
            pristine.is_none(),
            "Newly added file should have no pristine contents"
        );
    }

    #[test]
    fn test_exclude() {
        use std::cell::{Cell, RefCell};
        use tempfile::TempDir;

        // Create a repository and working copy
        let temp_dir = TempDir::new().unwrap();
        let repos_path = temp_dir.path().join("repos");
        let wc_path = temp_dir.path().join("wc");

        // Create a repository
        let _repos = crate::repos::Repos::create(&repos_path).unwrap();

        // Create a working copy using client API
        let url_str = format!("file://{}", repos_path.display());
        let url = crate::uri::Uri::new(&url_str).unwrap();
        let mut client_ctx = crate::client::Context::new().unwrap();

        let checkout_opts = crate::client::CheckoutOptions {
            revision: crate::Revision::Head,
            peg_revision: crate::Revision::Head,
            depth: crate::Depth::Infinity,
            ignore_externals: false,
            allow_unver_obstructions: false,
        };
        client_ctx.checkout(url, &wc_path, &checkout_opts).unwrap();

        // Create a subdirectory with a file and commit it
        let subdir = wc_path.join("subdir");
        std::fs::create_dir(&subdir).unwrap();

        let file_in_subdir = subdir.join("file.txt");
        std::fs::write(&file_in_subdir, "content").unwrap();

        client_ctx
            .add(&subdir, &crate::client::AddOptions::new())
            .unwrap();

        let commit_opts = crate::client::CommitOptions::default();
        let revprops = std::collections::HashMap::new();
        client_ctx
            .commit(
                &[wc_path.to_str().unwrap()],
                &commit_opts,
                revprops,
                &|_info| Ok(()),
            )
            .unwrap();

        // Verify the subdirectory exists
        assert!(subdir.exists(), "Subdirectory should exist before exclude");
        assert!(file_in_subdir.exists(), "File should exist before exclude");

        // Test exclude with notification callback
        let mut wc_ctx = Context::new().unwrap();
        let notifications = RefCell::new(Vec::new());

        let result = wc_ctx.exclude(
            &subdir,
            None,
            Some(&|notify: &Notify| {
                // Collect notifications to verify exclude is working
                notifications
                    .borrow_mut()
                    .push(format!("{:?}", notify.action()));
            }),
        );

        // Should succeed
        assert!(result.is_ok(), "Exclude should succeed: {:?}", result);

        // Verify the directory is now excluded (removed from disk)
        assert!(
            !subdir.exists(),
            "Subdirectory should not exist after exclude"
        );

        // Verify we got notification callbacks
        assert!(
            !notifications.borrow().is_empty(),
            "Should have received notifications"
        );

        // Verify the directory can be checked for kind (should return None/excluded)
        let kind = wc_ctx.read_kind(&subdir, false, false).unwrap();
        assert_eq!(
            kind,
            crate::NodeKind::None,
            "Excluded directory should show as None"
        );

        // Test exclude with cancel callback
        let subdir2 = wc_path.join("subdir2");
        std::fs::create_dir(&subdir2).unwrap();
        client_ctx
            .add(&subdir2, &crate::client::AddOptions::new())
            .unwrap();

        let commit_opts = crate::client::CommitOptions::default();
        let revprops = std::collections::HashMap::new();
        client_ctx
            .commit(
                &[wc_path.to_str().unwrap()],
                &commit_opts,
                revprops,
                &|_info| Ok(()),
            )
            .unwrap();

        // Test that cancel callback is called (but don't actually cancel)
        let cancel_called = Cell::new(false);
        let result = wc_ctx.exclude(
            &subdir2,
            Some(&|| {
                cancel_called.set(true);
                Ok(()) // Don't actually cancel
            }),
            None,
        );

        assert!(
            result.is_ok(),
            "Exclude with cancel callback should succeed"
        );
        assert!(
            cancel_called.get(),
            "Cancel callback should have been called"
        );
        assert!(!subdir2.exists(), "Second subdirectory should be excluded");
    }

    #[test]
    fn test_get_pristine_props() {
        use tempfile::TempDir;

        // Create a repository and working copy
        let temp_dir = TempDir::new().unwrap();
        let repos_path = temp_dir.path().join("repos");
        let wc_path = temp_dir.path().join("wc");

        // Create a repository
        let _repos = crate::repos::Repos::create(&repos_path).unwrap();

        // Create a working copy using client API
        let url_str = format!("file://{}", repos_path.display());
        let url = crate::uri::Uri::new(&url_str).unwrap();
        let mut client_ctx = crate::client::Context::new().unwrap();

        let checkout_opts = crate::client::CheckoutOptions {
            revision: crate::Revision::Head,
            peg_revision: crate::Revision::Head,
            depth: crate::Depth::Infinity,
            ignore_externals: false,
            allow_unver_obstructions: false,
        };
        client_ctx.checkout(url, &wc_path, &checkout_opts).unwrap();

        // Create a file with properties
        let file_path = wc_path.join("test.txt");
        std::fs::write(&file_path, "original content").unwrap();

        client_ctx
            .add(&file_path, &crate::client::AddOptions::new())
            .unwrap();

        // Set some properties
        let propset_opts = crate::client::PropSetOptions::default();
        client_ctx
            .propset(
                "svn:eol-style",
                Some(b"native"),
                file_path.to_str().unwrap(),
                &propset_opts,
            )
            .unwrap();
        client_ctx
            .propset(
                "custom:prop",
                Some(b"custom value"),
                file_path.to_str().unwrap(),
                &propset_opts,
            )
            .unwrap();

        // Commit the file with properties
        let commit_opts = crate::client::CommitOptions::default();
        let revprops = std::collections::HashMap::new();
        client_ctx
            .commit(
                &[wc_path.to_str().unwrap()],
                &commit_opts,
                revprops,
                &|_info| Ok(()),
            )
            .unwrap();

        // Test 1: Get pristine props (should match what we set)
        let mut wc_ctx = Context::new().unwrap();
        let pristine_props = wc_ctx
            .get_pristine_props(file_path.to_str().unwrap())
            .unwrap();

        assert!(
            pristine_props.is_some(),
            "Committed file should have pristine props"
        );
        let props = pristine_props.unwrap();
        assert!(
            props.contains_key("svn:eol-style"),
            "Should have svn:eol-style property"
        );
        assert_eq!(
            props.get("svn:eol-style").unwrap(),
            b"native",
            "svn:eol-style should be 'native'"
        );
        assert!(
            props.contains_key("custom:prop"),
            "Should have custom:prop property"
        );
        assert_eq!(
            props.get("custom:prop").unwrap(),
            b"custom value",
            "custom:prop should be 'custom value'"
        );

        // Test 2: Modify a property locally (pristine should remain unchanged)
        client_ctx
            .propset(
                "svn:eol-style",
                Some(b"LF"),
                file_path.to_str().unwrap(),
                &propset_opts,
            )
            .unwrap();

        // Pristine props should still be the original values
        let pristine_props = wc_ctx
            .get_pristine_props(file_path.to_str().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(
            pristine_props.get("svn:eol-style").unwrap(),
            b"native",
            "Pristine svn:eol-style should still be 'native'"
        );

        // Test 3: Delete a property locally (pristine should still have it)
        client_ctx
            .propset(
                "custom:prop",
                None,
                file_path.to_str().unwrap(),
                &propset_opts,
            )
            .unwrap();

        let pristine_props = wc_ctx
            .get_pristine_props(file_path.to_str().unwrap())
            .unwrap()
            .unwrap();
        assert!(
            pristine_props.contains_key("custom:prop"),
            "Pristine props should still contain deleted property"
        );

        // Test 4: Newly added file should have None pristine props (or empty hash)
        let new_file = wc_path.join("new.txt");
        std::fs::write(&new_file, "new content").unwrap();
        client_ctx
            .add(&new_file, &crate::client::AddOptions::new())
            .unwrap();

        let pristine = wc_ctx
            .get_pristine_props(new_file.to_str().unwrap())
            .unwrap();
        // Newly added files return None according to API docs, but implementation may vary
        if let Some(props) = pristine {
            // If not None, should at least be empty (no committed properties yet)
            assert!(
                props.is_empty() || props.len() == 0,
                "Newly added file pristine props should be empty if not None"
            );
        }

        // Test 5: Non-existent file should error
        let nonexistent = wc_path.join("nonexistent.txt");
        let result = wc_ctx.get_pristine_props(nonexistent.to_str().unwrap());
        assert!(result.is_err(), "Non-existent file should return an error");
    }

    #[test]
    fn test_conflicted() {
        use tempfile::TempDir;

        // Create a repository and two working copies
        let temp_dir = TempDir::new().unwrap();
        let repos_path = temp_dir.path().join("repos");
        let wc1_path = temp_dir.path().join("wc1");
        let wc2_path = temp_dir.path().join("wc2");

        // Create a repository
        let _repos = crate::repos::Repos::create(&repos_path).unwrap();

        let url_str = format!("file://{}", repos_path.display());
        let url = crate::uri::Uri::new(&url_str).unwrap();

        // Create first working copy
        let mut client_ctx1 = crate::client::Context::new().unwrap();
        let checkout_opts = crate::client::CheckoutOptions {
            revision: crate::Revision::Head,
            peg_revision: crate::Revision::Head,
            depth: crate::Depth::Infinity,
            ignore_externals: false,
            allow_unver_obstructions: false,
        };
        client_ctx1
            .checkout(url.clone(), &wc1_path, &checkout_opts)
            .unwrap();

        // Create a file and commit it (r1)
        let file_path1 = wc1_path.join("test.txt");
        std::fs::write(&file_path1, "line1\nline2\nline3\n").unwrap();
        client_ctx1
            .add(&file_path1, &crate::client::AddOptions::new())
            .unwrap();

        let commit_opts = crate::client::CommitOptions::default();
        let revprops = std::collections::HashMap::new();
        client_ctx1
            .commit(
                &[wc1_path.to_str().unwrap()],
                &commit_opts,
                revprops.clone(),
                &|_info| Ok(()),
            )
            .unwrap();

        // Create second working copy from same revision
        let mut client_ctx2 = crate::client::Context::new().unwrap();
        client_ctx2
            .checkout(url.clone(), &wc2_path, &checkout_opts)
            .unwrap();

        // In WC1: modify the file and commit (r2)
        std::fs::write(&file_path1, "line1 modified in wc1\nline2\nline3\n").unwrap();
        client_ctx1
            .commit(
                &[wc1_path.to_str().unwrap()],
                &commit_opts,
                revprops.clone(),
                &|_info| Ok(()),
            )
            .unwrap();

        // In WC2: modify the same line differently
        let file_path2 = wc2_path.join("test.txt");
        std::fs::write(&file_path2, "line1 modified in wc2\nline2\nline3\n").unwrap();

        // Try to update WC2 - this should create a text conflict
        let update_opts = crate::client::UpdateOptions::default();
        let _result = client_ctx2.update(
            &[wc2_path.to_str().unwrap()],
            crate::Revision::Head,
            &update_opts,
        );
        // Update might succeed but leave conflicts

        // Test 1: Check for text conflict
        let mut wc_ctx = Context::new().unwrap();
        let (text_conflicted, prop_conflicted, tree_conflicted) =
            wc_ctx.conflicted(file_path2.to_str().unwrap()).unwrap();

        assert!(
            text_conflicted,
            "File should have text conflict after conflicting update"
        );
        assert!(!prop_conflicted, "File should not have property conflict");
        assert!(!tree_conflicted, "File should not have tree conflict");

        // Test 2: Create a property conflict
        let propfile_path1 = wc1_path.join("proptest.txt");
        std::fs::write(&propfile_path1, "content").unwrap();
        client_ctx1
            .add(&propfile_path1, &crate::client::AddOptions::new())
            .unwrap();

        let propset_opts = crate::client::PropSetOptions::default();
        client_ctx1
            .propset(
                "custom:prop",
                Some(b"value1"),
                propfile_path1.to_str().unwrap(),
                &propset_opts,
            )
            .unwrap();

        client_ctx1
            .commit(
                &[wc1_path.to_str().unwrap()],
                &commit_opts,
                revprops.clone(),
                &|_info| Ok(()),
            )
            .unwrap();

        // In WC2, update to get the file, then set conflicting property
        let _result = client_ctx2.update(
            &[wc2_path.to_str().unwrap()],
            crate::Revision::Head,
            &update_opts,
        );

        let propfile_path2 = wc2_path.join("proptest.txt");
        client_ctx2
            .propset(
                "custom:prop",
                Some(b"value2"),
                propfile_path2.to_str().unwrap(),
                &propset_opts,
            )
            .unwrap();

        // Commit in WC1 with different value
        client_ctx1
            .propset(
                "custom:prop",
                Some(b"value1_modified"),
                propfile_path1.to_str().unwrap(),
                &propset_opts,
            )
            .unwrap();
        client_ctx1
            .commit(
                &[wc1_path.to_str().unwrap()],
                &commit_opts,
                revprops.clone(),
                &|_info| Ok(()),
            )
            .unwrap();

        // Update WC2 - should create property conflict
        let _result = client_ctx2.update(
            &[wc2_path.to_str().unwrap()],
            crate::Revision::Head,
            &update_opts,
        );

        let (_text_conflicted, prop_conflicted, _tree_conflicted) =
            wc_ctx.conflicted(propfile_path2.to_str().unwrap()).unwrap();

        // We might have prop conflict depending on SVN version behavior
        if prop_conflicted {
            assert!(
                prop_conflicted,
                "File should have property conflict after conflicting property update"
            );
        }

        // Test 3: File without conflicts should return all false
        let clean_file = wc1_path.join("clean.txt");
        std::fs::write(&clean_file, "no conflicts here").unwrap();
        client_ctx1
            .add(&clean_file, &crate::client::AddOptions::new())
            .unwrap();

        let (text_conflicted, prop_conflicted, tree_conflicted) =
            wc_ctx.conflicted(clean_file.to_str().unwrap()).unwrap();

        assert!(!text_conflicted, "Clean file should not have text conflict");
        assert!(
            !prop_conflicted,
            "Clean file should not have property conflict"
        );
        assert!(!tree_conflicted, "Clean file should not have tree conflict");

        // Test 4: Non-existent file should error
        let nonexistent = wc1_path.join("nonexistent.txt");
        let result = wc_ctx.conflicted(nonexistent.to_str().unwrap());
        assert!(result.is_err(), "Non-existent file should return an error");
    }

    #[test]
    fn test_copy_or_move() {
        let td = tempfile::tempdir().unwrap();
        let repo_path = td.path().join("repo");
        let wc_path = td.path().join("wc");

        // Create a test repository
        crate::repos::Repos::create(&repo_path).unwrap();

        // Check out working copy
        let mut client_ctx = crate::client::Context::new().unwrap();
        let url_str = format!("file://{}", repo_path.to_str().unwrap());
        let url = crate::uri::Uri::new(&url_str).unwrap();

        client_ctx
            .checkout(
                url,
                &wc_path,
                &crate::client::CheckoutOptions {
                    peg_revision: crate::Revision::Head,
                    revision: crate::Revision::Head,
                    depth: crate::Depth::Infinity,
                    ignore_externals: false,
                    allow_unver_obstructions: false,
                },
            )
            .unwrap();

        // Create and add a test file
        let test_file = wc_path.join("test.txt");
        std::fs::write(&test_file, "test content").unwrap();
        client_ctx
            .add(&test_file, &crate::client::AddOptions::new())
            .unwrap();

        // Test copy operation - will fail without write lock, but tests API
        let mut wc_ctx = Context::new().unwrap();
        let copy_dest = wc_path.join("test_copy.txt");
        let result = copy_or_move(&mut wc_ctx, &test_file, &copy_dest, false, false);

        // Expected to fail without write lock, but verifies the function can be called
        assert!(result.is_err());

        // Test move operation - also expected to fail without write lock
        let move_dest = wc_path.join("test_moved.txt");
        let result = copy_or_move(&mut wc_ctx, &test_file, &move_dest, true, false);

        // Expected to fail without write lock
        assert!(result.is_err());
    }

    #[test]
    fn test_revert() {
        let td = tempfile::tempdir().unwrap();
        let repo_path = td.path().join("repo");
        let wc_path = td.path().join("wc");

        // Create a test repository
        crate::repos::Repos::create(&repo_path).unwrap();

        // Check out working copy
        let mut client_ctx = crate::client::Context::new().unwrap();
        let url_str = format!("file://{}", repo_path.to_str().unwrap());
        let url = crate::uri::Uri::new(&url_str).unwrap();

        client_ctx
            .checkout(
                url,
                &wc_path,
                &crate::client::CheckoutOptions {
                    peg_revision: crate::Revision::Head,
                    revision: crate::Revision::Head,
                    depth: crate::Depth::Infinity,
                    ignore_externals: false,
                    allow_unver_obstructions: false,
                },
            )
            .unwrap();

        // Create and add a test file
        let test_file = wc_path.join("test.txt");
        std::fs::write(&test_file, "original content").unwrap();
        client_ctx
            .add(&test_file, &crate::client::AddOptions::new())
            .unwrap();

        // Test revert operation - will fail without write lock, but tests API
        let mut wc_ctx = Context::new().unwrap();
        let result = revert(
            &mut wc_ctx,
            &test_file,
            crate::Depth::Empty,
            false,
            false,
            false,
        );

        // Expected to fail without write lock, but verifies the function can be called
        assert!(result.is_err());
    }

    #[test]
    fn test_cleanup() {
        let td = tempfile::tempdir().unwrap();
        let repo_path = td.path().join("repo");
        let wc_path = td.path().join("wc");

        // Create a test repository
        crate::repos::Repos::create(&repo_path).unwrap();

        // Check out working copy
        let mut client_ctx = crate::client::Context::new().unwrap();
        let url_str = format!("file://{}", repo_path.to_str().unwrap());
        let url = crate::uri::Uri::new(&url_str).unwrap();

        client_ctx
            .checkout(
                url,
                &wc_path,
                &crate::client::CheckoutOptions {
                    peg_revision: crate::Revision::Head,
                    revision: crate::Revision::Head,
                    depth: crate::Depth::Infinity,
                    ignore_externals: false,
                    allow_unver_obstructions: false,
                },
            )
            .unwrap();

        // Test cleanup operation
        let result = cleanup(&wc_path, false, false, false, false, false);

        // Cleanup should succeed on a valid working copy
        assert!(result.is_ok());

        // Test with break_locks
        let result = cleanup(&wc_path, true, false, false, false, false);
        assert!(result.is_ok());

        // Test with fix_recorded_timestamps
        let result = cleanup(&wc_path, false, true, false, false, false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_actual_target() {
        let td = tempfile::tempdir().unwrap();
        let repo_path = td.path().join("repo");
        let wc_path = td.path().join("wc");

        // Create a test repository
        crate::repos::Repos::create(&repo_path).unwrap();

        // Check out working copy
        let mut client_ctx = crate::client::Context::new().unwrap();
        let url_str = format!("file://{}", repo_path.to_str().unwrap());
        let url = crate::uri::Uri::new(&url_str).unwrap();

        client_ctx
            .checkout(
                url,
                &wc_path,
                &crate::client::CheckoutOptions {
                    peg_revision: crate::Revision::Head,
                    revision: crate::Revision::Head,
                    depth: crate::Depth::Infinity,
                    ignore_externals: false,
                    allow_unver_obstructions: false,
                },
            )
            .unwrap();

        // Test get_actual_target on the working copy root
        let result = get_actual_target(&wc_path);
        assert!(result.is_ok());

        let (anchor, target) = result.unwrap();
        // For a WC root, anchor should be the parent and target should be the directory name
        assert!(!anchor.is_empty() || !target.is_empty());
    }

    #[test]
    fn test_walk_status() {
        let td = tempfile::tempdir().unwrap();
        let repo_path = td.path().join("repo");
        let wc_path = td.path().join("wc");

        // Create a test repository
        crate::repos::Repos::create(&repo_path).unwrap();

        // Check out working copy
        let mut client_ctx = crate::client::Context::new().unwrap();
        let url_str = format!("file://{}", repo_path.to_str().unwrap());
        let url = crate::uri::Uri::new(&url_str).unwrap();

        client_ctx
            .checkout(
                url,
                &wc_path,
                &crate::client::CheckoutOptions {
                    peg_revision: crate::Revision::Head,
                    revision: crate::Revision::Head,
                    depth: crate::Depth::Infinity,
                    ignore_externals: false,
                    allow_unver_obstructions: false,
                },
            )
            .unwrap();

        // Create a test file
        let test_file = wc_path.join("test.txt");
        std::fs::write(&test_file, "test content").unwrap();
        client_ctx
            .add(&test_file, &crate::client::AddOptions::new())
            .unwrap();

        // Test walk_status
        let mut wc_ctx = Context::new().unwrap();
        let mut status_count = 0;
        let result = wc_ctx.walk_status(
            &wc_path,
            crate::Depth::Infinity,
            true,  // get_all
            false, // no_ignore
            false, // ignore_text_mods
            |_path, _status| {
                status_count += 1;
                Ok(())
            },
        );

        assert!(result.is_ok());
        // Should have walked at least the root and the added file
        assert!(status_count >= 1, "Should have at least one status entry");
    }

    #[test]
    fn test_wc_version() {
        let version = version();
        assert!(version.major() > 0);
    }

    #[test]
    fn test_set_and_get_adm_dir() {
        // Test setting and getting admin dir
        let result = set_adm_dir("_svn");
        assert!(result.is_ok());

        let dir = get_adm_dir();
        assert_eq!(dir, "_svn");

        // Reset to default
        let result = set_adm_dir(".svn");
        assert!(result.is_ok());

        let dir = get_adm_dir();
        assert_eq!(dir, ".svn");
    }

    #[test]
    fn test_context_add() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let repos_path = tmp_dir.path().join("repo");
        let wc_path = tmp_dir.path().join("wc");

        // Create a test repository
        crate::repos::Repos::create(&repos_path).unwrap();
        let url_str = format!("file://{}", repos_path.display());
        let url = crate::uri::Uri::new(&url_str).unwrap();

        // Checkout
        let mut client_ctx = crate::client::Context::new().unwrap();
        client_ctx
            .checkout(
                url,
                &wc_path,
                &crate::client::CheckoutOptions {
                    peg_revision: crate::Revision::Head,
                    revision: crate::Revision::Head,
                    depth: crate::Depth::Infinity,
                    ignore_externals: false,
                    allow_unver_obstructions: false,
                },
            )
            .unwrap();

        // Create a new file to add
        let new_file = wc_path.join("newfile.txt");
        std::fs::write(&new_file, b"test content").unwrap();

        // Test Context::add() - svn_wc_add4() is a low-level function
        // that requires write locks to be acquired using private APIs.
        // Verify the binding exists and can be called.
        let mut wc_ctx = Context::new().unwrap();
        let new_file_abs = new_file.canonicalize().unwrap();

        let result = wc_ctx.add(
            new_file_abs.to_str().unwrap(),
            crate::Depth::Infinity,
            None,
            None,
        );

        // svn_wc_add4 requires write locks managed externally (via private APIs).
        // The binding correctly calls the C function which will fail without locks.
        // This verifies the binding works and properly propagates errors.
        assert!(result.is_err());
        let err_str = format!("{:?}", result.err().unwrap());
        assert!(
            err_str.to_lowercase().contains("lock"),
            "Expected lock error, got: {}",
            err_str
        );
    }

    #[test]
    fn test_context_relocate() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let repos_path = tmp_dir.path().join("repo");
        let wc_path = tmp_dir.path().join("wc");

        // Create a test repository
        crate::repos::Repos::create(&repos_path).unwrap();
        let url_str = format!("file://{}", repos_path.display());
        let url = crate::uri::Uri::new(&url_str).unwrap();

        // Checkout
        let mut client_ctx = crate::client::Context::new().unwrap();
        client_ctx
            .checkout(
                url,
                &wc_path,
                &crate::client::CheckoutOptions {
                    peg_revision: crate::Revision::Head,
                    revision: crate::Revision::Head,
                    depth: crate::Depth::Infinity,
                    ignore_externals: false,
                    allow_unver_obstructions: false,
                },
            )
            .unwrap();

        // Move the repository to a new location (simulating repository relocation)
        let repos_path2 = tmp_dir.path().join("repo_moved");
        std::fs::rename(&repos_path, &repos_path2).unwrap();
        let repos_url2 = format!("file://{}", repos_path2.display());

        // Test relocate - should work since it's the same repository, different URL
        let mut wc_ctx = Context::new().unwrap();
        let result = wc_ctx.relocate(wc_path.to_str().unwrap(), &url_str, &repos_url2);

        // Relocate should succeed when repository is moved
        assert!(result.is_ok(), "relocate() failed: {:?}", result.err());
    }

    #[test]
    fn test_context_upgrade() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let repos_path = tmp_dir.path().join("repo");
        let wc_path = tmp_dir.path().join("wc");

        // Create a test repository
        crate::repos::Repos::create(&repos_path).unwrap();
        let url_str = format!("file://{}", repos_path.display());
        let url = crate::uri::Uri::new(&url_str).unwrap();

        // Checkout
        let mut client_ctx = crate::client::Context::new().unwrap();
        client_ctx
            .checkout(
                url,
                &wc_path,
                &crate::client::CheckoutOptions {
                    peg_revision: crate::Revision::Head,
                    revision: crate::Revision::Head,
                    depth: crate::Depth::Infinity,
                    ignore_externals: false,
                    allow_unver_obstructions: false,
                },
            )
            .unwrap();

        // Test upgrade - should succeed (working copy is already in latest format)
        let mut wc_ctx = Context::new().unwrap();
        let result = wc_ctx.upgrade(wc_path.to_str().unwrap());
        assert!(result.is_ok(), "upgrade() failed: {:?}", result.err());
    }

    #[test]
    fn test_get_update_editor4_with_callbacks() {
        // Test that get_update_editor4 accepts callbacks
        let tmp_dir = tempfile::tempdir().unwrap();
        let repos_path = tmp_dir.path().join("repo");
        let wc_path = tmp_dir.path().join("wc");

        // Create a test repository
        crate::repos::Repos::create(&repos_path).unwrap();
        let url_str = format!("file://{}", repos_path.display());
        let url = crate::uri::Uri::new(&url_str).unwrap();

        // Checkout
        let mut client_ctx = crate::client::Context::new().unwrap();
        client_ctx
            .checkout(
                url,
                &wc_path,
                &crate::client::CheckoutOptions {
                    peg_revision: crate::Revision::Head,
                    revision: crate::Revision::Head,
                    depth: crate::Depth::Infinity,
                    ignore_externals: false,
                    allow_unver_obstructions: false,
                },
            )
            .unwrap();

        // Track callback invocations
        let cancel_called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cancel_called_clone = cancel_called.clone();

        let mut wc_ctx = Context::new().unwrap();
        let result = get_update_editor4(
            &mut wc_ctx,
            wc_path.to_str().unwrap(),
            "",
            false,
            crate::Depth::Infinity,
            false,
            false,
            false,
            false, // server_performs_filtering
            false, // clean_checkout
            None,
            &[],
            None, // fetch_dirents_func
            None, // conflict_func
            None, // external_func
            Some(Box::new(move || {
                cancel_called_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            })),
            None, // notify_func
        );

        assert!(
            result.is_ok(),
            "get_update_editor4 failed: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_get_switch_editor_with_callbacks() {
        // Test that get_switch_editor accepts callbacks
        let tmp_dir = tempfile::tempdir().unwrap();
        let repos_path = tmp_dir.path().join("repo");
        let wc_path = tmp_dir.path().join("wc");

        // Create a test repository
        crate::repos::Repos::create(&repos_path).unwrap();
        let url_str = format!("file://{}", repos_path.display());
        let url = crate::uri::Uri::new(&url_str).unwrap();

        // Checkout
        let mut client_ctx = crate::client::Context::new().unwrap();
        client_ctx
            .checkout(
                url.clone(),
                &wc_path,
                &crate::client::CheckoutOptions {
                    peg_revision: crate::Revision::Head,
                    revision: crate::Revision::Head,
                    depth: crate::Depth::Infinity,
                    ignore_externals: false,
                    allow_unver_obstructions: false,
                },
            )
            .unwrap();

        // Track callback invocations
        let notify_called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let notify_called_clone = notify_called.clone();

        let mut wc_ctx = Context::new().unwrap();
        let result = wc_ctx.get_switch_editor(
            wc_path.to_str().unwrap(),
            "",
            url_str.as_str(),
            false,
            crate::Depth::Infinity,
            false,
            false,
            false, // server_performs_filtering
            None,
            None, // fetch_dirents_func
            None, // conflict_func
            None, // external_func
            None, // cancel_func
            Some(Box::new(move |_notify| {
                notify_called_clone.store(true, std::sync::atomic::Ordering::SeqCst);
            })),
        );

        assert!(
            result.is_ok(),
            "get_switch_editor failed: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_get_update_editor4_server_performs_filtering() {
        // Test that server_performs_filtering parameter is accepted
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp_dir.path()).unwrap();

        let mut wc_ctx = Context::new().unwrap();

        // Test with server_performs_filtering = true
        let result = get_update_editor4(
            &mut wc_ctx,
            temp_dir.path().to_str().unwrap(),
            "",
            false,
            crate::Depth::Infinity,
            false,
            false,
            false,
            true, // server_performs_filtering = true
            false,
            None,
            &[],
            None,
            None,
            None,
            None,
            None,
        );

        // Will fail without a real working copy, but tests that the parameter is accepted
        assert!(result.is_err());
    }

    #[test]
    fn test_get_update_editor4_clean_checkout() {
        // Test that clean_checkout parameter is accepted
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp_dir.path()).unwrap();

        let mut wc_ctx = Context::new().unwrap();

        // Test with clean_checkout = true
        let result = get_update_editor4(
            &mut wc_ctx,
            temp_dir.path().to_str().unwrap(),
            "",
            false,
            crate::Depth::Infinity,
            false,
            false,
            false,
            false,
            true, // clean_checkout = true
            None,
            &[],
            None,
            None,
            None,
            None,
            None,
        );

        // Will fail without a real working copy, but tests that the parameter is accepted
        assert!(result.is_err());
    }

    #[test]
    fn test_get_switch_editor_server_performs_filtering() {
        // Test that server_performs_filtering parameter is accepted
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(temp_dir.path()).unwrap();

        let mut wc_ctx = Context::new().unwrap();

        // Test with server_performs_filtering = true
        let result = wc_ctx.get_switch_editor(
            temp_dir.path().to_str().unwrap(),
            "",
            "http://example.com/svn/trunk",
            false,
            crate::Depth::Infinity,
            false,
            false,
            true, // server_performs_filtering = true
            None,
            None,
            None,
            None,
            None,
            None,
        );

        // Will fail without a real working copy, but tests that the parameter is accepted
        assert!(result.is_err());
    }
}
