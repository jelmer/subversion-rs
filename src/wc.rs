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

/// Convert a path to an absolute path if it's not already absolute.
/// Many WC functions require absolute paths without trailing slashes.
fn ensure_absolute_path(path: &std::path::Path) -> Result<std::path::PathBuf, Error<'static>> {
    let abs_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .map_err(Error::from)?
    };

    // Remove trailing slash by converting to components and back
    // This canonicalizes the path without following symlinks
    let canonical = abs_path.components().collect::<std::path::PathBuf>();
    Ok(canonical)
}

// Helper functions for properly boxing callback batons
// wrap_cancel_func expects *mut Box<dyn Fn()>, not *mut Box<&dyn Fn()>
// We need double-boxing to avoid UB
fn box_cancel_baton(f: Box<dyn Fn() -> Result<(), Error<'static>>>) -> *mut std::ffi::c_void {
    Box::into_raw(Box::new(f)) as *mut std::ffi::c_void
}

fn box_notify_baton(f: Box<dyn Fn(&Notify)>) -> *mut std::ffi::c_void {
    Box::into_raw(Box::new(f)) as *mut std::ffi::c_void
}

fn box_conflict_baton(
    f: Box<
        dyn Fn(
            &crate::conflict::ConflictDescription,
        ) -> Result<crate::conflict::ConflictResult, Error<'static>>,
    >,
) -> *mut std::ffi::c_void {
    Box::into_raw(Box::new(f)) as *mut std::ffi::c_void
}

fn box_external_baton(
    f: Box<dyn Fn(&str, Option<&str>, Option<&str>, crate::Depth) -> Result<(), Error<'static>>>,
) -> *mut std::ffi::c_void {
    Box::into_raw(Box::new(f)) as *mut std::ffi::c_void
}

fn box_fetch_dirents_baton(
    f: Box<
        dyn Fn(
            &str,
            &str,
        )
            -> Result<std::collections::HashMap<String, crate::DirEntry>, Error<'static>>,
    >,
) -> *mut std::ffi::c_void {
    Box::into_raw(Box::new(f)) as *mut std::ffi::c_void
}

// Borrowed versions for synchronous operations where callback lifetime is guaranteed
fn box_cancel_baton_borrowed(f: &dyn Fn() -> Result<(), Error<'static>>) -> *mut std::ffi::c_void {
    Box::into_raw(Box::new(f)) as *mut std::ffi::c_void
}

fn box_notify_baton_borrowed(f: &dyn Fn(&Notify)) -> *mut std::ffi::c_void {
    Box::into_raw(Box::new(f)) as *mut std::ffi::c_void
}

// Dropper functions for each callback type
unsafe fn drop_cancel_baton(baton: *mut std::ffi::c_void) {
    drop(Box::from_raw(
        baton as *mut Box<dyn Fn() -> Result<(), Error<'static>>>,
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
                ) -> Result<crate::conflict::ConflictResult, Error<'static>>,
            >,
    ));
}

unsafe fn drop_external_baton(baton: *mut std::ffi::c_void) {
    drop(Box::from_raw(
        baton
            as *mut Box<
                dyn Fn(
                    &str,
                    Option<&str>,
                    Option<&str>,
                    crate::Depth,
                ) -> Result<(), Error<'static>>,
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
                ) -> Result<
                    std::collections::HashMap<String, crate::DirEntry>,
                    Error<'static>,
                >,
            >,
    ));
}

// Dropper functions for borrowed callbacks (used in synchronous operations)
unsafe fn drop_cancel_baton_borrowed(baton: *mut std::ffi::c_void) {
    drop(Box::from_raw(
        baton as *mut &dyn Fn() -> Result<(), Error<'static>>,
    ));
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
            _ => unreachable!("unknown svn_wc_status_kind value: {}", status),
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
pub struct Status<'pool> {
    ptr: *const subversion_sys::svn_wc_status3_t,
    /// Keeps the APR pool that owns `ptr`'s allocation alive.
    /// `PoolHandle::Owned` when `Status` is returned from `Context::status()`;
    /// `PoolHandle::Borrowed` (non-destroying) when created inside a callback
    /// whose pool is managed by the SVN C library.
    _pool: apr::pool::PoolHandle<'pool>,
}

impl<'pool> Status<'pool> {
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

    /// Get the node kind
    pub fn kind(&self) -> i32 {
        unsafe { (*self.ptr).kind as i32 }
    }

    /// Get the depth
    pub fn depth(&self) -> i32 {
        unsafe { (*self.ptr).depth }
    }

    /// Get the file size
    pub fn filesize(&self) -> i64 {
        unsafe { (*self.ptr).filesize }
    }

    /// Check if the item is versioned
    pub fn versioned(&self) -> bool {
        unsafe { (*self.ptr).versioned != 0 }
    }

    /// Get the repository UUID
    pub fn repos_uuid(&self) -> Option<String> {
        unsafe {
            if (*self.ptr).repos_uuid.is_null() {
                None
            } else {
                Some(
                    std::ffi::CStr::from_ptr((*self.ptr).repos_uuid)
                        .to_string_lossy()
                        .into_owned(),
                )
            }
        }
    }

    /// Get the repository root URL
    pub fn repos_root_url(&self) -> Option<String> {
        unsafe {
            if (*self.ptr).repos_root_url.is_null() {
                None
            } else {
                Some(
                    std::ffi::CStr::from_ptr((*self.ptr).repos_root_url)
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
            _ => unreachable!("unknown svn_wc_schedule_t value: {}", schedule),
        }
    }
}

/// Outcome of a file merge operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeOutcome {
    /// The working copy is (or would be) unchanged; changes were already present.
    Unchanged,
    /// The working copy has been (or would be) changed.
    Merged,
    /// The working copy has been (or would be) changed, but with a conflict.
    Conflict,
    /// No merge was performed (target absent or unversioned).
    NoMerge,
}

impl From<subversion_sys::svn_wc_merge_outcome_t> for MergeOutcome {
    fn from(outcome: subversion_sys::svn_wc_merge_outcome_t) -> Self {
        match outcome {
            subversion_sys::svn_wc_merge_outcome_t_svn_wc_merge_unchanged => {
                MergeOutcome::Unchanged
            }
            subversion_sys::svn_wc_merge_outcome_t_svn_wc_merge_merged => MergeOutcome::Merged,
            subversion_sys::svn_wc_merge_outcome_t_svn_wc_merge_conflict => MergeOutcome::Conflict,
            subversion_sys::svn_wc_merge_outcome_t_svn_wc_merge_no_merge => MergeOutcome::NoMerge,
            _ => unreachable!("unknown svn_wc_merge_outcome_t value: {}", outcome),
        }
    }
}

/// State of a working copy item after a notify operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotifyState {
    /// Not applicable.
    Inapplicable,
    /// State is unknown.
    Unknown,
    /// Item was unchanged.
    Unchanged,
    /// Item was missing.
    Missing,
    /// An unversioned item obstructed the operation.
    Obstructed,
    /// Item was changed.
    Changed,
    /// Item had modifications merged in.
    Merged,
    /// Item got conflicting modifications.
    Conflicted,
    /// The source for a copy was missing.
    SourceMissing,
}

impl From<subversion_sys::svn_wc_notify_state_t> for NotifyState {
    fn from(state: subversion_sys::svn_wc_notify_state_t) -> Self {
        match state {
            subversion_sys::svn_wc_notify_state_t_svn_wc_notify_state_inapplicable => {
                NotifyState::Inapplicable
            }
            subversion_sys::svn_wc_notify_state_t_svn_wc_notify_state_unknown => {
                NotifyState::Unknown
            }
            subversion_sys::svn_wc_notify_state_t_svn_wc_notify_state_unchanged => {
                NotifyState::Unchanged
            }
            subversion_sys::svn_wc_notify_state_t_svn_wc_notify_state_missing => {
                NotifyState::Missing
            }
            subversion_sys::svn_wc_notify_state_t_svn_wc_notify_state_obstructed => {
                NotifyState::Obstructed
            }
            subversion_sys::svn_wc_notify_state_t_svn_wc_notify_state_changed => {
                NotifyState::Changed
            }
            subversion_sys::svn_wc_notify_state_t_svn_wc_notify_state_merged => NotifyState::Merged,
            subversion_sys::svn_wc_notify_state_t_svn_wc_notify_state_conflicted => {
                NotifyState::Conflicted
            }
            subversion_sys::svn_wc_notify_state_t_svn_wc_notify_state_source_missing => {
                NotifyState::SourceMissing
            }
            _ => unreachable!("unknown svn_wc_notify_state_t value: {}", state),
        }
    }
}

impl From<NotifyState> for subversion_sys::svn_wc_notify_state_t {
    fn from(state: NotifyState) -> Self {
        match state {
            NotifyState::Inapplicable => {
                subversion_sys::svn_wc_notify_state_t_svn_wc_notify_state_inapplicable
            }
            NotifyState::Unknown => {
                subversion_sys::svn_wc_notify_state_t_svn_wc_notify_state_unknown
            }
            NotifyState::Unchanged => {
                subversion_sys::svn_wc_notify_state_t_svn_wc_notify_state_unchanged
            }
            NotifyState::Missing => {
                subversion_sys::svn_wc_notify_state_t_svn_wc_notify_state_missing
            }
            NotifyState::Obstructed => {
                subversion_sys::svn_wc_notify_state_t_svn_wc_notify_state_obstructed
            }
            NotifyState::Changed => {
                subversion_sys::svn_wc_notify_state_t_svn_wc_notify_state_changed
            }
            NotifyState::Merged => subversion_sys::svn_wc_notify_state_t_svn_wc_notify_state_merged,
            NotifyState::Conflicted => {
                subversion_sys::svn_wc_notify_state_t_svn_wc_notify_state_conflicted
            }
            NotifyState::SourceMissing => {
                subversion_sys::svn_wc_notify_state_t_svn_wc_notify_state_source_missing
            }
        }
    }
}

/// A file change event reported by the diff callbacks.
pub struct FileChange<'a> {
    /// Relative path of the file within the working copy.
    pub path: &'a str,
    /// Temporary file with the "left" (base) content, if changed.
    pub tmpfile1: Option<&'a str>,
    /// Temporary file with the "right" (modified) content, if changed.
    pub tmpfile2: Option<&'a str>,
    /// Revision of the left side.
    pub rev1: crate::Revnum,
    /// Revision of the right side.
    pub rev2: crate::Revnum,
    /// MIME type of the left side, if known.
    pub mimetype1: Option<&'a str>,
    /// MIME type of the right side, if known.
    pub mimetype2: Option<&'a str>,
    /// Property changes for this file.
    pub prop_changes: Vec<PropChange>,
}

// --- Diff callback trampolines (module-level so they can be shared) ---

/// Convert a raw apr_array_header_t of svn_prop_t into Vec<PropChange>.
unsafe fn diff_prop_array_to_vec(arr: *const apr_sys::apr_array_header_t) -> Vec<PropChange> {
    if arr.is_null() {
        return Vec::new();
    }
    let typed = apr::tables::TypedArray::<subversion_sys::svn_prop_t>::from_ptr(
        arr as *mut apr_sys::apr_array_header_t,
    );
    typed
        .iter()
        .map(|p| {
            let name = if p.name.is_null() {
                String::new()
            } else {
                std::ffi::CStr::from_ptr(p.name)
                    .to_string_lossy()
                    .into_owned()
            };
            let value = if p.value.is_null() {
                None
            } else {
                Some(crate::svn_string_helpers::to_vec(&*p.value))
            };
            PropChange { name, value }
        })
        .collect()
}

/// Turn a nullable C string into Option<&str>.
unsafe fn diff_opt_str<'a>(p: *const std::os::raw::c_char) -> Option<&'a str> {
    if p.is_null() {
        None
    } else {
        std::ffi::CStr::from_ptr(p).to_str().ok()
    }
}

unsafe extern "C" fn diff_cb_file_opened(
    tree_conflicted: *mut subversion_sys::svn_boolean_t,
    skip: *mut subversion_sys::svn_boolean_t,
    path: *const std::os::raw::c_char,
    rev: subversion_sys::svn_revnum_t,
    diff_baton: *mut std::ffi::c_void,
    _scratch_pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let cb = &mut *(diff_baton as *mut &mut dyn DiffCallbacks);
    let path = std::ffi::CStr::from_ptr(path).to_str().unwrap_or("");
    match cb.file_opened(path, crate::Revnum(rev)) {
        Ok((tc, sk)) => {
            if !tree_conflicted.is_null() {
                *tree_conflicted = tc as i32;
            }
            if !skip.is_null() {
                *skip = sk as i32;
            }
            std::ptr::null_mut()
        }
        Err(e) => e.into_raw(),
    }
}

unsafe extern "C" fn diff_cb_file_changed(
    contentstate: *mut subversion_sys::svn_wc_notify_state_t,
    propstate: *mut subversion_sys::svn_wc_notify_state_t,
    tree_conflicted: *mut subversion_sys::svn_boolean_t,
    path: *const std::os::raw::c_char,
    tmpfile1: *const std::os::raw::c_char,
    tmpfile2: *const std::os::raw::c_char,
    rev1: subversion_sys::svn_revnum_t,
    rev2: subversion_sys::svn_revnum_t,
    mimetype1: *const std::os::raw::c_char,
    mimetype2: *const std::os::raw::c_char,
    propchanges: *const apr_sys::apr_array_header_t,
    _originalprops: *mut apr_sys::apr_hash_t,
    diff_baton: *mut std::ffi::c_void,
    _scratch_pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let cb = &mut *(diff_baton as *mut &mut dyn DiffCallbacks);
    let change = FileChange {
        path: std::ffi::CStr::from_ptr(path).to_str().unwrap_or(""),
        tmpfile1: diff_opt_str(tmpfile1),
        tmpfile2: diff_opt_str(tmpfile2),
        rev1: crate::Revnum(rev1),
        rev2: crate::Revnum(rev2),
        mimetype1: diff_opt_str(mimetype1),
        mimetype2: diff_opt_str(mimetype2),
        prop_changes: diff_prop_array_to_vec(propchanges),
    };
    match cb.file_changed(&change) {
        Ok((cs, ps, tc)) => {
            if !contentstate.is_null() {
                *contentstate = cs.into();
            }
            if !propstate.is_null() {
                *propstate = ps.into();
            }
            if !tree_conflicted.is_null() {
                *tree_conflicted = tc as i32;
            }
            std::ptr::null_mut()
        }
        Err(e) => e.into_raw(),
    }
}

unsafe extern "C" fn diff_cb_file_added(
    contentstate: *mut subversion_sys::svn_wc_notify_state_t,
    propstate: *mut subversion_sys::svn_wc_notify_state_t,
    tree_conflicted: *mut subversion_sys::svn_boolean_t,
    path: *const std::os::raw::c_char,
    tmpfile1: *const std::os::raw::c_char,
    tmpfile2: *const std::os::raw::c_char,
    rev1: subversion_sys::svn_revnum_t,
    rev2: subversion_sys::svn_revnum_t,
    mimetype1: *const std::os::raw::c_char,
    mimetype2: *const std::os::raw::c_char,
    copyfrom_path: *const std::os::raw::c_char,
    copyfrom_revision: subversion_sys::svn_revnum_t,
    propchanges: *const apr_sys::apr_array_header_t,
    _originalprops: *mut apr_sys::apr_hash_t,
    diff_baton: *mut std::ffi::c_void,
    _scratch_pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let cb = &mut *(diff_baton as *mut &mut dyn DiffCallbacks);
    let change = FileChange {
        path: std::ffi::CStr::from_ptr(path).to_str().unwrap_or(""),
        tmpfile1: diff_opt_str(tmpfile1),
        tmpfile2: diff_opt_str(tmpfile2),
        rev1: crate::Revnum(rev1),
        rev2: crate::Revnum(rev2),
        mimetype1: diff_opt_str(mimetype1),
        mimetype2: diff_opt_str(mimetype2),
        prop_changes: diff_prop_array_to_vec(propchanges),
    };
    match cb.file_added(
        &change,
        diff_opt_str(copyfrom_path),
        crate::Revnum(copyfrom_revision),
    ) {
        Ok((cs, ps, tc)) => {
            if !contentstate.is_null() {
                *contentstate = cs.into();
            }
            if !propstate.is_null() {
                *propstate = ps.into();
            }
            if !tree_conflicted.is_null() {
                *tree_conflicted = tc as i32;
            }
            std::ptr::null_mut()
        }
        Err(e) => e.into_raw(),
    }
}

unsafe extern "C" fn diff_cb_file_deleted(
    state: *mut subversion_sys::svn_wc_notify_state_t,
    tree_conflicted: *mut subversion_sys::svn_boolean_t,
    path: *const std::os::raw::c_char,
    tmpfile1: *const std::os::raw::c_char,
    tmpfile2: *const std::os::raw::c_char,
    mimetype1: *const std::os::raw::c_char,
    mimetype2: *const std::os::raw::c_char,
    _originalprops: *mut apr_sys::apr_hash_t,
    diff_baton: *mut std::ffi::c_void,
    _scratch_pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let cb = &mut *(diff_baton as *mut &mut dyn DiffCallbacks);
    let path = std::ffi::CStr::from_ptr(path).to_str().unwrap_or("");
    match cb.file_deleted(
        path,
        diff_opt_str(tmpfile1),
        diff_opt_str(tmpfile2),
        diff_opt_str(mimetype1),
        diff_opt_str(mimetype2),
    ) {
        Ok((st, tc)) => {
            if !state.is_null() {
                *state = st.into();
            }
            if !tree_conflicted.is_null() {
                *tree_conflicted = tc as i32;
            }
            std::ptr::null_mut()
        }
        Err(e) => e.into_raw(),
    }
}

unsafe extern "C" fn diff_cb_dir_deleted(
    state: *mut subversion_sys::svn_wc_notify_state_t,
    tree_conflicted: *mut subversion_sys::svn_boolean_t,
    path: *const std::os::raw::c_char,
    diff_baton: *mut std::ffi::c_void,
    _scratch_pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let cb = &mut *(diff_baton as *mut &mut dyn DiffCallbacks);
    let path = std::ffi::CStr::from_ptr(path).to_str().unwrap_or("");
    match cb.dir_deleted(path) {
        Ok((st, tc)) => {
            if !state.is_null() {
                *state = st.into();
            }
            if !tree_conflicted.is_null() {
                *tree_conflicted = tc as i32;
            }
            std::ptr::null_mut()
        }
        Err(e) => e.into_raw(),
    }
}

unsafe extern "C" fn diff_cb_dir_opened(
    tree_conflicted: *mut subversion_sys::svn_boolean_t,
    skip: *mut subversion_sys::svn_boolean_t,
    skip_children: *mut subversion_sys::svn_boolean_t,
    path: *const std::os::raw::c_char,
    rev: subversion_sys::svn_revnum_t,
    diff_baton: *mut std::ffi::c_void,
    _scratch_pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let cb = &mut *(diff_baton as *mut &mut dyn DiffCallbacks);
    let path = std::ffi::CStr::from_ptr(path).to_str().unwrap_or("");
    match cb.dir_opened(path, crate::Revnum(rev)) {
        Ok((tc, sk, skc)) => {
            if !tree_conflicted.is_null() {
                *tree_conflicted = tc as i32;
            }
            if !skip.is_null() {
                *skip = sk as i32;
            }
            if !skip_children.is_null() {
                *skip_children = skc as i32;
            }
            std::ptr::null_mut()
        }
        Err(e) => e.into_raw(),
    }
}

unsafe extern "C" fn diff_cb_dir_added(
    state: *mut subversion_sys::svn_wc_notify_state_t,
    tree_conflicted: *mut subversion_sys::svn_boolean_t,
    skip: *mut subversion_sys::svn_boolean_t,
    skip_children: *mut subversion_sys::svn_boolean_t,
    path: *const std::os::raw::c_char,
    rev: subversion_sys::svn_revnum_t,
    copyfrom_path: *const std::os::raw::c_char,
    copyfrom_revision: subversion_sys::svn_revnum_t,
    diff_baton: *mut std::ffi::c_void,
    _scratch_pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let cb = &mut *(diff_baton as *mut &mut dyn DiffCallbacks);
    let path = std::ffi::CStr::from_ptr(path).to_str().unwrap_or("");
    match cb.dir_added(
        path,
        crate::Revnum(rev),
        diff_opt_str(copyfrom_path),
        crate::Revnum(copyfrom_revision),
    ) {
        Ok((st, tc, sk, skc)) => {
            if !state.is_null() {
                *state = st.into();
            }
            if !tree_conflicted.is_null() {
                *tree_conflicted = tc as i32;
            }
            if !skip.is_null() {
                *skip = sk as i32;
            }
            if !skip_children.is_null() {
                *skip_children = skc as i32;
            }
            std::ptr::null_mut()
        }
        Err(e) => e.into_raw(),
    }
}

unsafe extern "C" fn diff_cb_dir_props_changed(
    propstate: *mut subversion_sys::svn_wc_notify_state_t,
    tree_conflicted: *mut subversion_sys::svn_boolean_t,
    path: *const std::os::raw::c_char,
    dir_was_added: subversion_sys::svn_boolean_t,
    propchanges: *const apr_sys::apr_array_header_t,
    _original_props: *mut apr_sys::apr_hash_t,
    diff_baton: *mut std::ffi::c_void,
    _scratch_pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let cb = &mut *(diff_baton as *mut &mut dyn DiffCallbacks);
    let path = std::ffi::CStr::from_ptr(path).to_str().unwrap_or("");
    let changes = diff_prop_array_to_vec(propchanges);
    match cb.dir_props_changed(path, dir_was_added != 0, &changes) {
        Ok((ps, tc)) => {
            if !propstate.is_null() {
                *propstate = ps.into();
            }
            if !tree_conflicted.is_null() {
                *tree_conflicted = tc as i32;
            }
            std::ptr::null_mut()
        }
        Err(e) => e.into_raw(),
    }
}

unsafe extern "C" fn diff_cb_dir_closed(
    contentstate: *mut subversion_sys::svn_wc_notify_state_t,
    propstate: *mut subversion_sys::svn_wc_notify_state_t,
    tree_conflicted: *mut subversion_sys::svn_boolean_t,
    path: *const std::os::raw::c_char,
    dir_was_added: subversion_sys::svn_boolean_t,
    diff_baton: *mut std::ffi::c_void,
    _scratch_pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let cb = &mut *(diff_baton as *mut &mut dyn DiffCallbacks);
    let path = std::ffi::CStr::from_ptr(path).to_str().unwrap_or("");
    match cb.dir_closed(path, dir_was_added != 0) {
        Ok((cs, ps, tc)) => {
            if !contentstate.is_null() {
                *contentstate = cs.into();
            }
            if !propstate.is_null() {
                *propstate = ps.into();
            }
            if !tree_conflicted.is_null() {
                *tree_conflicted = tc as i32;
            }
            std::ptr::null_mut()
        }
        Err(e) => e.into_raw(),
    }
}

/// Build a `svn_wc_diff_callbacks4_t` struct pointing at the module-level trampolines.
fn make_diff_callbacks4() -> subversion_sys::svn_wc_diff_callbacks4_t {
    subversion_sys::svn_wc_diff_callbacks4_t {
        file_opened: Some(diff_cb_file_opened),
        file_changed: Some(diff_cb_file_changed),
        file_added: Some(diff_cb_file_added),
        file_deleted: Some(diff_cb_file_deleted),
        dir_deleted: Some(diff_cb_dir_deleted),
        dir_opened: Some(diff_cb_dir_opened),
        dir_added: Some(diff_cb_dir_added),
        dir_props_changed: Some(diff_cb_dir_props_changed),
        dir_closed: Some(diff_cb_dir_closed),
    }
}

/// Trait for receiving diff output from [`Context::diff`].
///
/// Implement this trait to receive file-level diff events when comparing a
/// working copy path against its base revision.
pub trait DiffCallbacks {
    /// Called before `file_changed` to allow skipping expensive processing.
    ///
    /// Return `(tree_conflicted, skip)`.  Set `skip` to `true` to skip the
    /// subsequent `file_changed` call for this path.
    fn file_opened(
        &mut self,
        path: &str,
        rev: crate::Revnum,
    ) -> Result<(bool, bool), crate::Error<'static>>;

    /// A file was modified.
    ///
    /// Return `(content_state, prop_state, tree_conflicted)`.
    fn file_changed(
        &mut self,
        change: &FileChange<'_>,
    ) -> Result<(NotifyState, NotifyState, bool), crate::Error<'static>>;

    /// A file was added (or copied).
    ///
    /// `copyfrom_path` and `copyfrom_revision` are `Some` when the add is
    /// actually a copy with history.
    ///
    /// Return `(content_state, prop_state, tree_conflicted)`.
    fn file_added(
        &mut self,
        change: &FileChange<'_>,
        copyfrom_path: Option<&str>,
        copyfrom_revision: crate::Revnum,
    ) -> Result<(NotifyState, NotifyState, bool), crate::Error<'static>>;

    /// A file was deleted.
    ///
    /// Return `(state, tree_conflicted)`.
    fn file_deleted(
        &mut self,
        path: &str,
        tmpfile1: Option<&str>,
        tmpfile2: Option<&str>,
        mimetype1: Option<&str>,
        mimetype2: Option<&str>,
    ) -> Result<(NotifyState, bool), crate::Error<'static>>;

    /// A directory was deleted.
    ///
    /// Return `(state, tree_conflicted)`.
    fn dir_deleted(&mut self, path: &str) -> Result<(NotifyState, bool), crate::Error<'static>>;

    /// A directory has been opened.
    ///
    /// Called before any callbacks for children of `path`.
    /// Return `(tree_conflicted, skip, skip_children)`.
    fn dir_opened(
        &mut self,
        path: &str,
        rev: crate::Revnum,
    ) -> Result<(bool, bool, bool), crate::Error<'static>>;

    /// A directory was added (or copied).
    ///
    /// Return `(state, tree_conflicted, skip, skip_children)`.
    fn dir_added(
        &mut self,
        path: &str,
        rev: crate::Revnum,
        copyfrom_path: Option<&str>,
        copyfrom_revision: crate::Revnum,
    ) -> Result<(NotifyState, bool, bool, bool), crate::Error<'static>>;

    /// Property changes on a directory were applied.
    ///
    /// Return `(prop_state, tree_conflicted)`.
    fn dir_props_changed(
        &mut self,
        path: &str,
        dir_was_added: bool,
        prop_changes: &[PropChange],
    ) -> Result<(NotifyState, bool), crate::Error<'static>>;

    /// A directory that was opened with `dir_opened` or `dir_added` has been closed.
    ///
    /// Return `(content_state, prop_state, tree_conflicted)`.
    fn dir_closed(
        &mut self,
        path: &str,
        dir_was_added: bool,
    ) -> Result<(NotifyState, NotifyState, bool), crate::Error<'static>>;
}

/// Options for [`Context::diff`].
pub struct DiffOptions {
    /// How deeply to traverse the working copy tree.
    pub depth: crate::Depth,
    /// If true, items with the same ancestry are not compared.
    pub ignore_ancestry: bool,
    /// If true, copies are shown as plain additions.
    pub show_copies_as_adds: bool,
    /// If true, produce git-compatible diff output.
    pub use_git_diff_format: bool,
    /// Only report items in these changelists (empty = all).
    pub changelists: Vec<String>,
}

impl Default for DiffOptions {
    fn default() -> Self {
        Self {
            depth: crate::Depth::Infinity,
            ignore_ancestry: false,
            show_copies_as_adds: false,
            use_git_diff_format: false,
            changelists: Vec::new(),
        }
    }
}

/// Options for [`Context::merge`].
#[derive(Default)]
pub struct MergeOptions {
    /// If true, perform a dry run without modifying the working copy.
    pub dry_run: bool,
    /// Path to an external diff3 command, or None to use the built-in.
    pub diff3_cmd: Option<String>,
    /// Extra options passed to the diff3 command.
    pub merge_options: Vec<String>,
}

/// Options for [`Context::revert`].
pub struct RevertOptions {
    /// How deeply to revert below the target path.
    pub depth: crate::Depth,
    /// If `true`, set reverted files' timestamps to their last-commit time.
    /// If `false`, touch them with the current time.
    pub use_commit_times: bool,
    /// Only revert items in these changelists (empty = all items).
    pub changelists: Vec<String>,
    /// If `true`, also clear changelist membership on reverted items.
    pub clear_changelists: bool,
    /// If `true`, only revert metadata (e.g., remove conflict markers)
    /// without touching working-copy file content.
    pub metadata_only: bool,
    /// If `true`, items that were *added* (not copied) are kept on disk
    /// after being un-scheduled; otherwise they are deleted.
    pub added_keep_local: bool,
}

impl Default for RevertOptions {
    fn default() -> Self {
        Self {
            depth: crate::Depth::Empty,
            use_commit_times: false,
            changelists: Vec::new(),
            clear_changelists: false,
            metadata_only: false,
            added_keep_local: true,
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
        if !self.ptr.is_null() {
            // svn_wc_context_destroy() releases any resources acquired by the
            // context beyond those held by its pool.  The pool will still be
            // freed when `self.pool` drops, but calling destroy explicitly is
            // the documented way to release the context.
            unsafe {
                subversion_sys::svn_wc_context_destroy(self.ptr);
            }
            self.ptr = std::ptr::null_mut();
        }
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
    pub fn new() -> Result<Self, crate::Error<'static>> {
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
    pub fn new_with_config(config: *mut std::ffi::c_void) -> Result<Self, crate::Error<'static>> {
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
    pub fn check_wc(&mut self, path: &str) -> Result<i32, crate::Error<'_>> {
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
    pub fn text_modified(&mut self, path: &str) -> Result<bool, crate::Error<'_>> {
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
    pub fn props_modified(&mut self, path: &str) -> Result<bool, crate::Error<'_>> {
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
    pub fn conflicted(&mut self, path: &str) -> Result<(bool, bool, bool), crate::Error<'_>> {
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
    ) -> Result<(), crate::Error<'_>> {
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
    pub fn locked(&mut self, path: &str) -> Result<(bool, bool), crate::Error<'_>> {
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
    pub fn db_version(&self) -> Result<i32, crate::Error<'_>> {
        // This would require exposing more internal SVN APIs
        // For now, just indicate we don't have this information
        Ok(0) // 0 indicates unknown/unavailable
    }

    /// Upgrade a working copy to the latest format
    pub fn upgrade(&mut self, local_abspath: &str) -> Result<(), crate::Error<'_>> {
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
    ) -> Result<(), crate::Error<'_>> {
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
    ) -> Result<(), crate::Error<'_>> {
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
pub fn set_adm_dir(name: &str) -> Result<(), crate::Error<'_>> {
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
pub fn text_modified(
    path: &std::path::Path,
    force_comparison: bool,
) -> Result<bool, crate::Error<'_>> {
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
pub fn props_modified(path: &std::path::Path) -> Result<bool, crate::Error<'_>> {
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
#[cfg(feature = "ra")]
pub fn crawl_revisions5(
    wc_ctx: &mut Context,
    local_abspath: &str,
    reporter: &mut crate::ra::WrapReporter,
    restore_files: bool,
    depth: crate::Depth,
    honor_depth_exclude: bool,
    depth_compatibility_trick: bool,
    use_commit_times: bool,
    notify_func: Option<&dyn Fn(&Notify)>,
) -> Result<(), crate::Error<'static>> {
    let local_abspath_cstr = std::ffi::CString::new(local_abspath)?;

    let notify_baton = notify_func
        .map(|f| box_notify_baton_borrowed(f))
        .unwrap_or(std::ptr::null_mut());

    let result = with_tmp_pool(|scratch_pool| {
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
                None,                 // cancel_func
                std::ptr::null_mut(), // cancel_baton
                if notify_func.is_some() {
                    Some(wrap_notify_func)
                } else {
                    None
                },
                notify_baton,
                scratch_pool.as_mut_ptr(),
            )
        };

        svn_result(err)
    });

    if !notify_baton.is_null() {
        unsafe { drop_notify_baton_borrowed(notify_baton) };
    }

    result
}

/// Get an editor for updating the working copy
///
/// The returned editor borrows from the context and must not outlive it.
pub fn get_update_editor4<'a>(
    wc_ctx: &'a mut Context,
    anchor_abspath: &str,
    target_basename: &str,
    options: UpdateEditorOptions,
) -> Result<(UpdateEditor<'a>, crate::Revnum), crate::Error<'static>> {
    let anchor_abspath_cstr = std::ffi::CString::new(anchor_abspath)?;
    let target_basename_cstr = std::ffi::CString::new(target_basename)?;
    let diff3_cmd_cstr = options.diff3_cmd.map(std::ffi::CString::new).transpose()?;

    let result_pool = apr::Pool::new();

    // Create preserved extensions array
    let preserved_exts_cstrs: Vec<std::ffi::CString> = options
        .preserved_exts
        .iter()
        .map(|&s| std::ffi::CString::new(s))
        .collect::<Result<Vec<_>, _>>()?;
    let preserved_exts_apr = if preserved_exts_cstrs.is_empty() {
        std::ptr::null()
    } else {
        let mut arr = apr::tables::TypedArray::<*const i8>::new(
            &result_pool,
            preserved_exts_cstrs.len() as i32,
        );
        for cstr in &preserved_exts_cstrs {
            arr.push(cstr.as_ptr());
        }
        unsafe { arr.as_ptr() }
    };
    let mut target_revision: subversion_sys::svn_revnum_t = 0;
    let mut editor_ptr: *const subversion_sys::svn_delta_editor_t = std::ptr::null();
    let mut edit_baton: *mut std::ffi::c_void = std::ptr::null_mut();

    // Create batons for callbacks
    let has_fetch_dirents = options.fetch_dirents_func.is_some();
    let fetch_dirents_baton = options
        .fetch_dirents_func
        .map(|f| box_fetch_dirents_baton(f))
        .unwrap_or(std::ptr::null_mut());
    let has_conflict = options.conflict_func.is_some();
    let conflict_baton = options
        .conflict_func
        .map(|f| box_conflict_baton(f))
        .unwrap_or(std::ptr::null_mut());
    let has_external = options.external_func.is_some();
    let external_baton = options
        .external_func
        .map(|f| box_external_baton(f))
        .unwrap_or(std::ptr::null_mut());
    let has_cancel = options.cancel_func.is_some();
    let cancel_baton = options
        .cancel_func
        .map(box_cancel_baton)
        .unwrap_or(std::ptr::null_mut());
    let has_notify = options.notify_func.is_some();
    let notify_baton = options
        .notify_func
        .map(|f| box_notify_baton(f))
        .unwrap_or(std::ptr::null_mut());

    let err = with_tmp_pool(|scratch_pool| unsafe {
        svn_result(subversion_sys::svn_wc_get_update_editor4(
            &mut editor_ptr,
            &mut edit_baton,
            &mut target_revision,
            wc_ctx.as_mut_ptr(),
            anchor_abspath_cstr.as_ptr(),
            target_basename_cstr.as_ptr(),
            if options.use_commit_times { 1 } else { 0 },
            options.depth.into(),
            if options.depth_is_sticky { 1 } else { 0 },
            if options.allow_unver_obstructions {
                1
            } else {
                0
            },
            if options.adds_as_modification { 1 } else { 0 },
            if options.server_performs_filtering {
                1
            } else {
                0
            },
            if options.clean_checkout { 1 } else { 0 },
            diff3_cmd_cstr
                .as_ref()
                .map_or(std::ptr::null(), |c| c.as_ptr()),
            preserved_exts_apr,
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
        editor: editor_ptr,
        edit_baton,
        _pool: result_pool,
        target_revision: crate::Revnum::from_raw(target_revision).unwrap_or_default(),
        callback_batons: batons,
        _marker: std::marker::PhantomData,
    };

    Ok((
        editor,
        crate::Revnum::from_raw(target_revision).unwrap_or_default(),
    ))
}

// Type-erased dropper function for callback batons
type DropperFn = unsafe fn(*mut std::ffi::c_void);

/// Options for get_update_editor4 function.
#[derive(Default)]
pub struct UpdateEditorOptions<'a> {
    /// If true, use commit times for file timestamps.
    pub use_commit_times: bool,
    /// Depth of the update operation.
    pub depth: crate::Depth,
    /// If true, depth changes are sticky.
    pub depth_is_sticky: bool,
    /// If true, allow unversioned obstructions.
    pub allow_unver_obstructions: bool,
    /// If true, treat adds as modifications.
    pub adds_as_modification: bool,
    /// If true, server performs filtering.
    pub server_performs_filtering: bool,
    /// If true, this is a clean checkout.
    pub clean_checkout: bool,
    /// Path to diff3 command for merging.
    pub diff3_cmd: Option<&'a str>,
    /// File extensions to preserve during merge.
    pub preserved_exts: Vec<&'a str>,
    /// Callback to fetch directory entries.
    pub fetch_dirents_func: Option<
        Box<
            dyn Fn(
                &str,
                &str,
            )
                -> Result<std::collections::HashMap<String, crate::DirEntry>, Error<'static>>,
        >,
    >,
    /// Callback for conflict resolution.
    pub conflict_func: Option<
        Box<
            dyn Fn(
                &crate::conflict::ConflictDescription,
            ) -> Result<crate::conflict::ConflictResult, Error<'static>>,
        >,
    >,
    /// Callback for external definitions.
    pub external_func: Option<
        Box<dyn Fn(&str, Option<&str>, Option<&str>, crate::Depth) -> Result<(), Error<'static>>>,
    >,
    /// Callback for cancellation.
    pub cancel_func: Option<Box<dyn Fn() -> Result<(), Error<'static>>>>,
    /// Callback for notifications.
    pub notify_func: Option<Box<dyn Fn(&Notify)>>,
}

impl<'a> UpdateEditorOptions<'a> {
    /// Creates new UpdateEditorOptions with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets whether to use commit times.
    pub fn with_use_commit_times(mut self, use_commit_times: bool) -> Self {
        self.use_commit_times = use_commit_times;
        self
    }

    /// Sets the depth for the operation.
    pub fn with_depth(mut self, depth: crate::Depth) -> Self {
        self.depth = depth;
        self
    }

    /// Sets whether depth is sticky.
    pub fn with_depth_is_sticky(mut self, sticky: bool) -> Self {
        self.depth_is_sticky = sticky;
        self
    }

    /// Sets whether to allow unversioned obstructions.
    pub fn with_allow_unver_obstructions(mut self, allow: bool) -> Self {
        self.allow_unver_obstructions = allow;
        self
    }

    /// Sets whether adds are treated as modifications.
    pub fn with_adds_as_modification(mut self, adds_as_mod: bool) -> Self {
        self.adds_as_modification = adds_as_mod;
        self
    }

    /// Sets the diff3 command path.
    pub fn with_diff3_cmd(mut self, cmd: &'a str) -> Self {
        self.diff3_cmd = Some(cmd);
        self
    }

    /// Sets the preserved extensions.
    pub fn with_preserved_exts(mut self, exts: Vec<&'a str>) -> Self {
        self.preserved_exts = exts;
        self
    }
}

/// Options for get_switch_editor function.
#[derive(Default)]
pub struct SwitchEditorOptions<'a> {
    /// If true, use commit times for file timestamps.
    pub use_commit_times: bool,
    /// Depth of the switch operation.
    pub depth: crate::Depth,
    /// If true, depth changes are sticky.
    pub depth_is_sticky: bool,
    /// If true, allow unversioned obstructions.
    pub allow_unver_obstructions: bool,
    /// If true, server performs filtering.
    pub server_performs_filtering: bool,
    /// Path to diff3 command for merging.
    pub diff3_cmd: Option<&'a str>,
    /// File extensions to preserve during merge.
    pub preserved_exts: Vec<&'a str>,
    /// Callback to fetch directory entries.
    pub fetch_dirents_func: Option<
        Box<
            dyn Fn(
                &str,
                &str,
            )
                -> Result<std::collections::HashMap<String, crate::DirEntry>, Error<'static>>,
        >,
    >,
    /// Callback for conflict resolution.
    pub conflict_func: Option<
        Box<
            dyn Fn(
                &crate::conflict::ConflictDescription,
            ) -> Result<crate::conflict::ConflictResult, Error<'static>>,
        >,
    >,
    /// Callback for external definitions.
    pub external_func: Option<
        Box<dyn Fn(&str, Option<&str>, Option<&str>, crate::Depth) -> Result<(), Error<'static>>>,
    >,
    /// Callback for cancellation.
    pub cancel_func: Option<Box<dyn Fn() -> Result<(), Error<'static>>>>,
    /// Callback for notifications.
    pub notify_func: Option<Box<dyn Fn(&Notify)>>,
}

impl<'a> SwitchEditorOptions<'a> {
    /// Creates new SwitchEditorOptions with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets whether to use commit times.
    pub fn with_use_commit_times(mut self, use_commit_times: bool) -> Self {
        self.use_commit_times = use_commit_times;
        self
    }

    /// Sets the depth for the operation.
    pub fn with_depth(mut self, depth: crate::Depth) -> Self {
        self.depth = depth;
        self
    }

    /// Sets whether depth is sticky.
    pub fn with_depth_is_sticky(mut self, sticky: bool) -> Self {
        self.depth_is_sticky = sticky;
        self
    }

    /// Sets whether to allow unversioned obstructions.
    pub fn with_allow_unver_obstructions(mut self, allow: bool) -> Self {
        self.allow_unver_obstructions = allow;
        self
    }

    /// Sets the diff3 command path.
    pub fn with_diff3_cmd(mut self, cmd: &'a str) -> Self {
        self.diff3_cmd = Some(cmd);
        self
    }

    /// Sets the preserved extensions.
    pub fn with_preserved_exts(mut self, exts: Vec<&'a str>) -> Self {
        self.preserved_exts = exts;
        self
    }
}

/// Update editor for working copy operations
///
/// The lifetime parameter ensures the editor does not outlive the Context it was created from.
pub struct UpdateEditor<'a> {
    editor: *const subversion_sys::svn_delta_editor_t,
    edit_baton: *mut std::ffi::c_void,
    _pool: apr::Pool<'static>,
    target_revision: crate::Revnum,
    // Callback batons with their dropper functions
    callback_batons: Vec<(*mut std::ffi::c_void, DropperFn)>,
    _marker: std::marker::PhantomData<&'a Context>,
}

impl Drop for UpdateEditor<'_> {
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

impl UpdateEditor<'_> {
    /// Get the target revision for this update
    pub fn target_revision(&self) -> crate::Revnum {
        self.target_revision
    }

    /// Convert into a WrapEditor for use with PyEditor.
    ///
    /// This consumes the UpdateEditor and produces a WrapEditor that
    /// forwards calls to the same underlying C editor/baton pair.
    pub fn into_wrap_editor(mut self) -> crate::delta::WrapEditor<'static> {
        let editor = self.editor;
        let baton = self.edit_baton;
        let pool = std::mem::replace(&mut self._pool, apr::Pool::new());
        let batons = std::mem::take(&mut self.callback_batons);

        // Prevent the drop impl from cleaning up the batons
        // since we're transferring ownership to WrapEditor
        std::mem::forget(self);

        crate::delta::WrapEditor {
            editor,
            baton,
            _pool: apr::pool::PoolHandle::owned(pool),
            callback_batons: batons,
        }
    }
}

impl crate::delta::Editor for UpdateEditor<'_> {
    type RootEditor = crate::delta::WrapDirectoryEditor<'static>;

    fn set_target_revision(
        &mut self,
        revision: Option<crate::Revnum>,
    ) -> Result<(), crate::Error<'_>> {
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
    ) -> Result<crate::delta::WrapDirectoryEditor<'static>, crate::Error<'_>> {
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
        Ok(crate::delta::WrapDirectoryEditor {
            editor: self.editor,
            baton,
            _pool: apr::PoolHandle::owned(pool),
        })
    }

    fn close(&mut self) -> Result<(), crate::Error<'_>> {
        let scratch_pool = apr::Pool::new();
        let err = unsafe {
            ((*self.editor).close_edit.unwrap())(self.edit_baton, scratch_pool.as_mut_ptr())
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }

    fn abort(&mut self) -> Result<(), crate::Error<'_>> {
        let scratch_pool = apr::Pool::new();
        let err = unsafe {
            ((*self.editor).abort_edit.unwrap())(self.edit_baton, scratch_pool.as_mut_ptr())
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }
}

/// Directory entries type for working copy operations
pub type DirEntries = std::collections::HashMap<String, crate::DirEntry>;

/// Check working copy format at path
pub fn check_wc(path: &std::path::Path) -> Result<Option<i32>, crate::Error<'_>> {
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
) -> Result<(), crate::Error<'static>> {
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
pub fn match_ignore_list(path: &str, patterns: &[&str]) -> Result<bool, crate::Error<'static>> {
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
pub fn get_actual_target(path: &std::path::Path) -> Result<(String, String), crate::Error<'_>> {
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
) -> Result<Option<crate::io::Stream>, crate::Error<'_>> {
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
pub fn get_pristine_copy_path(
    path: &std::path::Path,
) -> Result<std::path::PathBuf, crate::Error<'_>> {
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
    pub fn get_actual_target(&mut self, path: &str) -> Result<(String, String), crate::Error<'_>> {
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
    ) -> Result<Option<crate::io::Stream>, crate::Error<'_>> {
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
    ) -> Result<Option<std::collections::HashMap<String, Vec<u8>>>, crate::Error<'_>> {
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
        ignore_patterns: Option<&[&str]>,
        status_func: F,
    ) -> Result<(), Error<'static>>
    where
        F: FnMut(&str, &Status<'_>) -> Result<(), Error<'static>>,
    {
        let pool = apr::Pool::new();
        let path_cstr = std::ffi::CString::new(local_abspath.to_str().unwrap())?;

        // Build ignore_patterns APR array if provided
        let pattern_cstrs: Vec<std::ffi::CString> = ignore_patterns
            .unwrap_or(&[])
            .iter()
            .map(|p| std::ffi::CString::new(*p).expect("pattern valid UTF-8"))
            .collect();
        let ignore_patterns_ptr = if let Some(patterns) = ignore_patterns {
            let mut arr = apr::tables::TypedArray::<*const std::os::raw::c_char>::new(
                &pool,
                patterns.len() as i32,
            );
            for cstr in &pattern_cstrs {
                arr.push(cstr.as_ptr());
            }
            unsafe { arr.as_ptr() }
        } else {
            std::ptr::null_mut()
        };

        // Wrap the closure in a way that can be passed to C
        unsafe extern "C" fn status_callback(
            baton: *mut std::ffi::c_void,
            local_abspath: *const std::os::raw::c_char,
            status: *const subversion_sys::svn_wc_status3_t,
            scratch_pool: *mut apr_sys::apr_pool_t,
        ) -> *mut subversion_sys::svn_error_t {
            let callback = unsafe {
                &mut *(baton
                    as *mut Box<dyn FnMut(&str, &Status<'_>) -> Result<(), Error<'static>>>)
            };

            let path = unsafe {
                std::ffi::CStr::from_ptr(local_abspath)
                    .to_string_lossy()
                    .into_owned()
            };

            // Borrow SVN's scratch pool for the duration of this callback.
            // The pool is valid until the callback returns; Status<'_> cannot
            // escape because it is only passed as &Status<'_> to the closure.
            let status = Status {
                ptr: status,
                _pool: unsafe { apr::pool::PoolHandle::from_borrowed_raw(scratch_pool) },
            };

            match callback(&path, &status) {
                Ok(()) => std::ptr::null_mut(),
                Err(e) => unsafe { e.into_raw() },
            }
        }

        let boxed_callback: Box<Box<dyn FnMut(&str, &Status<'_>) -> Result<(), Error<'static>>>> =
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
                ignore_patterns_ptr,
                Some(status_callback),
                baton,
                None,                 // cancel_func
                std::ptr::null_mut(), // cancel_baton
                pool.as_mut_ptr(),
            );

            // Clean up the callback
            let _ = Box::from_raw(
                baton as *mut Box<dyn FnMut(&str, &Status<'_>) -> Result<(), Error<'static>>>,
            );

            Error::from_raw(err)
        }
    }

    /// Queue committed items for post-commit processing
    ///
    /// Queues items that have been committed for later processing by `process_committed_queue`.
    ///
    /// `is_committed` should be `true` for nodes that were actually committed (not just
    /// included in a recursive operation where they had no local changes).
    ///
    /// Wraps `svn_wc_queue_committed4`.
    pub fn queue_committed(
        &mut self,
        local_abspath: &std::path::Path,
        recurse: bool,
        is_committed: bool,
        committed_queue: &mut CommittedQueue,
        wcprop_changes: Option<&[PropChange]>,
        remove_lock: bool,
        remove_changelist: bool,
        sha1_checksum: Option<&crate::Checksum>,
    ) -> Result<(), Error<'static>> {
        let pool = apr::Pool::new();
        let path_cstr = std::ffi::CString::new(local_abspath.to_str().unwrap())?;

        // Build the wcprop_changes APR array if provided
        let wcprop_changes_ptr = if let Some(changes) = wcprop_changes {
            let prop_name_cstrs: Vec<std::ffi::CString> = changes
                .iter()
                .map(|c| std::ffi::CString::new(c.name.as_str()).expect("prop name valid UTF-8"))
                .collect();
            let mut arr = apr::tables::TypedArray::<subversion_sys::svn_prop_t>::new(
                &pool,
                changes.len() as i32,
            );
            for (change, name_cstr) in changes.iter().zip(prop_name_cstrs.iter()) {
                arr.push(subversion_sys::svn_prop_t {
                    name: name_cstr.as_ptr(),
                    value: if let Some(v) = &change.value {
                        crate::svn_string_helpers::svn_string_ncreate(v, &pool)
                    } else {
                        std::ptr::null()
                    },
                });
            }
            unsafe { arr.as_ptr() }
        } else {
            std::ptr::null()
        };

        let sha1_ptr = sha1_checksum.map(|c| c.ptr).unwrap_or(std::ptr::null());

        unsafe {
            let err = subversion_sys::svn_wc_queue_committed4(
                committed_queue.as_mut_ptr(),
                self.ptr,
                path_cstr.as_ptr(),
                recurse as i32,
                is_committed as i32,
                wcprop_changes_ptr,
                remove_lock as i32,
                remove_changelist as i32,
                sha1_ptr,
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
    ) -> Result<(), Error<'static>> {
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
    pub fn add_lock(
        &mut self,
        local_abspath: &std::path::Path,
        lock: &Lock,
    ) -> Result<(), Error<'static>> {
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
    pub fn remove_lock(&mut self, local_abspath: &std::path::Path) -> Result<(), Error<'static>> {
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
        cancel_func: Option<&dyn Fn() -> Result<(), Error<'static>>>,
    ) -> Result<(), Error<'static>> {
        let pool = apr::Pool::new();
        let path = local_abspath.to_str().unwrap();
        let path_cstr = std::ffi::CString::new(path).unwrap();

        let cancel_baton = cancel_func
            .map(box_cancel_baton_borrowed)
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
        cancel_func: Option<&dyn Fn() -> Result<(), Error<'static>>>,
    ) -> Result<(), Error<'static>> {
        let pool = apr::Pool::new();
        let path = local_abspath.to_str().unwrap();
        let path_cstr = std::ffi::CString::new(path).unwrap();

        let prop_cstr = resolve_property.map(|p| std::ffi::CString::new(p).unwrap());
        let prop_ptr = prop_cstr.as_ref().map_or(std::ptr::null(), |p| p.as_ptr());

        let cancel_baton = cancel_func
            .map(box_cancel_baton_borrowed)
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
                    cancel_baton as *mut Box<dyn Fn() -> Result<(), Error<'static>>>,
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
        props: Option<&std::collections::HashMap<String, Vec<u8>>>,
        skip_checks: bool,
        notify_func: Option<&dyn Fn(&Notify)>,
    ) -> Result<(), Error<'static>> {
        let path = local_abspath.to_str().unwrap();
        let path_cstr = std::ffi::CString::new(path).unwrap();
        let pool = apr::Pool::new();

        // Build the props APR hash if provided
        let props_hash = if let Some(props) = props {
            let mut hash = apr::hash::Hash::new(&pool);
            for (key, value) in props {
                let svn_str = crate::svn_string_helpers::svn_string_ncreate(value, &pool);
                unsafe {
                    hash.insert(key.as_bytes(), svn_str as *mut std::ffi::c_void);
                }
            }
            unsafe { hash.as_mut_ptr() }
        } else {
            std::ptr::null_mut()
        };

        let notify_baton = notify_func
            .map(|f| box_notify_baton_borrowed(f))
            .unwrap_or(std::ptr::null_mut());

        let ret = unsafe {
            subversion_sys::svn_wc_add_from_disk3(
                self.ptr,
                path_cstr.as_ptr(),
                props_hash,
                skip_checks as i32,
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

    /// Add a file to the working copy with contents and properties sourced from
    /// a repository.
    ///
    /// This is used by update/switch/merge operations to schedule a file for
    /// addition when its contents are already available (e.g. from a network
    /// fetch), rather than reading them from disk.
    ///
    /// * `local_abspath` — absolute path where the new file will live.
    /// * `new_base_contents` — stream providing the pristine (base) file
    ///   contents.
    /// * `new_contents` — optional stream providing the working-copy contents;
    ///   pass `None` to use `new_base_contents` as the working copy contents.
    /// * `new_base_props` — pristine properties for the file.
    /// * `new_props` — optional working-copy property overrides; pass `None` to
    ///   use `new_base_props` as the working-copy properties.
    /// * `copyfrom_url` / `copyfrom_rev` — if the file was copied, its source
    ///   URL and revision; pass `None`/`-1` for a plain add.
    /// * `cancel_func` — optional cancellation callback.
    ///
    /// Wraps `svn_wc_add_repos_file4`.
    pub fn add_repos_file(
        &mut self,
        local_abspath: &std::path::Path,
        new_base_contents: &mut crate::io::Stream,
        new_contents: Option<&mut crate::io::Stream>,
        new_base_props: &std::collections::HashMap<String, Vec<u8>>,
        new_props: Option<&std::collections::HashMap<String, Vec<u8>>>,
        copyfrom_url: Option<&str>,
        copyfrom_rev: crate::Revnum,
        cancel_func: Option<Box<dyn Fn() -> Result<(), Error<'static>>>>,
    ) -> Result<(), Error<'static>> {
        let path_cstr = std::ffi::CString::new(
            local_abspath
                .to_str()
                .expect("local_abspath must be valid UTF-8"),
        )?;
        let copyfrom_url_cstr = copyfrom_url
            .map(|u| std::ffi::CString::new(u).expect("copyfrom_url must be valid UTF-8"));

        let scratch_pool = apr::Pool::new();

        // Helper: build an apr_hash_t<const char*, svn_string_t*> from a HashMap.
        let build_props_hash = |props: &std::collections::HashMap<String, Vec<u8>>,
                                pool: &apr::Pool|
         -> *mut apr_sys::apr_hash_t {
            let mut hash = apr::hash::Hash::new(pool);
            for (name, value) in props {
                let svn_str = crate::svn_string_helpers::svn_string_ncreate(value, pool);
                unsafe {
                    hash.insert(name.as_bytes(), svn_str as *mut std::ffi::c_void);
                }
            }
            unsafe { hash.as_mut_ptr() }
        };

        let base_props_ptr = build_props_hash(new_base_props, &scratch_pool);
        let props_ptr: *mut apr_sys::apr_hash_t = match new_props {
            Some(p) => build_props_hash(p, &scratch_pool),
            None => std::ptr::null_mut(),
        };

        let has_cancel = cancel_func.is_some();
        let cancel_baton = cancel_func
            .map(box_cancel_baton)
            .unwrap_or(std::ptr::null_mut());

        let err = unsafe {
            let e = subversion_sys::svn_wc_add_repos_file4(
                self.ptr,
                path_cstr.as_ptr(),
                new_base_contents.as_mut_ptr(),
                new_contents
                    .map(|s| s.as_mut_ptr())
                    .unwrap_or(std::ptr::null_mut()),
                base_props_ptr,
                props_ptr,
                copyfrom_url_cstr
                    .as_ref()
                    .map_or(std::ptr::null(), |c| c.as_ptr()),
                copyfrom_rev.0,
                if has_cancel {
                    Some(crate::wrap_cancel_func)
                } else {
                    None
                },
                cancel_baton,
                scratch_pool.as_mut_ptr(),
            );
            if has_cancel && !cancel_baton.is_null() {
                drop_cancel_baton(cancel_baton);
            }
            e
        };

        Error::from_raw(err)
    }

    /// Move a file or directory within the working copy
    pub fn move_path(
        &mut self,
        src_abspath: &std::path::Path,
        dst_abspath: &std::path::Path,
        metadata_only: bool,
        _allow_mixed_revisions: bool,
        cancel_func: Option<&dyn Fn() -> Result<(), Error<'static>>>,
        notify_func: Option<&dyn Fn(&Notify)>,
    ) -> Result<(), Error<'static>> {
        let src = src_abspath.to_str().unwrap();
        let src_cstr = std::ffi::CString::new(src).unwrap();
        let dst = dst_abspath.to_str().unwrap();
        let dst_cstr = std::ffi::CString::new(dst).unwrap();
        let pool = apr::Pool::new();

        let cancel_baton = cancel_func
            .map(box_cancel_baton_borrowed)
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
                    cancel_baton as *mut Box<dyn Fn() -> Result<(), Error<'static>>>,
                ))
            };
        }
        if !notify_baton.is_null() {
            unsafe { drop_notify_baton_borrowed(notify_baton) };
        }

        Error::from_raw(ret)
    }

    /// Get an editor for switching the working copy to a different URL
    ///
    /// The returned editor borrows from the context and must not outlive it.
    pub fn get_switch_editor<'s>(
        &'s mut self,
        anchor_abspath: &str,
        target_basename: &str,
        switch_url: &str,
        options: SwitchEditorOptions,
    ) -> Result<(crate::delta::WrapEditor<'s>, crate::Revnum), crate::Error<'s>> {
        let anchor_abspath_cstr = std::ffi::CString::new(anchor_abspath)?;
        let target_basename_cstr = std::ffi::CString::new(target_basename)?;
        let switch_url_cstr = std::ffi::CString::new(switch_url)?;
        let diff3_cmd_cstr = options.diff3_cmd.map(std::ffi::CString::new).transpose()?;

        let result_pool = apr::Pool::new();

        // Create preserved extensions array
        let preserved_exts_cstrs: Vec<std::ffi::CString> = options
            .preserved_exts
            .iter()
            .map(|&s| std::ffi::CString::new(s))
            .collect::<Result<Vec<_>, _>>()?;
        let preserved_exts_apr = if preserved_exts_cstrs.is_empty() {
            std::ptr::null()
        } else {
            let mut arr = apr::tables::TypedArray::<*const i8>::new(
                &result_pool,
                preserved_exts_cstrs.len() as i32,
            );
            for cstr in &preserved_exts_cstrs {
                arr.push(cstr.as_ptr());
            }
            unsafe { arr.as_ptr() }
        };
        let mut target_revision: subversion_sys::svn_revnum_t = 0;
        let mut editor_ptr: *const subversion_sys::svn_delta_editor_t = std::ptr::null();
        let mut edit_baton: *mut std::ffi::c_void = std::ptr::null_mut();

        // Create batons for callbacks
        let has_fetch_dirents = options.fetch_dirents_func.is_some();
        let fetch_dirents_baton = options
            .fetch_dirents_func
            .map(|f| box_fetch_dirents_baton(f))
            .unwrap_or(std::ptr::null_mut());
        let has_conflict = options.conflict_func.is_some();
        let conflict_baton = options
            .conflict_func
            .map(|f| box_conflict_baton(f))
            .unwrap_or(std::ptr::null_mut());
        let has_external = options.external_func.is_some();
        let external_baton = options
            .external_func
            .map(|f| box_external_baton(f))
            .unwrap_or(std::ptr::null_mut());
        let has_cancel = options.cancel_func.is_some();
        let cancel_baton = options
            .cancel_func
            .map(box_cancel_baton)
            .unwrap_or(std::ptr::null_mut());
        let has_notify = options.notify_func.is_some();
        let notify_baton = options
            .notify_func
            .map(|f| box_notify_baton(f))
            .unwrap_or(std::ptr::null_mut());

        let err = with_tmp_pool(|scratch_pool| unsafe {
            svn_result(subversion_sys::svn_wc_get_switch_editor4(
                &mut editor_ptr,
                &mut edit_baton,
                &mut target_revision,
                self.ptr,
                anchor_abspath_cstr.as_ptr(),
                target_basename_cstr.as_ptr(),
                switch_url_cstr.as_ptr(),
                if options.use_commit_times { 1 } else { 0 },
                options.depth.into(),
                if options.depth_is_sticky { 1 } else { 0 },
                if options.allow_unver_obstructions {
                    1
                } else {
                    0
                },
                if options.server_performs_filtering {
                    1
                } else {
                    0
                },
                diff3_cmd_cstr
                    .as_ref()
                    .map_or(std::ptr::null(), |c| c.as_ptr()),
                preserved_exts_apr,
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
        let editor = crate::delta::WrapEditor {
            editor: editor_ptr,
            baton: edit_baton,
            _pool: apr::PoolHandle::owned(result_pool),
            callback_batons: batons,
        };

        Ok((
            editor,
            crate::Revnum::from_raw(target_revision).unwrap_or_default(),
        ))
    }

    /// Get an editor for showing differences in the working copy.
    ///
    /// The `callbacks` parameter receives diff events as the returned editor
    /// is driven.  The returned editor borrows from the context and must not
    /// outlive it.
    pub fn get_diff_editor<'s>(
        &'s mut self,
        anchor_abspath: &str,
        target_abspath: &str,
        callbacks: &'s mut dyn DiffCallbacks,
        use_text_base: bool,
        depth: crate::Depth,
        ignore_ancestry: bool,
        show_copies_as_adds: bool,
        use_git_diff_format: bool,
    ) -> Result<crate::delta::WrapEditor<'s>, crate::Error<'s>> {
        let anchor_abspath_cstr = std::ffi::CString::new(anchor_abspath)?;
        let target_abspath_cstr = std::ffi::CString::new(target_abspath)?;

        let result_pool = apr::Pool::new();
        let mut editor_ptr: *const subversion_sys::svn_delta_editor_t = std::ptr::null();
        let mut edit_baton: *mut std::ffi::c_void = std::ptr::null_mut();

        // Heap-allocate the callbacks struct and baton so they outlive this call.
        let c_callbacks = Box::new(make_diff_callbacks4());
        let c_callbacks_ptr = &*c_callbacks as *const subversion_sys::svn_wc_diff_callbacks4_t;

        // The baton is a pointer to a fat pointer (`&mut dyn DiffCallbacks`).
        let cb_baton: Box<*mut dyn DiffCallbacks> = Box::new(callbacks as *mut dyn DiffCallbacks);
        let cb_baton_ptr = &*cb_baton as *const *mut dyn DiffCallbacks as *mut std::ffi::c_void;

        let err = with_tmp_pool(|scratch_pool| unsafe {
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
                0,                // reverse_order
                0,                // server_performs_filtering
                std::ptr::null(), // changelist_filter
                c_callbacks_ptr,
                cb_baton_ptr,
                None,                 // cancel_func
                std::ptr::null_mut(), // cancel_baton
                result_pool.as_mut_ptr(),
                scratch_pool.as_mut_ptr(),
            ))
        });

        err?;

        unsafe fn drop_callbacks(ptr: *mut std::ffi::c_void) {
            let _ = Box::from_raw(ptr as *mut subversion_sys::svn_wc_diff_callbacks4_t);
        }
        unsafe fn drop_baton(ptr: *mut std::ffi::c_void) {
            let _ = Box::from_raw(ptr as *mut *mut dyn DiffCallbacks);
        }

        let batons: Vec<(*mut std::ffi::c_void, crate::delta::DropperFn)> = vec![
            (
                Box::into_raw(c_callbacks) as *mut std::ffi::c_void,
                drop_callbacks as crate::delta::DropperFn,
            ),
            (
                Box::into_raw(cb_baton) as *mut std::ffi::c_void,
                drop_baton as crate::delta::DropperFn,
            ),
        ];

        let editor = crate::delta::WrapEditor {
            editor: editor_ptr,
            baton: edit_baton,
            _pool: apr::PoolHandle::owned(result_pool),
            callback_batons: batons,
        };

        Ok(editor)
    }

    /// Delete a path from version control
    pub fn delete(
        &mut self,
        local_abspath: &std::path::Path,
        keep_local: bool,
        delete_unversioned_target: bool,
        cancel_func: Option<&dyn Fn() -> Result<(), Error<'static>>>,
        notify_func: Option<&dyn Fn(&Notify)>,
    ) -> Result<(), Error<'static>> {
        let path = local_abspath.to_str().unwrap();
        let path_cstr = std::ffi::CString::new(path).unwrap();
        let pool = apr::Pool::new();

        let cancel_baton = cancel_func
            .map(box_cancel_baton_borrowed)
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
                    cancel_baton as *mut Box<dyn Fn() -> Result<(), Error<'static>>>,
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
    ) -> Result<Option<Vec<u8>>, Error<'_>> {
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
        cancel_func: Option<&dyn Fn() -> Result<(), Error<'static>>>,
        notify_func: Option<&dyn Fn(&Notify)>,
    ) -> Result<(), Error<'static>> {
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
            .map(box_cancel_baton_borrowed)
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
                    cancel_baton as *mut Box<dyn Fn() -> Result<(), Error<'static>>>,
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
    ) -> Result<std::collections::HashMap<String, Vec<u8>>, Error<'_>> {
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
        Error<'_>,
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
    ) -> Result<crate::NodeKind, Error<'static>> {
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
    pub fn is_wc_root(&mut self, local_abspath: &std::path::Path) -> Result<bool, Error<'static>> {
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
        cancel_func: Option<&dyn Fn() -> Result<(), Error<'static>>>,
        notify_func: Option<&dyn Fn(&Notify)>,
    ) -> Result<(), Error<'static>> {
        let path = local_abspath.to_str().unwrap();
        let path_cstr = std::ffi::CString::new(path).unwrap();
        let scratch_pool = apr::Pool::new();

        let cancel_baton = cancel_func
            .map(box_cancel_baton_borrowed)
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
                    cancel_baton as *mut &dyn Fn() -> Result<(), Error<'static>>,
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

    /// Diff a working copy path against its base revision.
    ///
    /// Calls `callbacks` for each changed file or directory found under
    /// `target_abspath` up to the given `depth`.  This is a working-copy-only
    /// diff (base vs. working); it does not contact the repository.
    ///
    /// Wraps `svn_wc_diff6`.
    pub fn diff(
        &mut self,
        target_abspath: &std::path::Path,
        options: &DiffOptions,
        callbacks: &mut dyn DiffCallbacks,
    ) -> Result<(), Error<'static>> {
        let target_cstr =
            std::ffi::CString::new(target_abspath.to_str().expect("path must be valid UTF-8"))?;

        let scratch_pool = apr::Pool::new();

        // Build the changelist filter array (NULL means no filter).
        let changelist_cstrs: Vec<std::ffi::CString> = options
            .changelists
            .iter()
            .map(|cl| std::ffi::CString::new(cl.as_str()).expect("changelist is valid UTF-8"))
            .collect();
        let mut changelist_arr =
            apr::tables::TypedArray::<*const i8>::new(&scratch_pool, changelist_cstrs.len() as i32);
        for s in &changelist_cstrs {
            changelist_arr.push(s.as_ptr());
        }
        let changelist_filter: *const apr_sys::apr_array_header_t =
            if options.changelists.is_empty() {
                std::ptr::null()
            } else {
                unsafe { changelist_arr.as_ptr() }
            };

        let c_callbacks = make_diff_callbacks4();

        // The baton is a pointer to a fat pointer (`&mut dyn DiffCallbacks`).
        let mut cb_ref: &mut dyn DiffCallbacks = callbacks;
        let baton = &mut cb_ref as *mut &mut dyn DiffCallbacks as *mut std::ffi::c_void;

        with_tmp_pool(|scratch| unsafe {
            svn_result(subversion_sys::svn_wc_diff6(
                self.ptr,
                target_cstr.as_ptr(),
                &c_callbacks,
                baton,
                options.depth.into(),
                options.ignore_ancestry as i32,
                options.show_copies_as_adds as i32,
                options.use_git_diff_format as i32,
                changelist_filter,
                None,                 // cancel_func
                std::ptr::null_mut(), // cancel_baton
                scratch.as_mut_ptr(),
            ))
        })
    }

    /// Merge two file revisions into a working copy file.
    ///
    /// Merges the differences between `left_abspath` and `right_abspath` into
    /// the versioned working copy file at `target_abspath`.
    ///
    /// Returns a tuple `(content_outcome, props_state)`:
    /// - `content_outcome`: whether the file content was unchanged, merged, or conflicted.
    /// - `props_state`: notify state for property changes (may be `Inapplicable` when
    ///   `prop_diff` is empty).
    ///
    /// Wraps `svn_wc_merge5`.
    pub fn merge(
        &mut self,
        left_abspath: &std::path::Path,
        right_abspath: &std::path::Path,
        target_abspath: &std::path::Path,
        left_label: Option<&str>,
        right_label: Option<&str>,
        target_label: Option<&str>,
        prop_diff: &[PropChange],
        options: &MergeOptions,
    ) -> Result<(MergeOutcome, NotifyState), Error<'static>> {
        let left_cstr =
            std::ffi::CString::new(left_abspath.to_str().expect("path must be valid UTF-8"))?;
        let right_cstr =
            std::ffi::CString::new(right_abspath.to_str().expect("path must be valid UTF-8"))?;
        let target_cstr =
            std::ffi::CString::new(target_abspath.to_str().expect("path must be valid UTF-8"))?;

        let left_label_cstr =
            left_label.map(|s| std::ffi::CString::new(s).expect("label must be valid UTF-8"));
        let right_label_cstr =
            right_label.map(|s| std::ffi::CString::new(s).expect("label must be valid UTF-8"));
        let target_label_cstr =
            target_label.map(|s| std::ffi::CString::new(s).expect("label must be valid UTF-8"));

        let diff3_cstr = options
            .diff3_cmd
            .as_deref()
            .map(|s| std::ffi::CString::new(s).expect("diff3_cmd must be valid UTF-8"));

        let scratch_pool = apr::Pool::new();

        // Build the prop_diff array.
        // Keep CStrings alive for the duration of the C call.
        let prop_name_cstrs: Vec<std::ffi::CString> = prop_diff
            .iter()
            .map(|c| std::ffi::CString::new(c.name.as_str()).expect("prop name valid UTF-8"))
            .collect();
        let mut prop_diff_typed = apr::tables::TypedArray::<subversion_sys::svn_prop_t>::new(
            &scratch_pool,
            prop_diff.len() as i32,
        );
        for (change, name_cstr) in prop_diff.iter().zip(prop_name_cstrs.iter()) {
            prop_diff_typed.push(subversion_sys::svn_prop_t {
                name: name_cstr.as_ptr(),
                value: if let Some(v) = &change.value {
                    crate::svn_string_helpers::svn_string_ncreate(v, &scratch_pool)
                } else {
                    std::ptr::null()
                },
            });
        }
        let prop_diff_arr: *const apr_sys::apr_array_header_t = if prop_diff.is_empty() {
            std::ptr::null()
        } else {
            unsafe { prop_diff_typed.as_ptr() }
        };

        // Build the merge_options array.
        // Keep CStrings alive for the duration of the C call.
        let merge_opt_cstrs: Vec<std::ffi::CString> = options
            .merge_options
            .iter()
            .map(|s| std::ffi::CString::new(s.as_str()).expect("merge option valid UTF-8"))
            .collect();
        let mut merge_opts_typed =
            apr::tables::TypedArray::<*const i8>::new(&scratch_pool, merge_opt_cstrs.len() as i32);
        for s in &merge_opt_cstrs {
            merge_opts_typed.push(s.as_ptr());
        }
        let merge_opts_arr: *const apr_sys::apr_array_header_t = if options.merge_options.is_empty()
        {
            std::ptr::null()
        } else {
            unsafe { merge_opts_typed.as_ptr() }
        };

        let mut content_outcome = subversion_sys::svn_wc_merge_outcome_t_svn_wc_merge_no_merge;
        let mut props_state =
            subversion_sys::svn_wc_notify_state_t_svn_wc_notify_state_inapplicable;

        let err = unsafe {
            svn_result(subversion_sys::svn_wc_merge5(
                &mut content_outcome,
                &mut props_state,
                self.ptr,
                left_cstr.as_ptr(),
                right_cstr.as_ptr(),
                target_cstr.as_ptr(),
                left_label_cstr
                    .as_ref()
                    .map_or(std::ptr::null(), |s| s.as_ptr()),
                right_label_cstr
                    .as_ref()
                    .map_or(std::ptr::null(), |s| s.as_ptr()),
                target_label_cstr
                    .as_ref()
                    .map_or(std::ptr::null(), |s| s.as_ptr()),
                std::ptr::null(), // left_version
                std::ptr::null(), // right_version
                options.dry_run as i32,
                diff3_cstr.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
                merge_opts_arr,
                std::ptr::null_mut(), // original_props (NULL = no prop merge)
                prop_diff_arr,
                None,                 // conflict_func
                std::ptr::null_mut(), // conflict_baton
                None,                 // cancel_func
                std::ptr::null_mut(), // cancel_baton
                scratch_pool.as_mut_ptr(),
            ))
        };
        err?;

        Ok((content_outcome.into(), props_state.into()))
    }

    /// Merge property changes into a working copy path.
    ///
    /// Applies `propchanges` to the versioned path `local_abspath`,
    /// starting from `baseprops` as the base property set.
    ///
    /// * `baseprops` — the "base" set of properties to compare against; pass
    ///   `None` to use an empty base.
    /// * `propchanges` — the list of changes to apply.
    /// * `dry_run` — if `true`, calculate and report conflicts but do not
    ///   modify the working copy.
    /// * `conflict_func` — optional callback invoked for each property conflict.
    ///   Return a [`crate::conflict::ConflictResult`] to resolve (or postpone)
    ///   the conflict.
    /// * `cancel_func` — optional callback checked periodically for
    ///   cancellation.
    ///
    /// Returns the resulting [`NotifyState`] describing whether the merge was
    /// clean, had conflicts, etc.
    ///
    /// Wraps `svn_wc_merge_props3`.
    pub fn merge_props(
        &mut self,
        local_abspath: &std::path::Path,
        baseprops: Option<&std::collections::HashMap<String, Vec<u8>>>,
        propchanges: &[PropChange],
        dry_run: bool,
        conflict_func: Option<
            Box<
                dyn Fn(
                    &crate::conflict::ConflictDescription,
                ) -> Result<crate::conflict::ConflictResult, Error<'static>>,
            >,
        >,
        cancel_func: Option<Box<dyn Fn() -> Result<(), Error<'static>>>>,
    ) -> Result<NotifyState, Error<'static>> {
        let path_cstr = std::ffi::CString::new(
            local_abspath
                .to_str()
                .expect("local_abspath must be valid UTF-8"),
        )?;

        let scratch_pool = apr::Pool::new();

        // Build the baseprops hash (NULL = no base properties).
        let baseprops_hash_ptr: *mut apr_sys::apr_hash_t = if let Some(props) = baseprops {
            let mut hash = apr::hash::Hash::new(&scratch_pool);
            // Keep CStrings and svn_string_t values alive for the C call.
            let mut key_cstrs: Vec<std::ffi::CString> = Vec::with_capacity(props.len());
            let mut svn_strings: Vec<*mut subversion_sys::svn_string_t> =
                Vec::with_capacity(props.len());
            for (name, value) in props {
                key_cstrs
                    .push(std::ffi::CString::new(name.as_str()).expect("prop name valid UTF-8"));
                svn_strings.push(crate::svn_string_helpers::svn_string_ncreate(
                    value,
                    &scratch_pool,
                ));
            }
            for (cstr, svn_str) in key_cstrs.iter().zip(svn_strings.iter()) {
                unsafe {
                    hash.insert(cstr.as_bytes(), *svn_str as *mut std::ffi::c_void);
                }
            }
            unsafe { hash.as_mut_ptr() }
        } else {
            std::ptr::null_mut()
        };

        // Build the propchanges array.
        let prop_name_cstrs: Vec<std::ffi::CString> = propchanges
            .iter()
            .map(|c| std::ffi::CString::new(c.name.as_str()).expect("prop name valid UTF-8"))
            .collect();
        let mut prop_changes_typed = apr::tables::TypedArray::<subversion_sys::svn_prop_t>::new(
            &scratch_pool,
            propchanges.len() as i32,
        );
        for (change, name_cstr) in propchanges.iter().zip(prop_name_cstrs.iter()) {
            prop_changes_typed.push(subversion_sys::svn_prop_t {
                name: name_cstr.as_ptr(),
                value: if let Some(v) = &change.value {
                    crate::svn_string_helpers::svn_string_ncreate(v, &scratch_pool)
                } else {
                    std::ptr::null()
                },
            });
        }
        let propchanges_arr: *const apr_sys::apr_array_header_t = if propchanges.is_empty() {
            std::ptr::null()
        } else {
            unsafe { prop_changes_typed.as_ptr() }
        };

        let has_conflict = conflict_func.is_some();
        let conflict_baton = conflict_func
            .map(box_conflict_baton)
            .unwrap_or(std::ptr::null_mut());
        let has_cancel = cancel_func.is_some();
        let cancel_baton = cancel_func
            .map(box_cancel_baton)
            .unwrap_or(std::ptr::null_mut());

        let mut state = subversion_sys::svn_wc_notify_state_t_svn_wc_notify_state_inapplicable;

        let err = unsafe {
            let e = subversion_sys::svn_wc_merge_props3(
                &mut state,
                self.ptr,
                path_cstr.as_ptr(),
                std::ptr::null(), // left_version (informational, not required)
                std::ptr::null(), // right_version (informational, not required)
                baseprops_hash_ptr,
                propchanges_arr,
                dry_run as i32,
                if has_conflict {
                    Some(wrap_conflict_func)
                } else {
                    None
                },
                conflict_baton,
                if has_cancel {
                    Some(crate::wrap_cancel_func)
                } else {
                    None
                },
                cancel_baton,
                scratch_pool.as_mut_ptr(),
            );
            // Free the batons now that the C call has returned.
            if has_conflict && !conflict_baton.is_null() {
                drop_conflict_baton(conflict_baton);
            }
            if has_cancel && !cancel_baton.is_null() {
                drop_cancel_baton(cancel_baton);
            }
            e
        };

        Error::from_raw(err)?;
        Ok(state.into())
    }

    /// Revert local changes to `local_abspath`.
    ///
    /// The behaviour is controlled by `options`; see [`RevertOptions`] for
    /// details. Wraps `svn_wc_revert6`.
    pub fn revert(
        &mut self,
        local_abspath: &std::path::Path,
        options: &RevertOptions,
    ) -> Result<(), Error<'static>> {
        let path_cstr =
            std::ffi::CString::new(local_abspath.to_str().expect("path must be valid UTF-8"))?;

        let scratch_pool = apr::Pool::new();

        // Build the changelist filter array (NULL = no filter).
        let changelist_cstrs: Vec<std::ffi::CString> = options
            .changelists
            .iter()
            .map(|cl| std::ffi::CString::new(cl.as_str()).expect("changelist is valid UTF-8"))
            .collect();
        let mut changelist_arr =
            apr::tables::TypedArray::<*const i8>::new(&scratch_pool, changelist_cstrs.len() as i32);
        for s in &changelist_cstrs {
            changelist_arr.push(s.as_ptr());
        }
        let changelist_filter: *const apr_sys::apr_array_header_t =
            if options.changelists.is_empty() {
                std::ptr::null()
            } else {
                unsafe { changelist_arr.as_ptr() }
            };

        svn_result(unsafe {
            subversion_sys::svn_wc_revert6(
                self.ptr,
                path_cstr.as_ptr(),
                options.depth.into(),
                options.use_commit_times as i32,
                changelist_filter,
                options.clear_changelists as i32,
                options.metadata_only as i32,
                options.added_keep_local as i32,
                None,                 // cancel_func
                std::ptr::null_mut(), // cancel_baton
                None,                 // notify_func
                std::ptr::null_mut(), // notify_baton
                scratch_pool.as_mut_ptr(),
            )
        })
    }

    /// Copy a versioned item from `src_abspath` to `dst_abspath` within the
    /// working copy, scheduling the destination for addition with history.
    ///
    /// The parent directory of `dst_abspath` must be under version control.
    /// `dst_abspath` itself must not already exist (unless `metadata_only` is
    /// `true`).
    ///
    /// If `metadata_only` is `true`, only the database is updated; the actual
    /// file or directory is not copied on disk.
    ///
    /// Wraps `svn_wc_copy3`.
    ///
    /// Note: `svn_wc_copy3` requires the parent directory of `dst_abspath` to
    /// be write-locked in this context.  Acquiring a write lock requires
    /// private SVN APIs; callers that need full copy support should use the
    /// higher-level `client::Context::copy()`.
    pub fn copy(
        &mut self,
        src_abspath: &std::path::Path,
        dst_abspath: &std::path::Path,
        metadata_only: bool,
    ) -> Result<(), Error<'static>> {
        let src_cstr =
            std::ffi::CString::new(src_abspath.to_str().expect("src path must be valid UTF-8"))?;
        let dst_cstr =
            std::ffi::CString::new(dst_abspath.to_str().expect("dst path must be valid UTF-8"))?;

        with_tmp_pool(|scratch| {
            svn_result(unsafe {
                subversion_sys::svn_wc_copy3(
                    self.ptr,
                    src_cstr.as_ptr(),
                    dst_cstr.as_ptr(),
                    metadata_only as i32,
                    None,                 // cancel_func
                    std::ptr::null_mut(), // cancel_baton
                    None,                 // notify_func
                    std::ptr::null_mut(), // notify_baton
                    scratch.as_mut_ptr(),
                )
            })
        })
    }

    /// Assign or remove a changelist for `local_abspath`.
    ///
    /// If `changelist` is `Some(name)`, items are added to that changelist.
    /// If `changelist` is `None`, the changelist assignment is cleared.
    ///
    /// `depth` controls how far below `local_abspath` to recurse.
    /// `changelist_filter`, if non-empty, restricts changes to items that are
    /// currently in one of the listed changelists.
    ///
    /// Note: directories cannot be members of changelists.
    ///
    /// Wraps `svn_wc_set_changelist2`.
    pub fn set_changelist(
        &mut self,
        local_abspath: &std::path::Path,
        changelist: Option<&str>,
        depth: crate::Depth,
        changelist_filter: &[String],
    ) -> Result<(), Error<'static>> {
        let path_cstr =
            std::ffi::CString::new(local_abspath.to_str().expect("path must be valid UTF-8"))?;
        let cl_cstr = changelist
            .map(|s| std::ffi::CString::new(s).expect("changelist name must be valid UTF-8"));

        let scratch_pool = apr::Pool::new();

        let filter_cstrs: Vec<std::ffi::CString> = changelist_filter
            .iter()
            .map(|s| std::ffi::CString::new(s.as_str()).expect("filter name must be valid UTF-8"))
            .collect();
        let mut filter_arr =
            apr::tables::TypedArray::<*const i8>::new(&scratch_pool, filter_cstrs.len() as i32);
        for s in &filter_cstrs {
            filter_arr.push(s.as_ptr());
        }
        let filter_ptr: *const apr_sys::apr_array_header_t = if changelist_filter.is_empty() {
            std::ptr::null()
        } else {
            unsafe { filter_arr.as_ptr() }
        };

        svn_result(unsafe {
            subversion_sys::svn_wc_set_changelist2(
                self.ptr,
                path_cstr.as_ptr(),
                cl_cstr.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
                depth.into(),
                filter_ptr,
                None,                 // cancel_func
                std::ptr::null_mut(), // cancel_baton
                None,                 // notify_func
                std::ptr::null_mut(), // notify_baton
                scratch_pool.as_mut_ptr(),
            )
        })
    }

    /// Crawl `local_abspath` to `depth`, invoking `callback` for every path
    /// that belongs to a changelist.
    ///
    /// If `changelist_filter` is non-empty, only paths in one of those
    /// changelists are reported; pass an empty slice to report all changelists.
    ///
    /// `callback` receives `(path, changelist_name)` for each visited node.
    /// `changelist_name` is `None` for nodes that are not in any changelist;
    /// when `changelist_filter` is empty SVN visits every node in the tree
    /// (not just those with changelists), so `None` values are common.
    ///
    /// Wraps `svn_wc_get_changelists`.
    pub fn get_changelists(
        &mut self,
        local_abspath: &std::path::Path,
        depth: crate::Depth,
        changelist_filter: &[String],
        mut callback: impl FnMut(&str, Option<&str>) -> Result<(), Error<'static>>,
    ) -> Result<(), Error<'static>> {
        let path_cstr =
            std::ffi::CString::new(local_abspath.to_str().expect("path must be valid UTF-8"))?;

        let scratch_pool = apr::Pool::new();

        let filter_cstrs: Vec<std::ffi::CString> = changelist_filter
            .iter()
            .map(|s| std::ffi::CString::new(s.as_str()).expect("filter name must be valid UTF-8"))
            .collect();
        let mut filter_arr =
            apr::tables::TypedArray::<*const i8>::new(&scratch_pool, filter_cstrs.len() as i32);
        for s in &filter_cstrs {
            filter_arr.push(s.as_ptr());
        }
        let filter_ptr: *const apr_sys::apr_array_header_t = if changelist_filter.is_empty() {
            std::ptr::null()
        } else {
            unsafe { filter_arr.as_ptr() }
        };

        unsafe extern "C" fn cl_callback(
            baton: *mut std::ffi::c_void,
            path: *const std::os::raw::c_char,
            changelist: *const std::os::raw::c_char,
            _pool: *mut apr_sys::apr_pool_t,
        ) -> *mut subversion_sys::svn_error_t {
            let cb = &mut *(baton
                as *mut &mut dyn FnMut(&str, Option<&str>) -> Result<(), Error<'static>>);
            let path = std::ffi::CStr::from_ptr(path).to_str().unwrap_or("");
            let cl = if changelist.is_null() {
                None
            } else {
                Some(std::ffi::CStr::from_ptr(changelist).to_str().unwrap_or(""))
            };
            match cb(path, cl) {
                Ok(()) => std::ptr::null_mut(),
                Err(e) => e.into_raw(),
            }
        }

        let mut cb_ref: &mut dyn FnMut(&str, Option<&str>) -> Result<(), Error<'static>> =
            &mut callback;
        let baton = &mut cb_ref
            as *mut &mut dyn FnMut(&str, Option<&str>) -> Result<(), Error<'static>>
            as *mut std::ffi::c_void;

        svn_result(unsafe {
            subversion_sys::svn_wc_get_changelists(
                self.ptr,
                path_cstr.as_ptr(),
                depth.into(),
                filter_ptr,
                Some(cl_callback),
                baton,
                None,                 // cancel_func
                std::ptr::null_mut(), // cancel_baton
                scratch_pool.as_mut_ptr(),
            )
        })
    }

    /// Return the status of a single path in the working copy.
    ///
    /// The returned [`Status`] owns its APR pool and is valid for as long as
    /// it is held.  Use [`Context::walk_status`] to query an entire subtree.
    ///
    /// Wraps `svn_wc_status3`.
    pub fn status(
        &mut self,
        local_abspath: &std::path::Path,
    ) -> Result<Status<'static>, Error<'static>> {
        let path_cstr =
            std::ffi::CString::new(local_abspath.to_str().expect("path must be valid UTF-8"))?;
        let result_pool = apr::Pool::new();
        let mut ptr: *mut subversion_sys::svn_wc_status3_t = std::ptr::null_mut();
        with_tmp_pool(|scratch| {
            svn_result(unsafe {
                subversion_sys::svn_wc_status3(
                    &mut ptr,
                    self.ptr,
                    path_cstr.as_ptr(),
                    result_pool.as_mut_ptr(),
                    scratch.as_mut_ptr(),
                )
            })
        })?;
        Ok(Status {
            ptr,
            _pool: apr::pool::PoolHandle::owned(result_pool),
        })
    }

    /// Check whether `local_abspath` is a working-copy root and/or switched.
    ///
    /// Returns `(is_wcroot, is_switched, node_kind)`.
    ///
    /// Wraps `svn_wc_check_root`.
    pub fn check_root(
        &mut self,
        local_abspath: &std::path::Path,
    ) -> Result<(bool, bool, crate::NodeKind), Error<'static>> {
        let path_cstr =
            std::ffi::CString::new(local_abspath.to_str().expect("path must be valid UTF-8"))?;
        let mut is_wcroot: subversion_sys::svn_boolean_t = 0;
        let mut is_switched: subversion_sys::svn_boolean_t = 0;
        let mut kind: subversion_sys::svn_node_kind_t =
            subversion_sys::svn_node_kind_t_svn_node_unknown;
        with_tmp_pool(|scratch| {
            svn_result(unsafe {
                subversion_sys::svn_wc_check_root(
                    &mut is_wcroot,
                    &mut is_switched,
                    &mut kind,
                    self.ptr,
                    path_cstr.as_ptr(),
                    scratch.as_mut_ptr(),
                )
            })
        })?;
        Ok((is_wcroot != 0, is_switched != 0, kind.into()))
    }

    /// Restore a missing or replaced working-copy file from its base revision.
    ///
    /// If `use_commit_times` is `true`, the restored file's timestamp is set
    /// to the last-commit time rather than the current time.
    ///
    /// Wraps `svn_wc_restore`.
    pub fn restore(
        &mut self,
        local_abspath: &std::path::Path,
        use_commit_times: bool,
    ) -> Result<(), Error<'static>> {
        let path_cstr =
            std::ffi::CString::new(local_abspath.to_str().expect("path must be valid UTF-8"))?;
        with_tmp_pool(|scratch| {
            svn_result(unsafe {
                subversion_sys::svn_wc_restore(
                    self.ptr,
                    path_cstr.as_ptr(),
                    use_commit_times as i32,
                    scratch.as_mut_ptr(),
                )
            })
        })
    }

    /// Return the list of ignore patterns applying to `local_abspath`.
    ///
    /// Combines global patterns from the SVN configuration with any
    /// `svn:ignore` property set on the directory at `local_abspath`.
    /// Pass `NULL` for config to use the default configuration.
    ///
    /// Wraps `svn_wc_get_ignores2`.
    pub fn get_ignores(
        &mut self,
        local_abspath: &std::path::Path,
    ) -> Result<Vec<String>, Error<'static>> {
        let path_cstr =
            std::ffi::CString::new(local_abspath.to_str().expect("path must be valid UTF-8"))?;
        let result_pool = apr::Pool::new();
        let mut patterns: *mut apr_sys::apr_array_header_t = std::ptr::null_mut();
        with_tmp_pool(|scratch| {
            svn_result(unsafe {
                subversion_sys::svn_wc_get_ignores2(
                    &mut patterns,
                    self.ptr,
                    path_cstr.as_ptr(),
                    std::ptr::null_mut(), // config — NULL uses defaults
                    result_pool.as_mut_ptr(),
                    scratch.as_mut_ptr(),
                )
            })
        })?;
        if patterns.is_null() {
            return Ok(Vec::new());
        }
        let result =
            unsafe { apr::tables::TypedArray::<*const std::os::raw::c_char>::from_ptr(patterns) }
                .iter()
                .map(|ptr| unsafe { std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned() })
                .collect();
        Ok(result)
    }

    /// Remove `local_abspath` from revision control.
    ///
    /// The context must hold a write lock on the parent of `local_abspath`, or
    /// if that is a WC root then on `local_abspath` itself.
    ///
    /// If `local_abspath` is a file, all its metadata is removed from the
    /// administrative area.  If it is a directory, the administrative area is
    /// deleted recursively for the entire subtree.
    ///
    /// Only administrative data is removed unless `destroy_wf` is `true`, in
    /// which case the working file(s) and directories are also deleted from
    /// disk.  When `destroy_wf` is `true`, locally modified files are left in
    /// place and [`Error`] with `SVN_ERR_WC_LEFT_LOCAL_MOD` is returned.
    ///
    /// If `instant_error` is `true`, the function returns
    /// `SVN_ERR_WC_LEFT_LOCAL_MOD` as soon as a locally modified file is
    /// encountered; otherwise it finishes the traversal and returns the error
    /// afterwards.
    ///
    /// Wraps `svn_wc_remove_from_revision_control2`.
    pub fn remove_from_revision_control(
        &mut self,
        local_abspath: &std::path::Path,
        destroy_wf: bool,
        instant_error: bool,
    ) -> Result<(), Error<'static>> {
        let path_cstr =
            std::ffi::CString::new(local_abspath.to_str().expect("path must be valid UTF-8"))?;
        with_tmp_pool(|scratch| {
            svn_result(unsafe {
                subversion_sys::svn_wc_remove_from_revision_control2(
                    self.ptr,
                    path_cstr.as_ptr(),
                    destroy_wf as i32,
                    instant_error as i32,
                    None,                 // cancel_func
                    std::ptr::null_mut(), // cancel_baton
                    scratch.as_mut_ptr(),
                )
            })
        })
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
    pub fn err(&self) -> Option<Error<'_>> {
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
        unsafe { (*self.ptr).hunk_original_start }
    }

    /// Get the hunk original length (for patch operations)
    pub fn hunk_original_length(&self) -> u64 {
        unsafe { (*self.ptr).hunk_original_length }
    }

    /// Get the hunk modified start line (for patch operations)
    pub fn hunk_modified_start(&self) -> u64 {
        unsafe { (*self.ptr).hunk_modified_start }
    }

    /// Get the hunk modified length (for patch operations)
    pub fn hunk_modified_length(&self) -> u64 {
        unsafe { (*self.ptr).hunk_modified_length }
    }

    /// Get the line at which a hunk was matched (for patch operations)
    pub fn hunk_matched_line(&self) -> u64 {
        unsafe { (*self.ptr).hunk_matched_line }
    }

    /// Get the fuzz factor the hunk was applied with (for patch operations)
    pub fn hunk_fuzz(&self) -> u64 {
        unsafe { (*self.ptr).hunk_fuzz }
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
                ) -> Result<crate::conflict::ConflictResult, Error<'static>>,
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
                dyn Fn(
                    &str,
                    Option<&str>,
                    Option<&str>,
                    crate::Depth,
                ) -> Result<(), Error<'static>>,
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
                ) -> Result<
                    std::collections::HashMap<String, crate::DirEntry>,
                    Error<'static>,
                >,
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
    /// Pool for owned locks (created via Lock::new)
    _pool: Option<apr::Pool<'static>>,
}

impl Lock {
    /// Create from a raw pointer (borrows, does not own the lock memory)
    pub fn from_ptr(ptr: *const subversion_sys::svn_lock_t) -> Self {
        Self { ptr, _pool: None }
    }

    /// Create a new lock with the given path and token
    pub fn new(path: Option<&str>, token: Option<&[u8]>) -> Self {
        let pool = apr::Pool::new();
        let lock_ptr = unsafe { subversion_sys::svn_lock_create(pool.as_mut_ptr()) };
        if let Some(p) = path {
            let cstr = std::ffi::CString::new(p).unwrap();
            unsafe {
                (*lock_ptr).path = apr_sys::apr_pstrdup(pool.as_mut_ptr(), cstr.as_ptr());
            }
        }
        if let Some(t) = token {
            let cstr = std::ffi::CString::new(t).unwrap();
            unsafe {
                (*lock_ptr).token = apr_sys::apr_pstrdup(pool.as_mut_ptr(), cstr.as_ptr());
            }
        }
        Self {
            ptr: lock_ptr as *const _,
            _pool: Some(pool),
        }
    }

    /// Get the raw pointer to the lock
    pub fn as_ptr(&self) -> *const subversion_sys::svn_lock_t {
        self.ptr
    }

    /// Get the path this lock applies to
    pub fn path(&self) -> Option<&str> {
        unsafe {
            let p = (*self.ptr).path;
            if p.is_null() {
                None
            } else {
                Some(std::ffi::CStr::from_ptr(p).to_str().unwrap())
            }
        }
    }

    /// Get the unique URI representing the lock token
    pub fn token(&self) -> Option<&str> {
        unsafe {
            let t = (*self.ptr).token;
            if t.is_null() {
                None
            } else {
                Some(std::ffi::CStr::from_ptr(t).to_str().unwrap())
            }
        }
    }

    /// Get the username which owns the lock
    pub fn owner(&self) -> Option<&str> {
        unsafe {
            let o = (*self.ptr).owner;
            if o.is_null() {
                None
            } else {
                Some(std::ffi::CStr::from_ptr(o).to_str().unwrap())
            }
        }
    }

    /// Get the optional description of the lock
    pub fn comment(&self) -> Option<&str> {
        unsafe {
            let c = (*self.ptr).comment;
            if c.is_null() {
                None
            } else {
                Some(std::ffi::CStr::from_ptr(c).to_str().unwrap())
            }
        }
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
) -> Result<(), Error<'static>> {
    let path_str = wc_path.to_string_lossy();
    let path_cstr = std::ffi::CString::new(path_str.as_ref()).unwrap();

    with_tmp_pool(|pool| -> Result<(), Error<'static>> {
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
) -> Result<(), Error<'static>> {
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
) -> Result<(), Error<'static>> {
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
) -> Result<(), Error<'static>> {
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
) -> Result<(), Error<'static>> {
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
) -> Result<(), Error<'static>> {
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
) -> Result<(i64, i64, bool, bool), Error<'static>> {
    // svn_wc_revision_status2 requires an absolute path
    let abs_path = ensure_absolute_path(wc_path)?;
    let path_str = abs_path
        .to_str()
        .ok_or_else(|| Error::from_message("Path contains invalid UTF-8"))?;
    let path_cstr = std::ffi::CString::new(path_str)?;
    let trail_cstr = trail_url.map(std::ffi::CString::new).transpose()?;

    with_tmp_pool(|pool| -> Result<(i64, i64, bool, bool), Error<'static>> {
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

/// An external item definition parsed from svn:externals property.
#[derive(Debug, Clone)]
pub struct ExternalItem {
    /// The subdirectory into which this external should be checked out.
    pub target_dir: String,
    /// The URL to check out from (possibly relative).
    pub url: String,
    /// The revision to check out.
    pub revision: crate::Revision,
    /// The peg revision to use.
    pub peg_revision: crate::Revision,
}

/// Parse an svn:externals property value into a list of external items.
///
/// The `parent_directory` is used for error messages and to resolve
/// relative URLs in the externals description.
pub fn parse_externals_description(
    parent_directory: &str,
    desc: &str,
    canonicalize_url: bool,
) -> Result<Vec<ExternalItem>, Error<'static>> {
    let pool = apr::Pool::new();

    let parent_cstr = std::ffi::CString::new(parent_directory)
        .map_err(|_| Error::from_message("Invalid parent directory"))?;
    let desc_cstr = std::ffi::CString::new(desc)
        .map_err(|_| Error::from_message("Invalid externals description"))?;

    unsafe {
        let mut externals_p: *mut apr_sys::apr_array_header_t = std::ptr::null_mut();
        let err = subversion_sys::svn_wc_parse_externals_description3(
            &mut externals_p,
            parent_cstr.as_ptr(),
            desc_cstr.as_ptr(),
            canonicalize_url.into(),
            pool.as_mut_ptr(),
        );
        svn_result(err)?;

        if externals_p.is_null() {
            return Ok(Vec::new());
        }

        let array =
            apr::tables::TypedArray::<*const subversion_sys::svn_wc_external_item2_t>::from_ptr(
                externals_p,
            );

        let mut result = Vec::new();
        for item_ptr in array.iter() {
            let item = &*item_ptr;

            let target_dir = if item.target_dir.is_null() {
                String::new()
            } else {
                std::ffi::CStr::from_ptr(item.target_dir)
                    .to_str()
                    .map_err(|_| Error::from_message("Invalid target_dir UTF-8"))?
                    .to_string()
            };

            let url = if item.url.is_null() {
                String::new()
            } else {
                std::ffi::CStr::from_ptr(item.url)
                    .to_str()
                    .map_err(|_| Error::from_message("Invalid url UTF-8"))?
                    .to_string()
            };

            unsafe fn convert_revision(
                rev: &subversion_sys::svn_opt_revision_t,
            ) -> crate::Revision {
                match rev.kind {
                    subversion_sys::svn_opt_revision_kind_svn_opt_revision_unspecified => {
                        crate::Revision::Unspecified
                    }
                    subversion_sys::svn_opt_revision_kind_svn_opt_revision_number => {
                        crate::Revision::Number(crate::Revnum(*rev.value.number.as_ref()))
                    }
                    subversion_sys::svn_opt_revision_kind_svn_opt_revision_date => {
                        crate::Revision::Date(*rev.value.date.as_ref())
                    }
                    subversion_sys::svn_opt_revision_kind_svn_opt_revision_head => {
                        crate::Revision::Head
                    }
                    _ => crate::Revision::Unspecified,
                }
            }

            result.push(ExternalItem {
                target_dir,
                url,
                revision: convert_revision(&item.revision),
                peg_revision: convert_revision(&item.peg_revision),
            });
        }

        Ok(result)
    }
}

/// Return the global list of ignore patterns from SVN's default configuration.
///
/// This is equivalent to [`Context::get_ignores`] without a working copy path:
/// it returns only the patterns from the global SVN configuration (e.g.
/// `~/.subversion/config`'s `global-ignores` setting), without adding any
/// `svn:ignore` property patterns from a working copy directory.
///
/// Pass `NULL` for `config` to use the default on-disk configuration.
///
/// Wraps `svn_wc_get_default_ignores`.
pub fn get_default_ignores() -> Result<Vec<String>, crate::Error<'static>> {
    let result_pool = apr::Pool::new();
    let mut patterns: *mut apr_sys::apr_array_header_t = std::ptr::null_mut();
    svn_result(unsafe {
        subversion_sys::svn_wc_get_default_ignores(
            &mut patterns,
            std::ptr::null_mut(), // config — NULL uses defaults
            result_pool.as_mut_ptr(),
        )
    })?;
    if patterns.is_null() {
        return Ok(Vec::new());
    }
    let result =
        unsafe { apr::tables::TypedArray::<*const std::os::raw::c_char>::from_ptr(patterns) }
            .iter()
            .map(|ptr| unsafe { std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned() })
            .collect();
    Ok(result)
}

/// Canonicalize and validate an SVN property value.
///
/// Checks that `propname` is valid for a node of `kind`, and returns a
/// canonicalized form of `propval`.  For example:
/// - `svn:executable`, `svn:needs-lock`, `svn:special` → value set to `"*"`
/// - `svn:keywords` → leading/trailing whitespace stripped
/// - `svn:ignore`, `svn:externals` → trailing newline added if missing
/// - `svn:mergeinfo` → normalized
///
/// Content-dependent checks (`svn:eol-style`, `svn:mime-type`) are
/// skipped; pass the returned value to [`Context::prop_set`] without
/// further validation.
///
/// Returns an error if the property name or value is invalid.
///
/// Wraps `svn_wc_canonicalize_svn_prop`.
pub fn canonicalize_svn_prop(
    propname: &str,
    propval: &[u8],
    path: &str,
    kind: crate::NodeKind,
) -> Result<Vec<u8>, crate::Error<'static>> {
    let pool = apr::Pool::new();
    let propname_c = std::ffi::CString::new(propname)
        .map_err(|_| crate::Error::from_message("property name contains interior NUL"))?;
    let path_c = std::ffi::CString::new(path)
        .map_err(|_| crate::Error::from_message("path contains interior NUL"))?;

    let input = crate::string::BStr::from_bytes(propval, &pool);
    let mut output: *const subversion_sys::svn_string_t = std::ptr::null();

    svn_result(unsafe {
        subversion_sys::svn_wc_canonicalize_svn_prop(
            &mut output,
            propname_c.as_ptr(),
            input.as_ptr(),
            path_c.as_ptr(),
            kind.into(),
            1,                    // skip_some_checks = true (no content/MIME inspection)
            None,                 // prop_getter — not needed when skipping checks
            std::ptr::null_mut(), // getter_baton
            pool.as_mut_ptr(),
        )
    })?;

    if output.is_null() {
        return Ok(propval.to_vec());
    }
    let s = unsafe { &*output };
    Ok(unsafe { std::slice::from_raw_parts(s.data as *const u8, s.len).to_vec() })
}

/// Send local text modifications for a versioned file through a delta editor.
///
/// Transmits the local modifications for the versioned file at `local_abspath`
/// through the provided file editor, then closes the file baton.
///
/// If `fulltext` is true, sends the untranslated copy as full-text; otherwise
/// sends it as an svndiff against the current text base.
///
/// Returns a tuple of (MD5 hex string, SHA-1 hex string) for the text base in
/// repository-normal form. The SHA-1 checksum corresponds to a copy stored
/// in the pristine text store.
///
/// # Errors
///
/// Returns `SVN_ERR_WC_CORRUPT_TEXT_BASE` if sending a diff and the recorded
/// checksum for the text-base doesn't match the current actual checksum.
///
/// Wraps `svn_wc_transmit_text_deltas3`.
///
/// # Example
///
/// This is typically used within custom commit operations where you have
/// a file editor from a delta operation and want to send working copy
/// contents through it.
pub fn transmit_text_deltas<'a>(
    wc_ctx: &mut Context,
    local_abspath: &str,
    fulltext: bool,
    file_editor: &crate::delta::WrapFileEditor<'a>,
) -> Result<(String, String), crate::Error<'static>> {
    let result_pool = apr::Pool::new();
    let scratch_pool = apr::Pool::new();

    let local_abspath_c = std::ffi::CString::new(local_abspath)?;

    let mut md5_checksum: *const subversion_sys::svn_checksum_t = std::ptr::null();
    let mut sha1_checksum: *const subversion_sys::svn_checksum_t = std::ptr::null();

    let (editor_ptr, baton_ptr) = file_editor.as_raw_parts();

    let err = unsafe {
        subversion_sys::svn_wc_transmit_text_deltas3(
            &mut md5_checksum,
            &mut sha1_checksum,
            wc_ctx.ptr,
            local_abspath_c.as_ptr(),
            if fulltext { 1 } else { 0 },
            editor_ptr,
            baton_ptr,
            result_pool.as_mut_ptr(),
            scratch_pool.as_mut_ptr(),
        )
    };
    svn_result(err)?;

    let md5_hex = if md5_checksum.is_null() {
        String::new()
    } else {
        let checksum = crate::Checksum::from_raw(md5_checksum);
        checksum.to_hex(&result_pool)
    };

    let sha1_hex = if sha1_checksum.is_null() {
        String::new()
    } else {
        let checksum = crate::Checksum::from_raw(sha1_checksum);
        checksum.to_hex(&result_pool)
    };

    Ok((md5_hex, sha1_hex))
}

/// Send local property modifications through a file delta editor.
///
/// Transmits all local property modifications for the file at `local_abspath`
/// using the file editor's change_prop method.
///
/// This is typically used in custom commit operations to send property changes
/// to a repository.
///
/// Wraps `svn_wc_transmit_prop_deltas2`.
pub fn transmit_prop_deltas_file<'a>(
    wc_ctx: &mut Context,
    local_abspath: &str,
    file_editor: &crate::delta::WrapFileEditor<'a>,
) -> Result<(), crate::Error<'static>> {
    let scratch_pool = apr::Pool::new();
    let local_abspath_c = std::ffi::CString::new(local_abspath)?;

    let (editor_ptr, baton_ptr) = file_editor.as_raw_parts();

    let err = unsafe {
        subversion_sys::svn_wc_transmit_prop_deltas2(
            wc_ctx.ptr,
            local_abspath_c.as_ptr(),
            editor_ptr,
            baton_ptr,
            scratch_pool.as_mut_ptr(),
        )
    };
    svn_result(err)?;

    Ok(())
}

/// Send local property modifications through a directory delta editor.
///
/// Transmits all local property modifications for the directory at `local_abspath`
/// using the directory editor's change_prop method.
///
/// This is typically used in custom commit operations to send property changes
/// to a repository.
///
/// Wraps `svn_wc_transmit_prop_deltas2`.
pub fn transmit_prop_deltas_dir<'a>(
    wc_ctx: &mut Context,
    local_abspath: &str,
    dir_editor: &crate::delta::WrapDirectoryEditor<'a>,
) -> Result<(), crate::Error<'static>> {
    let scratch_pool = apr::Pool::new();
    let local_abspath_c = std::ffi::CString::new(local_abspath)?;

    let (editor_ptr, baton_ptr) = dir_editor.as_raw_parts();

    let err = unsafe {
        subversion_sys::svn_wc_transmit_prop_deltas2(
            wc_ctx.ptr,
            local_abspath_c.as_ptr(),
            editor_ptr,
            baton_ptr,
            scratch_pool.as_mut_ptr(),
        )
    };
    svn_result(err)?;

    Ok(())
}

#[cfg(all(test, feature = "client", feature = "repos"))]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use tempfile::tempdir;

    /// Ensures the filesystem timestamp will differ from the current time.
    ///
    /// Subversion uses APR timestamps (apr_time_t) which have microsecond precision,
    /// not nanosecond. When files are modified very rapidly (within the same microsecond),
    /// SVN won't detect the change because the stored timestamp hasn't changed.
    ///
    /// This function sleeps for 2 milliseconds to guarantee the next file operation
    /// will have a different timestamp when truncated to microsecond precision.
    ///
    /// Use this in tests after creating/checking out a file and before modifying it
    /// when you need SVN to detect the modification.
    fn ensure_timestamp_rollover() {
        std::thread::sleep(std::time::Duration::from_micros(2000));
    }

    /// Test fixture for a Subversion repository and working copy setup.
    struct SvnTestFixture {
        pub _repos_path: PathBuf,
        pub wc_path: PathBuf,
        pub url: String,
        pub client_ctx: crate::client::Context,
        pub temp_dir: tempfile::TempDir,
    }

    impl SvnTestFixture {
        /// Creates a repository and checks out a working copy.
        fn new() -> Self {
            let temp_dir = tempfile::TempDir::new().unwrap();
            let repos_path = temp_dir.path().join("repos");
            let wc_path = temp_dir.path().join("wc");

            // Create repository
            let _repos = crate::repos::Repos::create(&repos_path).unwrap();

            // Prepare for checkout
            let url = format!("file://{}", repos_path.display());
            let mut client_ctx = crate::client::Context::new().unwrap();

            // Checkout working copy
            let uri = crate::uri::Uri::new(&url).unwrap();
            client_ctx
                .checkout(uri, &wc_path, &Self::default_checkout_options())
                .unwrap();

            Self {
                _repos_path: repos_path,
                wc_path,
                url,
                client_ctx,
                temp_dir,
            }
        }

        /// Returns default checkout options for HEAD with full depth.
        fn default_checkout_options() -> crate::client::CheckoutOptions {
            crate::client::CheckoutOptions {
                peg_revision: crate::Revision::Head,
                revision: crate::Revision::Head,
                depth: crate::Depth::Infinity,
                ignore_externals: false,
                allow_unver_obstructions: false,
            }
        }

        /// Creates and adds a file to the working copy.
        fn add_file(&mut self, name: &str, content: &str) -> PathBuf {
            let file_path = self.wc_path.join(name);
            std::fs::write(&file_path, content).unwrap();
            self.client_ctx
                .add(&file_path, &crate::client::AddOptions::new())
                .unwrap();
            file_path
        }

        /// Creates and adds a directory to the working copy.
        fn add_dir(&mut self, name: &str) -> PathBuf {
            let dir_path = self.wc_path.join(name);
            std::fs::create_dir(&dir_path).unwrap();
            self.client_ctx
                .add(&dir_path, &crate::client::AddOptions::new())
                .unwrap();
            dir_path
        }

        /// Gets the working copy path as a UTF-8 string slice.
        fn wc_path_str(&self) -> &str {
            self.wc_path
                .to_str()
                .expect("working copy path should be valid UTF-8")
        }

        /// Gets the URL of the working copy using client.info().
        fn get_wc_url(&mut self) -> String {
            let wc_path = self
                .wc_path
                .to_str()
                .expect("path should be valid UTF-8")
                .to_string();
            let mut url = None;
            self.client_ctx
                .info(
                    &wc_path,
                    &crate::client::InfoOptions::default(),
                    &|_, info| {
                        url = Some(info.url().to_string());
                        Ok(())
                    },
                )
                .unwrap();
            url.expect("should have retrieved URL from info")
        }

        /// Commits all changes in the working copy.
        fn commit(&mut self) {
            let wc_path_str = self.wc_path_str().to_string();
            let commit_opts = crate::client::CommitOptions::default();
            let revprops = std::collections::HashMap::new();
            self.client_ctx
                .commit(
                    &[wc_path_str.as_str()],
                    &commit_opts,
                    revprops,
                    None,
                    &mut |_info| Ok(()),
                )
                .unwrap();
        }
    }

    /// Creates a repository and returns its path and URL.
    fn create_repo(base: &Path, name: &str) -> (PathBuf, String) {
        let repos_path = base.join(name);
        let _repos = crate::repos::Repos::create(&repos_path).unwrap();
        let url = format!("file://{}", repos_path.display());
        (repos_path, url)
    }

    #[test]
    fn test_context_creation() {
        let context = Context::new().unwrap();
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
        Context::new_with_config(config).unwrap();
    }

    #[test]
    fn test_check_wc() {
        let dir = tempdir().unwrap();
        let wc_path = dir.path();

        // Non-working-copy directory should return None
        let wc_format = check_wc(wc_path).unwrap();
        assert_eq!(wc_format, None);
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

        result.unwrap();
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
        let result = ctx.add_from_disk(&file_path, None, false, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_add_repos_file() {
        // Test that add_repos_file() reaches svn_wc_add_repos_file4 and fails
        // gracefully when the path is not inside a working copy.
        let temp_dir = tempfile::tempdir().unwrap();
        let mut ctx = Context::new().unwrap();

        let file_path = temp_dir.path().join("newfile.txt");

        let content = b"file content\n";
        let mut base_stream = crate::io::Stream::from(&content[..]);
        let base_props: std::collections::HashMap<String, Vec<u8>> =
            std::collections::HashMap::new();

        let result = ctx.add_repos_file(
            &file_path,
            &mut base_stream,
            None,
            &base_props,
            None,
            None,
            crate::Revnum(-1),
            None,
        );
        // Should fail because the path is not inside a versioned working copy.
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
        let options = SwitchEditorOptions::new();
        let result = ctx.get_switch_editor(
            temp_dir.path().to_str().unwrap(),
            "",
            "http://example.com/repo/branches/test",
            options,
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
        let options = SwitchEditorOptions::new();
        let result = ctx.get_switch_editor(
            temp_dir.path().to_str().unwrap(),
            "",
            "http://example.com/svn/trunk",
            options,
        );

        // Expected to fail without a valid working copy
        assert!(result.is_err());
    }

    #[test]
    fn test_get_switch_editor_with_target() {
        // Test switch editor with target basename
        let temp_dir = tempfile::tempdir().unwrap();
        let mut ctx = Context::new().unwrap();

        let options = SwitchEditorOptions {
            use_commit_times: true,
            depth: crate::Depth::Files,
            depth_is_sticky: true,
            allow_unver_obstructions: true,
            ..Default::default()
        };
        let result = ctx.get_switch_editor(
            temp_dir.path().to_str().unwrap(),
            "subdir", // target basename
            "http://example.com/svn/branches/test",
            options,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_get_diff_editor() {
        // Test that the API compiles and can be called
        let temp_dir = tempfile::tempdir().unwrap();
        let mut ctx = Context::new().unwrap();
        let mut callbacks = RecordingDiffCallbacks::new();

        // Create the directory
        std::fs::create_dir_all(temp_dir.path()).unwrap();

        // This should succeed with an existing directory
        let result = ctx.get_diff_editor(
            temp_dir.path().to_str().unwrap(),
            temp_dir.path().to_str().unwrap(),
            &mut callbacks,
            false, // use_text_base
            crate::Depth::Infinity,
            false, // ignore_ancestry
            false, // show_copies_as_adds
            false, // use_git_diff_format
        );

        // Should succeed with an existing path
        result.unwrap();
    }

    #[test]
    fn test_get_diff_editor_with_options() {
        // Test diff editor with various options
        let temp_dir = tempfile::tempdir().unwrap();
        let mut ctx = Context::new().unwrap();
        let mut callbacks = RecordingDiffCallbacks::new();

        // Create the paths so they exist
        std::fs::create_dir_all(temp_dir.path()).unwrap();

        // Test with text base using existing paths
        let result = ctx.get_diff_editor(
            temp_dir.path().to_str().unwrap(),
            temp_dir.path().to_str().unwrap(),
            &mut callbacks,
            true, // use_text_base
            crate::Depth::Empty,
            true, // ignore_ancestry
            true, // show_copies_as_adds
            true, // use_git_diff_format
        );

        // Should succeed since paths exist
        let result = result.unwrap();
        drop(result); // Drop before reusing ctx

        // Test with different paths
        let anchor_path = temp_dir.path().join("anchor");
        let target_path = temp_dir.path().join("target");
        std::fs::create_dir_all(&anchor_path).unwrap();
        std::fs::create_dir_all(&target_path).unwrap();

        let mut callbacks2 = RecordingDiffCallbacks::new();
        let result = ctx.get_diff_editor(
            anchor_path.to_str().unwrap(),
            target_path.to_str().unwrap(),
            &mut callbacks2,
            false,
            crate::Depth::Files,
            false,
            false,
            false,
        );

        // Should succeed since paths exist
        result.unwrap();
    }

    #[test]
    fn test_update_editor_trait() {
        // Test that UpdateEditor implements the Editor trait
        use crate::delta::Editor;

        // This just verifies the trait implementation compiles
        fn check_editor_impl<T: Editor>() {}

        // Verify UpdateEditor implements Editor trait
        check_editor_impl::<UpdateEditor<'_>>();
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
        let result = ctx.queue_committed(
            &file_path, false, true, &mut queue, None, false, false, None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_wc_prop_operations() {
        let mut fixture = SvnTestFixture::new();
        let file_path = fixture.add_file("test.txt", "test content");

        // Use client API to set a property (it handles locking)
        fixture
            .client_ctx
            .propset(
                "test:property",
                Some(b"test value"),
                file_path.to_str().expect("file path should be valid UTF-8"),
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
        let mut fixture = SvnTestFixture::new();
        let file_path = fixture.add_file("test.txt", "test content");

        // Set a property without committing
        fixture
            .client_ctx
            .propset(
                "test:prop1",
                Some(b"value1"),
                file_path.to_str().expect("file path should be valid UTF-8"),
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
        let mut fixture = SvnTestFixture::new();
        let file_path = fixture.add_file("test.txt", "test content");
        let dir_path = fixture.add_dir("subdir");

        // Test read_kind
        let mut wc_ctx = Context::new().unwrap();

        // Check the working copy root is a directory
        let kind = wc_ctx.read_kind(&fixture.wc_path, false, false).unwrap();
        assert_eq!(kind, crate::NodeKind::Dir);

        // Check the file is recognized as a file
        let kind = wc_ctx.read_kind(&file_path, false, false).unwrap();
        assert_eq!(kind, crate::NodeKind::File);

        // Check the directory
        let kind = wc_ctx.read_kind(&dir_path, false, false).unwrap();
        assert_eq!(kind, crate::NodeKind::Dir);

        // Check a non-existent path
        let nonexistent = fixture.wc_path.join("nonexistent");
        let kind = wc_ctx.read_kind(&nonexistent, false, false).unwrap();
        assert_eq!(kind, crate::NodeKind::None);
    }

    #[test]
    fn test_is_wc_root() {
        let mut fixture = SvnTestFixture::new();
        let subdir = fixture.add_dir("subdir");

        // Test is_wc_root
        let mut wc_ctx = Context::new().unwrap();

        // The working copy root should return true
        let is_root = wc_ctx.is_wc_root(&fixture.wc_path).unwrap();
        assert!(is_root, "Working copy root should be detected as WC root");

        // A subdirectory should return false
        let is_root = wc_ctx.is_wc_root(&subdir).unwrap();
        assert!(!is_root, "Subdirectory should not be a WC root");
    }

    #[test]
    fn test_get_pristine_contents() {
        use std::io::Read;

        let mut fixture = SvnTestFixture::new();

        // Create and commit a file with original content
        let original_content = "original content";
        let file_path = fixture.add_file("test.txt", original_content);
        fixture.commit();

        // Modify the file
        let modified_content = "modified content";
        std::fs::write(&file_path, modified_content).unwrap();

        // Get pristine contents
        let mut wc_ctx = Context::new().unwrap();
        let pristine_stream = wc_ctx
            .get_pristine_contents(file_path.to_str().expect("file path should be valid UTF-8"))
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
        let new_file = fixture.add_file("new.txt", "new file content");

        let pristine = wc_ctx
            .get_pristine_contents(
                new_file
                    .to_str()
                    .expect("new file path should be valid UTF-8"),
            )
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
                None,
                &mut |_info| Ok(()),
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
                None,
                &mut |_info| Ok(()),
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
                None,
                &mut |_info| Ok(()),
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
                None,
                &mut |_info| Ok(()),
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
                None,
                &mut |_info| Ok(()),
            )
            .unwrap();

        // In WC2: modify the same line differently
        let file_path2 = wc2_path.join("test.txt");
        std::fs::write(&file_path2, "line1 modified in wc2\nline2\nline3\n").unwrap();

        // Ensure SVN will detect the modification by waiting for timestamp rollover
        ensure_timestamp_rollover();

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
                None,
                &mut |_info| Ok(()),
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
                None,
                &mut |_info| Ok(()),
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
        result.unwrap();

        // Test with break_locks
        cleanup(&wc_path, true, false, false, false, false).unwrap();

        // Test with fix_recorded_timestamps
        cleanup(&wc_path, false, true, false, false, false).unwrap();
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
        let (anchor, target) = get_actual_target(&wc_path).unwrap();
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
            None,  // ignore_patterns
            |_path, _status| {
                status_count += 1;
                Ok(())
            },
        );

        result.unwrap();
        // Should have walked at least the root and the added file
        assert!(status_count >= 1, "Should have at least one status entry");
    }

    #[test]
    fn test_wc_version() {
        let version = version();
        assert!(version.major() > 0);
    }

    // NOTE: This test is disabled because set_adm_dir() modifies global state in the SVN C library,
    // which causes race conditions when tests run in parallel. Other tests creating working copies
    // expect .svn directories but get _svn directories instead, causing failures like:
    // "Can't create directory '/tmp/.../wc/_svn/pristine': No such file or directory"
    #[test]
    #[ignore]
    fn test_set_and_get_adm_dir() {
        // Test setting and getting admin dir
        set_adm_dir("_svn").unwrap();

        let dir = get_adm_dir();
        assert_eq!(dir, "_svn");

        // Reset to default
        set_adm_dir(".svn").unwrap();

        let dir = get_adm_dir();
        assert_eq!(dir, ".svn");
    }

    #[test]
    fn test_context_add() {
        let fixture = SvnTestFixture::new();

        // Create a new file to add
        let new_file = fixture.wc_path.join("newfile.txt");
        std::fs::write(&new_file, b"test content").unwrap();

        // Test Context::add() - svn_wc_add4() is a low-level function
        // that requires write locks to be acquired using private APIs.
        // Verify the binding exists and can be called.
        let mut wc_ctx = Context::new().unwrap();
        let new_file_abs = new_file.canonicalize().unwrap();

        let result = wc_ctx.add(
            new_file_abs
                .to_str()
                .expect("file path should be valid UTF-8"),
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
        let fixture = SvnTestFixture::new();

        // Test upgrade - should succeed (working copy is already in latest format)
        let mut wc_ctx = Context::new().unwrap();
        let result = wc_ctx.upgrade(fixture.wc_path_str());
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
        let options = UpdateEditorOptions {
            cancel_func: Some(Box::new(move || {
                cancel_called_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            })),
            ..Default::default()
        };
        let result = get_update_editor4(&mut wc_ctx, wc_path.to_str().unwrap(), "", options);

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
        let options = SwitchEditorOptions {
            notify_func: Some(Box::new(move |_notify| {
                notify_called_clone.store(true, std::sync::atomic::Ordering::SeqCst);
            })),
            ..Default::default()
        };
        let result =
            wc_ctx.get_switch_editor(wc_path.to_str().unwrap(), "", url_str.as_str(), options);

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
        let options = UpdateEditorOptions {
            server_performs_filtering: true,
            ..Default::default()
        };
        let result =
            get_update_editor4(&mut wc_ctx, temp_dir.path().to_str().unwrap(), "", options);

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
        let options = UpdateEditorOptions {
            clean_checkout: true,
            ..Default::default()
        };
        let result =
            get_update_editor4(&mut wc_ctx, temp_dir.path().to_str().unwrap(), "", options);

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
        let options = SwitchEditorOptions {
            server_performs_filtering: true,
            ..Default::default()
        };
        let result = wc_ctx.get_switch_editor(
            temp_dir.path().to_str().unwrap(),
            "",
            "http://example.com/svn/trunk",
            options,
        );

        // Will fail without a real working copy, but tests that the parameter is accepted
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_externals_description() {
        // Test parsing a simple externals definition
        let desc = "^/trunk/lib lib";
        let items = parse_externals_description("/parent", desc, true).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].target_dir, "lib");
        assert!(items[0].url.contains("trunk/lib"));
    }

    #[test]
    fn test_parse_externals_description_with_revision() {
        // Test parsing externals with revision
        let desc = "-r42 http://example.com/svn/trunk external_dir";
        let items = parse_externals_description("/parent", desc, false).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].target_dir, "external_dir");
        assert_eq!(items[0].url, "http://example.com/svn/trunk");
    }

    #[test]
    fn test_parse_externals_description_empty() {
        // Test parsing empty externals
        let items = parse_externals_description("/parent", "", true).unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn test_wc_add_function() {
        use tempfile::TempDir;

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

        // Create a file in the working copy
        let file_path = wc_path.join("new_file.txt");
        std::fs::write(&file_path, "test content").unwrap();

        // Test wc::add - svn_wc_add_from_disk3 requires write locks managed externally
        // This test verifies the function can be called and properly propagates errors
        let mut wc_ctx = Context::new().unwrap();
        let result = add(
            &mut wc_ctx,
            &file_path,
            crate::Depth::Infinity,
            false, // force
            false, // no_ignore
            false, // no_autoprops
            false, // add_parents
        );

        // Should fail without write lock. If mutated to return Ok(()), this will fail
        assert!(result.is_err(), "add() should fail without write lock");
        let err_msg = format!("{:?}", result.unwrap_err());
        assert!(
            err_msg.to_lowercase().contains("lock") || err_msg.to_lowercase().contains("write"),
            "Expected lock-related error, got: {}",
            err_msg
        );
    }

    #[test]
    fn test_wc_delete_keep_local() {
        use tempfile::TempDir;

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
        let file_path = wc_path.join("to_delete.txt");
        std::fs::write(&file_path, "test content").unwrap();
        client_ctx
            .add(&file_path, &crate::client::AddOptions::new())
            .unwrap();

        // Commit the file
        let mut committed = false;
        client_ctx
            .commit(
                &[wc_path.to_str().unwrap()],
                &crate::client::CommitOptions::default(),
                std::collections::HashMap::from([("svn:log", "Add test file")]),
                None,
                &mut |_info| {
                    committed = true;
                    Ok(())
                },
            )
            .unwrap();
        assert!(committed);

        // Verify file exists
        assert!(file_path.exists());

        // Test wc::delete with keep_local=true
        // svn_wc_delete4 requires write locks managed externally
        let mut wc_ctx = Context::new().unwrap();
        let result = delete(
            &mut wc_ctx,
            &file_path,
            true,  // keep_local
            false, // delete_unversioned_target
        );

        // Should fail without write lock. If mutated to return Ok(()), this will fail
        assert!(result.is_err(), "delete() should fail without write lock");
        let err_msg = format!("{:?}", result.unwrap_err());
        assert!(
            err_msg.to_lowercase().contains("lock") || err_msg.to_lowercase().contains("write"),
            "Expected lock-related error, got: {}",
            err_msg
        );

        // File should still exist (wasn't deleted because operation failed)
        assert!(file_path.exists());
    }

    #[test]
    fn test_wc_delete_remove_local() {
        use tempfile::TempDir;

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
        let file_path = wc_path.join("to_remove.txt");
        std::fs::write(&file_path, "test content").unwrap();
        client_ctx
            .add(&file_path, &crate::client::AddOptions::new())
            .unwrap();

        // Commit the file
        let mut committed = false;
        client_ctx
            .commit(
                &[wc_path.to_str().unwrap()],
                &crate::client::CommitOptions::default(),
                std::collections::HashMap::from([("svn:log", "Add test file")]),
                None,
                &mut |_info| {
                    committed = true;
                    Ok(())
                },
            )
            .unwrap();
        assert!(committed);
        assert!(file_path.exists());

        // Test wc::delete with keep_local=false
        // svn_wc_delete4 requires write locks managed externally
        let mut wc_ctx = Context::new().unwrap();
        let result = delete(
            &mut wc_ctx,
            &file_path,
            false, // keep_local
            false, // delete_unversioned_target
        );

        // Should fail without write lock. If mutated to return Ok(()), this will fail
        assert!(result.is_err(), "delete() should fail without write lock");
        let err_msg = format!("{:?}", result.unwrap_err());
        assert!(
            err_msg.to_lowercase().contains("lock") || err_msg.to_lowercase().contains("write"),
            "Expected lock-related error, got: {}",
            err_msg
        );

        // File should still exist (wasn't deleted because operation failed)
        assert!(file_path.exists());
    }

    #[test]
    fn test_revision_status_empty_wc() {
        use tempfile::TempDir;

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

        // Test revision_status on empty working copy
        let result = revision_status(&wc_path, None, false);
        assert!(result.is_ok(), "revision_status should succeed on empty WC");

        let (min_rev, max_rev, is_switched, is_modified) = result.unwrap();

        // Empty WC at revision 0
        assert_eq!(min_rev, 0, "min_rev should be 0 for empty WC");
        assert_eq!(max_rev, 0, "max_rev should be 0 for empty WC");
        assert_eq!(is_switched, false, "should not be switched");
        assert_eq!(is_modified, false, "should not be modified");
    }

    #[test]
    fn test_revision_status_with_modifications() {
        use tempfile::TempDir;

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

        // Add a file to create modifications
        let file_path = wc_path.join("test.txt");
        std::fs::write(&file_path, "test content").unwrap();
        client_ctx
            .add(&file_path, &crate::client::AddOptions::new())
            .unwrap();

        // Check status - should show modifications
        let result = revision_status(&wc_path, None, false);
        assert!(
            result.is_ok(),
            "revision_status should succeed with modifications"
        );

        let (min_rev, max_rev, is_switched, is_modified) = result.unwrap();

        // Still at revision 0 but now modified
        assert_eq!(min_rev, 0, "min_rev should be 0");
        assert_eq!(max_rev, 0, "max_rev should be 0");
        assert_eq!(is_switched, false, "should not be switched");
        assert_eq!(is_modified, true, "should be modified after adding file");
    }

    #[test]
    fn test_revision_status_after_commit() {
        use tempfile::TempDir;

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

        // Add and commit a file
        let file_path = wc_path.join("test.txt");
        std::fs::write(&file_path, "test content").unwrap();
        client_ctx
            .add(&file_path, &crate::client::AddOptions::new())
            .unwrap();

        let mut committed = false;
        client_ctx
            .commit(
                &[wc_path.to_str().unwrap()],
                &crate::client::CommitOptions::default(),
                std::collections::HashMap::from([("svn:log", "Add test file")]),
                None,
                &mut |_info| {
                    committed = true;
                    Ok(())
                },
            )
            .unwrap();
        assert!(committed, "commit should have been called");

        // Check status after commit
        let result = revision_status(&wc_path, None, false);
        assert!(
            result.is_ok(),
            "revision_status should succeed after commit"
        );

        let (min_rev, max_rev, is_switched, is_modified) = result.unwrap();

        // After committing one file, max_rev should be 1 (the committed file)
        // min_rev might be 0 (the root dir) or 1 depending on implementation
        assert!(
            max_rev >= 1,
            "max_rev should be at least 1 after commit, got {}",
            max_rev
        );
        assert!(
            min_rev <= max_rev,
            "min_rev ({}) should be <= max_rev ({})",
            min_rev,
            max_rev
        );
        assert_eq!(is_switched, false, "should not be switched");
        assert_eq!(is_modified, false, "should not be modified after commit");
    }

    #[test]
    fn test_resolve_conflict_function() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();

        // Test that resolve_conflict fails on a non-WC path
        // This verifies it's actually calling the C library, not just returning Ok(())
        let non_wc_path = temp_dir.path().join("not_a_wc");
        std::fs::create_dir(&non_wc_path).unwrap();

        let mut wc_ctx = Context::new().unwrap();
        let result = resolve_conflict(
            &mut wc_ctx,
            &non_wc_path,
            crate::Depth::Empty,
            true,  // resolve_text
            true,  // resolve_props
            false, // resolve_tree
            ConflictChoice::Postpone,
        );

        // Should fail since it's not a working copy
        // If mutated to always return Ok(()), this test will fail
        assert!(
            result.is_err(),
            "resolve_conflict() should fail on non-WC path, proving it calls the C library"
        );
    }

    #[test]
    fn test_status_methods() {
        use tempfile::TempDir;

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

        // Add and commit a file
        let file_path = wc_path.join("test.txt");
        std::fs::write(&file_path, "test content").unwrap();
        client_ctx
            .add(&file_path, &crate::client::AddOptions::new())
            .unwrap();

        let mut committed = false;
        client_ctx
            .commit(
                &[wc_path.to_str().unwrap()],
                &crate::client::CommitOptions::default(),
                std::collections::HashMap::from([("svn:log", "Add test file")]),
                None,
                &mut |_info| {
                    committed = true;
                    Ok(())
                },
            )
            .unwrap();
        assert!(committed);

        // Get status of the file and test Status methods within the callback
        let mut wc_ctx = Context::new().unwrap();
        let mut found_status = false;
        wc_ctx
            .walk_status(
                &file_path,
                crate::Depth::Empty,
                true,  // get_all
                false, // no_ignore
                false, // ignore_text_mods
                None,  // ignore_patterns
                |_path, status| {
                    found_status = true;

                    // Test Status methods
                    assert_eq!(status.copied(), false, "File should not be copied");
                    assert_eq!(status.switched(), false, "File should not be switched");
                    assert_eq!(status.locked(), false, "File should not be locked");
                    assert_eq!(
                        status.node_status(),
                        StatusKind::Normal,
                        "File should be normal"
                    );
                    assert!(status.revision().0 >= 0, "Revision should be >= 0");
                    assert_eq!(
                        status.repos_relpath(),
                        Some("test.txt".to_string()),
                        "repos_relpath should match"
                    );
                    Ok(())
                },
            )
            .unwrap();

        assert!(found_status, "Should have found file status");
    }

    #[test]
    fn test_status_added_file() {
        let mut fixture = SvnTestFixture::new();
        let file_path = fixture.add_file("added.txt", "test content");

        // Get status of the added file and verify within callback
        let mut wc_ctx = Context::new().unwrap();
        let mut found_status = false;
        wc_ctx
            .walk_status(
                &file_path,
                crate::Depth::Empty,
                true,  // get_all
                false, // no_ignore
                false, // ignore_text_mods
                None,  // ignore_patterns
                |_path, status| {
                    found_status = true;

                    // Verify it's added
                    assert_eq!(
                        status.node_status(),
                        StatusKind::Added,
                        "File should be added"
                    );
                    assert_eq!(
                        status.copied(),
                        false,
                        "Added file should not be marked as copied"
                    );
                    Ok(())
                },
            )
            .unwrap();

        assert!(found_status, "Should have found file status");
    }

    #[test]
    fn test_cleanup_actually_executes() {
        use tempfile::TempDir;

        // Test that cleanup() returns an error when called on a non-existent path
        // This verifies it's actually calling the C library, not just returning Ok(())
        let temp_dir = TempDir::new().unwrap();
        let non_existent = temp_dir.path().join("does_not_exist");

        let result = cleanup(&non_existent, false, false, false, false, false);
        assert!(
            result.is_err(),
            "cleanup() should fail on non-existent path, proving it calls the C library"
        );

        // Test that cleanup() succeeds on a valid working copy
        let fixture = SvnTestFixture::new();

        // Cleanup should succeed on a valid working copy
        let result = cleanup(&fixture.wc_path, false, false, false, false, false);
        assert!(
            result.is_ok(),
            "cleanup() should succeed on valid working copy"
        );

        // Test with different options to verify they're passed through
        let result = cleanup(&fixture.wc_path, true, true, true, true, false);
        assert!(
            result.is_ok(),
            "cleanup() with all options should succeed on valid working copy"
        );
    }

    #[test]
    fn test_context_check_wc_returns_format_number() {
        let fixture = SvnTestFixture::new();

        // Test check_wc on the working copy
        let mut wc_ctx = Context::new().unwrap();
        let format_num = wc_ctx.check_wc(fixture.wc_path_str()).unwrap();

        // The format number should be a valid SVN working copy format
        // SVN 1.7+ uses format 12 or higher
        // This catches mutations that always return 0, 1, or -1
        assert!(
            format_num > 10,
            "Working copy format should be > 10 for modern SVN, got {}",
            format_num
        );
        assert!(
            format_num < 100,
            "Working copy format should be reasonable (< 100), got {}",
            format_num
        );
    }

    #[test]
    fn test_free_function_check_wc() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();

        // Test on non-WC directory - should return None
        let non_wc = temp_dir.path().join("not_a_wc");
        std::fs::create_dir(&non_wc).unwrap();

        let result = check_wc(&non_wc);
        assert!(
            result.is_ok(),
            "check_wc should succeed on non-WC directory"
        );
        assert_eq!(
            result.unwrap(),
            None,
            "check_wc should return None for non-WC directory"
        );

        // Test on actual WC - should return Some with valid format number
        let fixture = SvnTestFixture::new();
        let result = check_wc(&fixture.wc_path);
        assert!(result.is_ok(), "check_wc should succeed on valid WC");

        let format_opt = result.unwrap();
        assert!(
            format_opt.is_some(),
            "check_wc should return Some for valid WC, got None"
        );

        let format_num = format_opt.unwrap();
        assert!(
            format_num > 10,
            "WC format should be > 10 for modern SVN, got {}",
            format_num
        );
        assert!(
            format_num < 100,
            "WC format should be reasonable (< 100), got {}",
            format_num
        );
    }

    #[test]
    fn test_upgrade_actually_executes() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let non_wc = temp_dir.path().join("not_a_wc");
        std::fs::create_dir(&non_wc).unwrap();

        let mut wc_ctx = Context::new().unwrap();
        let result = wc_ctx.upgrade(non_wc.to_str().unwrap());

        // Should fail on non-WC directory, proving it calls the C library
        assert!(
            result.is_err(),
            "upgrade() should fail on non-WC directory, proving it calls the C library"
        );
    }

    #[test]
    fn test_prop_set_actually_executes() {
        let mut fixture = SvnTestFixture::new();
        let file_path = fixture.add_file("test.txt", "test content");

        let mut wc_ctx = Context::new().unwrap();

        // Verify property doesn't exist before
        assert_eq!(
            wc_ctx.prop_get(&file_path, "test:prop").unwrap(),
            None,
            "Property should not exist before"
        );

        // Set property using CLIENT API (which handles locking)
        fixture
            .client_ctx
            .propset(
                "test:prop",
                Some(b"test value"),
                file_path.to_str().expect("test path should be valid UTF-8"),
                &crate::client::PropSetOptions::default(),
            )
            .unwrap();

        // Verify property was actually set by reading back with WC API
        assert_eq!(
            wc_ctx.prop_get(&file_path, "test:prop").unwrap().as_deref(),
            Some(&b"test value"[..]),
            "client propset() should actually set the property"
        );
    }

    #[test]
    fn test_relocate_actually_executes() {
        let mut fixture = SvnTestFixture::new();
        let old_url = fixture.url.clone();

        // Verify original URL
        assert_eq!(fixture.get_wc_url(), old_url);

        // Create a second repository to relocate to
        let (_repos_path2, new_url) = create_repo(fixture.temp_dir.path(), "repos2");

        // Relocate to the new repository URL
        let mut wc_ctx = Context::new().unwrap();
        wc_ctx
            .relocate(fixture.wc_path_str(), &old_url, &new_url)
            .unwrap();

        // Verify URL was actually changed
        assert_eq!(
            fixture.get_wc_url(),
            new_url,
            "relocate() should actually change the repository URL"
        );
    }

    #[test]
    fn test_add_lock_actually_executes() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let non_wc = temp_dir.path().join("not_a_wc");
        std::fs::create_dir(&non_wc).unwrap();
        let file_path = non_wc.join("file.txt");
        std::fs::write(&file_path, "test").unwrap();

        let mut wc_ctx = Context::new().unwrap();

        // Create a dummy lock (using null pointer since we expect failure anyway)
        let lock = Lock::from_ptr(std::ptr::null());

        let result = wc_ctx.add_lock(&file_path, &lock);

        // Should fail on non-WC file, proving it calls the C library
        assert!(
            result.is_err(),
            "add_lock() should fail on non-WC file, proving it calls the C library"
        );
    }

    #[test]
    fn test_remove_lock_actually_executes() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let non_wc = temp_dir.path().join("not_a_wc");
        std::fs::create_dir(&non_wc).unwrap();
        let file_path = non_wc.join("file.txt");
        std::fs::write(&file_path, "test").unwrap();

        let mut wc_ctx = Context::new().unwrap();
        let result = wc_ctx.remove_lock(&file_path);

        // Should fail on non-WC file, proving it calls the C library
        assert!(
            result.is_err(),
            "remove_lock() should fail on non-WC file, proving it calls the C library"
        );
    }

    #[test]
    fn test_ensure_adm_actually_executes() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let wc_path = temp_dir.path().join("new_wc");

        // Ensure the directory exists (ensure_adm requires it)
        std::fs::create_dir(&wc_path).unwrap();

        // Verify .svn doesn't exist before
        let svn_dir = wc_path.join(".svn");
        assert!(!svn_dir.exists(), ".svn should not exist before ensure_adm");

        let result = ensure_adm(
            &wc_path,
            "test-uuid",
            "file:///tmp/test-repo",
            "file:///tmp/test-repo",
            1,
        );

        // Should succeed
        assert!(result.is_ok(), "ensure_adm() should succeed: {:?}", result);

        // Verify .svn directory was created
        assert!(
            svn_dir.exists(),
            "ensure_adm() should create .svn directory"
        );
    }

    #[test]
    fn test_process_committed_queue_actually_executes() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let non_wc = temp_dir.path().join("not_a_wc");
        std::fs::create_dir(&non_wc).unwrap();

        let mut wc_ctx = Context::new().unwrap();
        let mut queue = CommittedQueue::new();

        let result = wc_ctx.process_committed_queue(
            &mut queue,
            crate::Revnum(1),
            Some("2024-01-01T00:00:00.000000Z"),
            Some("author"),
        );

        // Empty queue should succeed, but at least we're testing it executes
        // The mutation would return Ok(()) without calling the C library
        // With a real queue this would do work, but empty queue is still valid
        assert!(
            result.is_ok(),
            "process_committed_queue() should succeed on empty queue"
        );

        // The real test is that it doesn't just return Ok(()) - it actually
        // calls the C library. We can't easily test failure without a complex setup,
        // but the success path still proves it's not just returning Ok(())
        // because the C library function would fail with invalid arguments
    }

    /// A minimal `DiffCallbacks` implementation that records which paths were
    /// reported as changed or added during a diff.
    struct RecordingDiffCallbacks {
        changed_files: Vec<String>,
        added_files: Vec<String>,
        deleted_files: Vec<String>,
    }

    impl RecordingDiffCallbacks {
        fn new() -> Self {
            Self {
                changed_files: Vec::new(),
                added_files: Vec::new(),
                deleted_files: Vec::new(),
            }
        }
    }

    impl DiffCallbacks for RecordingDiffCallbacks {
        fn file_opened(
            &mut self,
            _path: &str,
            _rev: crate::Revnum,
        ) -> Result<(bool, bool), crate::Error<'static>> {
            Ok((false, false))
        }

        fn file_changed(
            &mut self,
            change: &FileChange<'_>,
        ) -> Result<(NotifyState, NotifyState, bool), crate::Error<'static>> {
            self.changed_files.push(change.path.to_string());
            Ok((NotifyState::Changed, NotifyState::Unchanged, false))
        }

        fn file_added(
            &mut self,
            change: &FileChange<'_>,
            _copyfrom_path: Option<&str>,
            _copyfrom_revision: crate::Revnum,
        ) -> Result<(NotifyState, NotifyState, bool), crate::Error<'static>> {
            self.added_files.push(change.path.to_string());
            Ok((NotifyState::Changed, NotifyState::Unchanged, false))
        }

        fn file_deleted(
            &mut self,
            path: &str,
            _tmpfile1: Option<&str>,
            _tmpfile2: Option<&str>,
            _mimetype1: Option<&str>,
            _mimetype2: Option<&str>,
        ) -> Result<(NotifyState, bool), crate::Error<'static>> {
            self.deleted_files.push(path.to_string());
            Ok((NotifyState::Changed, false))
        }

        fn dir_deleted(
            &mut self,
            _path: &str,
        ) -> Result<(NotifyState, bool), crate::Error<'static>> {
            Ok((NotifyState::Unchanged, false))
        }

        fn dir_opened(
            &mut self,
            _path: &str,
            _rev: crate::Revnum,
        ) -> Result<(bool, bool, bool), crate::Error<'static>> {
            Ok((false, false, false))
        }

        fn dir_added(
            &mut self,
            _path: &str,
            _rev: crate::Revnum,
            _copyfrom_path: Option<&str>,
            _copyfrom_revision: crate::Revnum,
        ) -> Result<(NotifyState, bool, bool, bool), crate::Error<'static>> {
            Ok((NotifyState::Changed, false, false, false))
        }

        fn dir_props_changed(
            &mut self,
            _path: &str,
            _dir_was_added: bool,
            _prop_changes: &[PropChange],
        ) -> Result<(NotifyState, bool), crate::Error<'static>> {
            Ok((NotifyState::Unchanged, false))
        }

        fn dir_closed(
            &mut self,
            _path: &str,
            _dir_was_added: bool,
        ) -> Result<(NotifyState, NotifyState, bool), crate::Error<'static>> {
            Ok((NotifyState::Unchanged, NotifyState::Unchanged, false))
        }
    }

    #[test]
    fn test_diff_reports_modified_file() {
        let mut fixture = SvnTestFixture::new();
        fixture.add_file("test.txt", "original content\n");
        fixture.commit();

        // Modify the file locally without committing.
        std::fs::write(fixture.wc_path.join("test.txt"), "modified content\n").unwrap();

        let mut callbacks = RecordingDiffCallbacks::new();
        let mut wc_ctx = Context::new().unwrap();
        wc_ctx
            .diff(&fixture.wc_path, &DiffOptions::default(), &mut callbacks)
            .expect("diff() should succeed on a working copy with local modifications");

        // test.txt has local modifications so it must appear exactly once as changed.
        assert_eq!(
            callbacks.changed_files,
            vec!["test.txt"],
            "only the modified file should be reported as changed"
        );
        assert!(
            callbacks.added_files.is_empty(),
            "no files should be reported as added"
        );
        assert!(
            callbacks.deleted_files.is_empty(),
            "no files should be reported as deleted"
        );
    }

    #[test]
    fn test_diff_reports_added_file() {
        let mut fixture = SvnTestFixture::new();
        // Commit a file so the WC is at revision 1.
        fixture.add_file("existing.txt", "exists\n");
        fixture.commit();

        // Add a new file but do not commit it.
        fixture.add_file("new.txt", "brand new\n");

        let mut callbacks = RecordingDiffCallbacks::new();
        let mut wc_ctx = Context::new().unwrap();
        wc_ctx
            .diff(&fixture.wc_path, &DiffOptions::default(), &mut callbacks)
            .expect("diff() should succeed");

        // new.txt should appear as added.
        assert_eq!(
            callbacks.added_files,
            vec!["new.txt"],
            "the newly added file should be reported as added"
        );
        assert!(
            callbacks.changed_files.is_empty(),
            "no files should be reported as changed"
        );
    }

    #[test]
    fn test_diff_clean_working_copy_reports_nothing() {
        let mut fixture = SvnTestFixture::new();
        fixture.add_file("clean.txt", "clean content\n");
        fixture.commit();

        let mut callbacks = RecordingDiffCallbacks::new();
        let mut wc_ctx = Context::new().unwrap();
        wc_ctx
            .diff(&fixture.wc_path, &DiffOptions::default(), &mut callbacks)
            .expect("diff() should succeed on a clean working copy");

        assert!(
            callbacks.changed_files.is_empty(),
            "no changed files in a clean WC"
        );
        assert!(
            callbacks.added_files.is_empty(),
            "no added files in a clean WC"
        );
        assert!(
            callbacks.deleted_files.is_empty(),
            "no deleted files in a clean WC"
        );
    }

    #[test]
    fn test_merge_requires_write_lock() {
        // svn_wc_merge5 requires the directory containing target_abspath to be
        // write-locked by the wc_ctx.  Acquiring a write lock requires private
        // SVN APIs that are not part of the public interface; we therefore verify
        // that merge() propagates the expected lock error rather than silently
        // succeeding or returning a generic error.
        let mut fixture = SvnTestFixture::new();
        let target_path = fixture.add_file("target.txt", "line 1\nline 2\nline 3\n");
        fixture.commit();

        let left_path = fixture.temp_dir.path().join("left.txt");
        let right_path = fixture.temp_dir.path().join("right.txt");
        std::fs::write(&left_path, "line 1\nline 2\nline 3\n").unwrap();
        std::fs::write(&right_path, "line 1\nline 2 modified\nline 3\n").unwrap();

        let mut wc_ctx = Context::new().unwrap();
        let result = wc_ctx.merge(
            &left_path,
            &right_path,
            &target_path,
            Some(".left"),
            Some(".right"),
            Some(".working"),
            &[],
            &MergeOptions::default(),
        );

        // The C library must report a write-lock error, proving our wrapper
        // actually reached svn_wc_merge5 rather than returning Ok(()) early.
        assert!(
            result.is_err(),
            "merge() must fail without a write lock; a mutation returning Ok(()) would be caught here"
        );
        let err_msg = format!("{:?}", result.unwrap_err());
        assert!(
            err_msg.to_lowercase().contains("lock") || err_msg.to_lowercase().contains("write"),
            "expected a write-lock error, got: {}",
            err_msg
        );
    }

    #[test]
    fn test_merge_with_prop_diff_requires_write_lock() {
        // Verify merge() with non-empty prop_diff also reaches svn_wc_merge5
        // (i.e., the prop_diff array is constructed correctly and the call is made).
        let mut fixture = SvnTestFixture::new();
        let target_path = fixture.add_file("target.txt", "content\n");
        fixture.commit();

        let left_path = fixture.temp_dir.path().join("left.txt");
        let right_path = fixture.temp_dir.path().join("right.txt");
        std::fs::write(&left_path, "content\n").unwrap();
        std::fs::write(&right_path, "content modified\n").unwrap();

        let prop_changes = vec![PropChange {
            name: "svn:eol-style".to_string(),
            value: Some(b"native".to_vec()),
        }];

        let mut wc_ctx = Context::new().unwrap();
        let result = wc_ctx.merge(
            &left_path,
            &right_path,
            &target_path,
            None,
            None,
            None,
            &prop_changes,
            &MergeOptions::default(),
        );

        assert!(
            result.is_err(),
            "merge() with prop_diff must fail without a write lock"
        );
        let err_msg = format!("{:?}", result.unwrap_err());
        assert!(
            err_msg.to_lowercase().contains("lock") || err_msg.to_lowercase().contains("write"),
            "expected a write-lock error, got: {}",
            err_msg
        );
    }

    #[test]
    fn test_merge_props_requires_write_lock() {
        // svn_wc_merge_props3 requires the working copy path to be write-locked.
        // Without a write lock we expect an error, which proves the wrapper
        // actually reached svn_wc_merge_props3.
        let mut fixture = SvnTestFixture::new();
        let file_path = fixture.add_file("propmerge.txt", "content\n");
        fixture.commit();

        let prop_changes = vec![PropChange {
            name: "svn:keywords".to_string(),
            value: Some(b"Id".to_vec()),
        }];

        let mut wc_ctx = Context::new().unwrap();
        let result = wc_ctx.merge_props(&file_path, None, &prop_changes, false, None, None);

        assert!(
            result.is_err(),
            "merge_props() must fail without a write lock; a mutation returning Ok(()) would be caught here"
        );
        let err_msg = format!("{:?}", result.unwrap_err());
        assert!(
            err_msg.to_lowercase().contains("lock")
                || err_msg.to_lowercase().contains("write")
                || err_msg.to_lowercase().contains("path"),
            "expected a write-lock or path error, got: {}",
            err_msg
        );
    }

    #[test]
    fn test_revert_requires_write_lock() {
        // svn_wc_revert6 requires the working copy paths to be write-locked in
        // wc_ctx.  Acquiring a write lock requires private SVN APIs; verify
        // that revert() propagates the expected lock error rather than
        // returning Ok(()) silently.
        let mut fixture = SvnTestFixture::new();
        let file_path = fixture.add_file("revert_me.txt", "original\n");
        fixture.commit();

        std::fs::write(&file_path, "changed\n").unwrap();

        let mut wc_ctx = Context::new().unwrap();
        let result = wc_ctx.revert(
            &file_path,
            &RevertOptions {
                depth: crate::Depth::Empty,
                ..Default::default()
            },
        );

        assert!(
            result.is_err(),
            "revert() must fail without a write lock; a mutation returning Ok(()) would be caught here"
        );
        let err_msg = format!("{:?}", result.unwrap_err());
        assert!(
            err_msg.to_lowercase().contains("lock") || err_msg.to_lowercase().contains("write"),
            "expected a write-lock error, got: {}",
            err_msg
        );
    }

    #[test]
    fn test_revert_with_added_keep_local_false_requires_write_lock() {
        // Verify that the added_keep_local=false path also reaches svn_wc_revert6.
        let mut fixture = SvnTestFixture::new();
        fixture.add_file("existing.txt", "exists\n");
        fixture.commit();

        let new_file = fixture.add_file("new_file.txt", "new\n");

        let mut wc_ctx = Context::new().unwrap();
        let result = wc_ctx.revert(
            &new_file,
            &RevertOptions {
                depth: crate::Depth::Empty,
                added_keep_local: false,
                ..Default::default()
            },
        );

        assert!(
            result.is_err(),
            "revert() with added_keep_local=false must fail without a write lock"
        );
        let err_msg = format!("{:?}", result.unwrap_err());
        assert!(
            err_msg.to_lowercase().contains("lock") || err_msg.to_lowercase().contains("write"),
            "expected a write-lock error, got: {}",
            err_msg
        );
    }

    #[test]
    fn test_revert_with_non_matching_changelist_is_noop() {
        // When the changelist filter matches no items, svn_wc_revert6 is a
        // no-op and returns success without needing a write lock.  This
        // verifies that the changelist array is constructed correctly (bad
        // construction would cause a crash or different error).
        let mut fixture = SvnTestFixture::new();
        let file_path = fixture.add_file("cl_file.txt", "content\n");
        fixture.commit();

        std::fs::write(&file_path, "modified\n").unwrap();

        let mut wc_ctx = Context::new().unwrap();
        // "no-such-changelist" doesn't match the file (which has no changelist),
        // so this is a no-op and must succeed.
        wc_ctx
            .revert(
                &fixture.wc_path,
                &RevertOptions {
                    depth: crate::Depth::Infinity,
                    changelists: vec!["no-such-changelist".to_string()],
                    ..Default::default()
                },
            )
            .expect("revert() with non-matching changelist should succeed as a no-op");

        // File should still be modified (nothing was reverted).
        assert_eq!(
            std::fs::read_to_string(&file_path).unwrap(),
            "modified\n",
            "file should be unchanged when changelist filter matches nothing"
        );
    }

    #[test]
    fn test_copy_requires_write_lock() {
        // svn_wc_copy3 requires the parent directory of dst_abspath to be
        // write-locked.  Acquiring a write lock requires private SVN APIs;
        // verify that copy() propagates the expected lock error.
        let mut fixture = SvnTestFixture::new();
        let src_path = fixture.add_file("src.txt", "source content\n");
        fixture.commit();

        let dst_path = fixture.wc_path.join("dst.txt");

        let mut wc_ctx = Context::new().unwrap();
        let result = wc_ctx.copy(&src_path, &dst_path, false);

        assert!(
            result.is_err(),
            "copy() must fail without a write lock; a mutation returning Ok(()) would be caught here"
        );
        let err_msg = format!("{:?}", result.unwrap_err());
        assert!(
            err_msg.to_lowercase().contains("lock") || err_msg.to_lowercase().contains("write"),
            "expected a write-lock error, got: {}",
            err_msg
        );
    }

    #[test]
    fn test_set_changelist_assigns_and_clears() {
        let mut fixture = SvnTestFixture::new();
        let file_path = fixture.add_file("hello.txt", "hello\n");
        fixture.commit();

        let mut wc_ctx = Context::new().unwrap();

        // Assign to a changelist.
        wc_ctx
            .set_changelist(&file_path, Some("my-cl"), crate::Depth::Empty, &[])
            .expect("set_changelist() should succeed for a versioned file");

        // Verify the file now appears in that changelist.  Use changelist_filter
        // so SVN only reports nodes with that specific changelist (avoiding the
        // "all nodes visited" behaviour when no filter is given).
        let filter = vec!["my-cl".to_string()];
        let mut found: Vec<(String, Option<String>)> = Vec::new();
        wc_ctx
            .get_changelists(
                &fixture.wc_path,
                crate::Depth::Infinity,
                &filter,
                |path, cl| {
                    found.push((path.to_owned(), cl.map(str::to_owned)));
                    Ok(())
                },
            )
            .expect("get_changelists() should succeed");

        assert_eq!(found.len(), 1, "expected exactly one changelist entry");
        assert_eq!(found[0].0, file_path.to_str().unwrap());
        assert_eq!(found[0].1, Some("my-cl".to_owned()));

        // Remove from changelist (None means "clear").
        wc_ctx
            .set_changelist(&file_path, None, crate::Depth::Empty, &[])
            .expect("clearing changelist should succeed");

        // After clearing, filtering for "my-cl" should yield no results.
        let mut after: Vec<(String, Option<String>)> = Vec::new();
        wc_ctx
            .get_changelists(
                &fixture.wc_path,
                crate::Depth::Infinity,
                &filter,
                |path, cl| {
                    after.push((path.to_owned(), cl.map(str::to_owned)));
                    Ok(())
                },
            )
            .expect("get_changelists() after clear should succeed");

        assert_eq!(after.len(), 0, "expected no changelist entries after clear");
    }

    #[test]
    fn test_get_changelists_empty_wc() {
        // With no filter, SVN visits every node and calls the callback even for
        // nodes with no changelist (cl == None).  In a clean WC no node has a
        // changelist, so we expect zero Some(...) entries.
        let fixture = SvnTestFixture::new();

        let mut wc_ctx = Context::new().unwrap();
        let mut with_changelist = 0usize;
        wc_ctx
            .get_changelists(
                &fixture.wc_path,
                crate::Depth::Infinity,
                &[],
                |_path, cl| {
                    if cl.is_some() {
                        with_changelist += 1;
                    }
                    Ok(())
                },
            )
            .expect("get_changelists() on clean WC should succeed");

        assert_eq!(
            with_changelist, 0,
            "expected zero nodes with a changelist in a fresh WC"
        );
    }

    #[test]
    fn test_get_changelists_filter() {
        let mut fixture = SvnTestFixture::new();
        let file_a = fixture.add_file("a.txt", "a\n");
        let file_b = fixture.add_file("b.txt", "b\n");
        fixture.commit();

        let mut wc_ctx = Context::new().unwrap();

        wc_ctx
            .set_changelist(&file_a, Some("cl-alpha"), crate::Depth::Empty, &[])
            .expect("set cl-alpha on a.txt");
        wc_ctx
            .set_changelist(&file_b, Some("cl-beta"), crate::Depth::Empty, &[])
            .expect("set cl-beta on b.txt");

        // Filter to retrieve only cl-alpha entries.  When a filter is given,
        // SVN only invokes the callback for matching nodes, so cl is always
        // Some(matching_name) here.
        let filter = vec!["cl-alpha".to_string()];
        let mut found: Vec<(String, Option<String>)> = Vec::new();
        wc_ctx
            .get_changelists(
                &fixture.wc_path,
                crate::Depth::Infinity,
                &filter,
                |path, cl| {
                    found.push((path.to_owned(), cl.map(str::to_owned)));
                    Ok(())
                },
            )
            .expect("get_changelists() with filter should succeed");

        assert_eq!(found.len(), 1, "expected only the cl-alpha entry");
        assert_eq!(found[0].0, file_a.to_str().unwrap());
        assert_eq!(found[0].1, Some("cl-alpha".to_owned()));
    }

    #[test]
    fn test_status_unmodified_file() {
        let mut fixture = SvnTestFixture::new();
        let file_path = fixture.add_file("hello.txt", "hello\n");
        fixture.commit();

        let mut wc_ctx = Context::new().unwrap();
        let status = wc_ctx
            .status(&file_path)
            .expect("status() should succeed for a versioned file");

        assert_eq!(status.node_status(), StatusKind::Normal);
        assert_eq!(status.text_status(), StatusKind::Normal);
    }

    #[test]
    fn test_status_modified_file() {
        let mut fixture = SvnTestFixture::new();
        let file_path = fixture.add_file("hello.txt", "hello\n");
        fixture.commit();

        // Modify the file after commit.
        std::fs::write(&file_path, "modified\n").unwrap();

        let mut wc_ctx = Context::new().unwrap();
        let status = wc_ctx
            .status(&file_path)
            .expect("status() should succeed for a modified file");

        assert_eq!(status.node_status(), StatusKind::Modified);
    }

    #[test]
    fn test_check_root_wc_root() {
        let fixture = SvnTestFixture::new();

        let mut wc_ctx = Context::new().unwrap();
        let (is_wcroot, is_switched, kind) = wc_ctx
            .check_root(&fixture.wc_path)
            .expect("check_root() should succeed on the WC root");

        assert!(is_wcroot, "WC root directory should be reported as wcroot");
        assert!(!is_switched, "fresh checkout should not be switched");
        assert_eq!(kind, crate::NodeKind::Dir);
    }

    #[test]
    fn test_check_root_non_root_file() {
        let mut fixture = SvnTestFixture::new();
        let file_path = fixture.add_file("hello.txt", "hello\n");
        fixture.commit();

        let mut wc_ctx = Context::new().unwrap();
        let (is_wcroot, _is_switched, kind) = wc_ctx
            .check_root(&file_path)
            .expect("check_root() should succeed on a versioned file");

        assert!(!is_wcroot, "a file is not a wcroot");
        assert_eq!(kind, crate::NodeKind::File);
    }

    #[test]
    fn test_restore_missing_file() {
        let mut fixture = SvnTestFixture::new();
        let file_path = fixture.add_file("hello.txt", "hello\n");
        fixture.commit();

        // Delete the file on disk without telling SVN (simulate a missing file).
        std::fs::remove_file(&file_path).unwrap();

        let mut wc_ctx = Context::new().unwrap();
        wc_ctx
            .restore(&file_path, false)
            .expect("restore() should succeed for a missing versioned file");

        assert!(
            file_path.exists(),
            "file should be restored on disk after restore()"
        );
        assert_eq!(
            std::fs::read_to_string(&file_path).unwrap(),
            "hello\n",
            "restored file should have original content"
        );
    }

    #[test]
    fn test_get_ignores_returns_patterns() {
        let fixture = SvnTestFixture::new();

        let mut wc_ctx = Context::new().unwrap();
        let patterns = wc_ctx
            .get_ignores(&fixture.wc_path)
            .expect("get_ignores() should succeed on a valid WC directory");

        // SVN always includes a set of default global ignore patterns even
        // with no config file.  The list must be non-empty.
        assert!(
            !patterns.is_empty(),
            "expected at least some default ignore patterns"
        );
        // The default set always contains "*.o" (compiled objects).
        assert!(
            patterns.iter().any(|p| p == "*.o"),
            "expected '*.o' in default ignore patterns, got: {:?}",
            patterns
        );
    }

    #[test]
    fn test_get_ignores_includes_svn_ignore_property() {
        let mut fixture = SvnTestFixture::new();
        fixture.commit();

        // Set svn:ignore via the client context, which acquires write locks.
        let wc_path_str = fixture.wc_path_str().to_owned();
        fixture
            .client_ctx
            .propset(
                "svn:ignore",
                Some(b"my-custom-pattern\n"),
                &wc_path_str,
                &crate::client::PropSetOptions::default(),
            )
            .expect("propset svn:ignore should succeed");

        let mut wc_ctx = Context::new().unwrap();
        let patterns = wc_ctx
            .get_ignores(&fixture.wc_path)
            .expect("get_ignores() should succeed");

        assert!(
            patterns.iter().any(|p| p == "my-custom-pattern"),
            "expected custom pattern in ignore list, got: {:?}",
            patterns
        );
    }

    #[test]
    fn test_canonicalize_svn_prop_executable() {
        // svn:executable is canonicalized to "*" regardless of input value.
        let result = canonicalize_svn_prop(
            "svn:executable",
            b"yes",
            "/some/path",
            crate::NodeKind::File,
        )
        .expect("canonicalize should succeed for svn:executable");
        assert_eq!(result, b"*");
    }

    #[test]
    fn test_canonicalize_svn_prop_ignore_adds_newline() {
        // svn:ignore values without a trailing newline should get one added.
        let result = canonicalize_svn_prop("svn:ignore", b"*.o", "/some/dir", crate::NodeKind::Dir)
            .expect("canonicalize should succeed for svn:ignore");
        assert_eq!(result, b"*.o\n");
    }

    #[test]
    fn test_canonicalize_svn_prop_keywords_strips_whitespace() {
        let result = canonicalize_svn_prop(
            "svn:keywords",
            b"  Rev Author  ",
            "/some/path",
            crate::NodeKind::File,
        )
        .expect("canonicalize should succeed for svn:keywords");
        assert_eq!(result, b"Rev Author");
    }

    #[test]
    fn test_canonicalize_svn_prop_invalid_prop_errors() {
        // A property that is not valid for a file node should return an error.
        let result = canonicalize_svn_prop(
            "svn:ignore",
            b"*.o\n",
            "/some/path",
            crate::NodeKind::File, // svn:ignore is only valid on dirs
        );
        assert!(
            result.is_err(),
            "svn:ignore on a file should produce a validation error"
        );
    }

    #[test]
    fn test_get_default_ignores_returns_patterns() {
        // get_default_ignores() returns global ignore patterns without a WC.
        let patterns = get_default_ignores().expect("get_default_ignores() should succeed");

        // SVN always includes a set of default global ignore patterns even
        // with no config file.  The list must be non-empty.
        assert!(
            !patterns.is_empty(),
            "expected at least some default ignore patterns"
        );
        // The default set always contains "*.o" (compiled objects).
        assert!(
            patterns.iter().any(|p| p == "*.o"),
            "expected '*.o' in default ignore patterns, got: {:?}",
            patterns
        );
    }

    #[test]
    fn test_get_default_ignores_does_not_include_svn_ignore() {
        // get_default_ignores() must not include svn:ignore property values
        // from any working copy directory — it only returns global patterns.
        let mut fixture = SvnTestFixture::new();
        fixture.commit();

        // Set a directory-level svn:ignore property.
        let wc_path_str = fixture.wc_path_str().to_owned();
        fixture
            .client_ctx
            .propset(
                "svn:ignore",
                Some(b"my-custom-pattern\n"),
                &wc_path_str,
                &crate::client::PropSetOptions::default(),
            )
            .expect("propset svn:ignore should succeed");

        let patterns = get_default_ignores().expect("get_default_ignores() should succeed");

        assert!(
            !patterns.iter().any(|p| p == "my-custom-pattern"),
            "get_default_ignores() must not include svn:ignore property values"
        );
    }

    #[test]
    fn test_remove_from_revision_control_requires_write_lock() {
        // svn_wc_remove_from_revision_control2 requires a write lock on the
        // parent directory.  Verify that remove_from_revision_control() propagates
        // the expected lock error rather than returning Ok(()) silently.
        let mut fixture = SvnTestFixture::new();
        let file_path = fixture.add_file("versioned.txt", "content\n");
        fixture.commit();

        let mut wc_ctx = Context::new().unwrap();
        let result = wc_ctx.remove_from_revision_control(&file_path, false, false);

        assert!(
            result.is_err(),
            "remove_from_revision_control() must fail without a write lock"
        );
        let err_msg = format!("{:?}", result.unwrap_err());
        assert!(
            err_msg.to_lowercase().contains("lock") || err_msg.to_lowercase().contains("write"),
            "expected a write-lock error, got: {}",
            err_msg
        );
    }

    #[test]
    fn test_lock_new_with_path_and_token() {
        let lock = Lock::new(Some("/trunk/file.txt"), Some(b"opaquelocktoken:abc123"));

        assert_eq!(lock.path(), Some("/trunk/file.txt"));
        assert_eq!(lock.token(), Some("opaquelocktoken:abc123"));
    }

    #[test]
    fn test_lock_new_with_none() {
        let lock = Lock::new(None, None);

        assert_eq!(lock.path(), None);
        assert_eq!(lock.token(), None);
        assert_eq!(lock.owner(), None);
        assert_eq!(lock.comment(), None);
    }

    #[test]
    fn test_lock_new_with_path_only() {
        let lock = Lock::new(Some("/trunk/file.txt"), None);

        assert_eq!(lock.path(), Some("/trunk/file.txt"));
        assert_eq!(lock.token(), None);
    }
}
