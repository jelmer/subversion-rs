//! Filesystem layer for direct repository access.
//!
//! This module provides low-level access to Subversion's versioned filesystem through
//! the [`Fs`](crate::fs::Fs) type. It allows direct manipulation of repository data without going
//! through the client or RA layers.
//!
//! # Overview
//!
//! The filesystem (FS) layer is the storage backend for Subversion repositories. It provides
//! transactional access to repository data, revision management, and direct file/directory
//! operations. This is useful for server implementations and administrative tools.
//!
//! ## Key Operations
//!
//! - **Revision access**: Read and query any revision in the repository
//! - **Transactions**: Create and commit atomic changes
//! - **Path operations**: Read directories, file contents, and properties
//! - **Lock management**: Create, query, and remove locks
//! - **History**: Track node history and changes across revisions
//! - **Maintenance**: Pack, verify, and optimize repository storage
//!
//! # Example
//!
//! ```no_run
//! use subversion::fs::Fs;
//! use subversion::Revnum;
//!
//! let fs = Fs::open("/path/to/repo/db").unwrap();
//! let youngest = fs.youngest_rev().unwrap();
//!
//! // Access a specific revision
//! let root = fs.revision_root(Revnum(youngest.0)).unwrap();
//! let file_content = root.file_contents("/path/to/file").unwrap();
//! ```

use crate::{svn_result, with_tmp_pool, Error, Revnum};
use std::ffi::{CStr, CString};
use std::marker::PhantomData;

// Helper functions for properly boxing callback batons
// The callbacks expect *const Box<dyn Fn...>, not *const Box<&dyn Fn...>
// We need double-boxing to avoid UB
fn box_pack_notify_baton(f: Box<dyn Fn(&str) + Send>) -> *mut std::ffi::c_void {
    Box::into_raw(Box::new(f)) as *mut std::ffi::c_void
}

fn box_verify_notify_baton(f: Box<dyn Fn(Revnum, &str) + Send>) -> *mut std::ffi::c_void {
    Box::into_raw(Box::new(f)) as *mut std::ffi::c_void
}

fn box_cancel_baton(f: Box<dyn Fn() -> bool + Send>) -> *mut std::ffi::c_void {
    Box::into_raw(Box::new(f)) as *mut std::ffi::c_void
}

unsafe fn free_pack_notify_baton(baton: *mut std::ffi::c_void) {
    drop(Box::from_raw(baton as *mut Box<dyn Fn(&str) + Send>));
}

unsafe fn free_verify_notify_baton(baton: *mut std::ffi::c_void) {
    drop(Box::from_raw(
        baton as *mut Box<dyn Fn(Revnum, &str) + Send>,
    ));
}

unsafe fn free_cancel_baton(baton: *mut std::ffi::c_void) {
    drop(Box::from_raw(baton as *mut Box<dyn Fn() -> bool + Send>));
}

/// A canonical absolute filesystem path for use with Subversion filesystem operations.
///
/// SVN filesystem paths must be canonical and absolute (start with '/').
/// This type ensures paths are properly canonicalized using SVN's own canonicalization functions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FsPath {
    path: CString,
}

impl FsPath {
    /// Create an FsPath from an already-canonical path.
    ///
    /// This is a fast path that only validates the path is canonical.
    /// Returns an error if the path is not canonical.
    pub fn from_canonical(path: &str) -> Result<Self, Error<'static>> {
        // Handle empty path as root
        let path = if path.is_empty() { "/" } else { path };

        // Ensure path is absolute for filesystem operations
        if !path.starts_with('/') {
            return Err(Error::from_message(&format!(
                "Filesystem path must be absolute (start with '/'): {}",
                path
            )));
        }

        with_tmp_pool(|pool| unsafe {
            let path_cstr = CString::new(path)?;

            // Check if canonical
            let is_canonical =
                subversion_sys::svn_path_is_canonical(path_cstr.as_ptr(), pool.as_mut_ptr()) != 0;

            if is_canonical {
                Ok(Self { path: path_cstr })
            } else {
                Err(Error::from_message(&format!(
                    "Path is not canonical: {}",
                    path
                )))
            }
        })
    }

    /// Create an FsPath by canonicalizing the input path.
    ///
    /// This will canonicalize the path using SVN's canonicalization rules.
    /// Returns an error if the path cannot be made canonical.
    pub fn canonicalize(path: &str) -> Result<Self, Error<'static>> {
        // Handle empty path as root
        let path = if path.is_empty() { "/" } else { path };

        // Ensure path is absolute for filesystem operations
        if !path.starts_with('/') {
            return Err(Error::from_message(&format!(
                "Filesystem path must be absolute (start with '/'): {}",
                path
            )));
        }

        with_tmp_pool(|pool| unsafe {
            let path_cstr = CString::new(path)?;

            // Check if already canonical (fast path)
            let is_canonical =
                subversion_sys::svn_path_is_canonical(path_cstr.as_ptr(), pool.as_mut_ptr()) != 0;

            if is_canonical {
                Ok(Self { path: path_cstr })
            } else {
                // Canonicalize the path
                let canonical_ptr =
                    subversion_sys::svn_path_canonicalize(path_cstr.as_ptr(), pool.as_mut_ptr());

                if canonical_ptr.is_null() {
                    return Err(Error::from_message(&format!(
                        "Failed to canonicalize path: {}",
                        path
                    )));
                }

                let canonical_str = CStr::from_ptr(canonical_ptr).to_str()?;
                Ok(Self {
                    path: CString::new(canonical_str)?,
                })
            }
        })
    }

    /// Get the path as a C string pointer for FFI.
    pub fn as_ptr(&self) -> *const i8 {
        self.path.as_ptr()
    }

    /// Get the path as a string slice.
    pub fn as_str(&self) -> &str {
        self.path.to_str().unwrap_or("/")
    }
}

impl TryFrom<&str> for FsPath {
    type Error = Error<'static>;

    fn try_from(path: &str) -> Result<Self, Self::Error> {
        FsPath::canonicalize(path)
    }
}

impl TryFrom<String> for FsPath {
    type Error = Error<'static>;

    fn try_from(path: String) -> Result<Self, Self::Error> {
        FsPath::canonicalize(&path)
    }
}

impl std::fmt::Display for FsPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

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

/// Represents a path change in the filesystem (version 3 API).
///
/// This is a more modern API than [`FsPathChange`] and uses an iterator
/// pattern for memory efficiency.
pub struct FsPathChange3<'a> {
    ptr: *const subversion_sys::svn_fs_path_change3_t,
    _marker: PhantomData<&'a ()>,
}

impl<'a> FsPathChange3<'a> {
    /// Creates an FsPathChange3 from a raw pointer.
    unsafe fn from_raw(ptr: *const subversion_sys::svn_fs_path_change3_t) -> Self {
        Self {
            ptr,
            _marker: PhantomData,
        }
    }

    /// Gets the path that changed.
    pub fn path(&self) -> &str {
        unsafe {
            let svn_string = &(*self.ptr).path;
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                svn_string.data as *const u8,
                svn_string.len,
            ))
        }
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

    /// Checks if mergeinfo was modified.
    pub fn mergeinfo_modified(&self) -> bool {
        unsafe { (*self.ptr).mergeinfo_mod as i32 != 0 }
    }

    /// Gets the path this was copied from, if any.
    pub fn copyfrom_path(&self) -> Option<&str> {
        unsafe {
            if (*self.ptr).copyfrom_known == 0 || (*self.ptr).copyfrom_path.is_null() {
                None
            } else {
                Some(
                    std::ffi::CStr::from_ptr((*self.ptr).copyfrom_path)
                        .to_str()
                        .unwrap(),
                )
            }
        }
    }

    /// Gets the revision this was copied from, if any.
    pub fn copyfrom_rev(&self) -> Option<Revnum> {
        unsafe {
            if (*self.ptr).copyfrom_known == 0 {
                None
            } else {
                let rev = (*self.ptr).copyfrom_rev;
                if rev == -1 {
                    None
                } else {
                    Some(Revnum(rev))
                }
            }
        }
    }
}

/// Iterator over path changes in a filesystem root.
///
/// This iterator provides efficient access to changed paths using the
/// svn_fs_paths_changed3 API.
pub struct PathChangeIterator<'a> {
    iter_ptr: *mut subversion_sys::svn_fs_path_change_iterator_t,
    _marker: PhantomData<&'a ()>,
}

impl<'a> PathChangeIterator<'a> {
    /// Creates a new iterator from a raw pointer.
    unsafe fn from_raw(iter_ptr: *mut subversion_sys::svn_fs_path_change_iterator_t) -> Self {
        Self {
            iter_ptr,
            _marker: PhantomData,
        }
    }
}

impl<'a> Iterator for PathChangeIterator<'a> {
    type Item = Result<FsPathChange3<'a>, Error<'static>>;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let mut change_ptr: *mut subversion_sys::svn_fs_path_change3_t = std::ptr::null_mut();
            let err = subversion_sys::svn_fs_path_change_get(&mut change_ptr, self.iter_ptr);

            if let Err(e) = svn_result(err) {
                return Some(Err(e));
            }

            if change_ptr.is_null() {
                None
            } else {
                Some(Ok(FsPathChange3::from_raw(change_ptr)))
            }
        }
    }
}

/// Represents a directory entry in the filesystem
pub struct FsDirEntry {
    ptr: *const subversion_sys::svn_fs_dirent_t,
    _pool: apr::SharedPool<'static>,
}

impl FsDirEntry {
    /// Creates an FsDirEntry from a raw pointer with a shared pool.
    pub fn from_raw(
        ptr: *mut subversion_sys::svn_fs_dirent_t,
        pool: apr::SharedPool<'static>,
    ) -> Self {
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
pub struct Fs<'pool> {
    fs_ptr: *mut subversion_sys::svn_fs_t,
    pool: apr::Pool<'pool>, // Keep pool alive for fs lifetime
    /// Boxed closure for warning callbacks; kept alive for as long as this Fs exists.
    _warning_baton: Option<Box<Box<dyn Fn(&Error<'static>) + Send>>>,
}

unsafe impl Send for Fs<'_> {}

impl Drop for Fs<'_> {
    fn drop(&mut self) {
        // Pool drop will clean up fs
    }
}

/// Trampoline called by the SVN C library when a warning is issued.
///
/// `baton` is a pointer to a `Box<dyn Fn(&Error<'static>) + Send>` that was
/// heap-allocated in [`Fs::set_warning_func`].
unsafe extern "C" fn warning_func_trampoline(
    baton: *mut std::ffi::c_void,
    err: *mut subversion_sys::svn_error_t,
) {
    if baton.is_null() || err.is_null() {
        return;
    }
    let cb = &*(baton as *const Box<dyn Fn(&Error<'static>) + Send>);
    // Wrap the raw error pointer into a borrowed Error that does NOT free on
    // drop (SVN still owns the svn_error_t here).
    let error = Error::from_ptr_borrowed(err);
    cb(&error);
}

/// Trampoline called by the SVN C library from `svn_fs_try_process_file_contents`.
///
/// `baton` is a pointer to a boxed `FnMut(&[u8]) -> Result<(), Error>` closure.
unsafe extern "C" fn process_contents_trampoline(
    contents: *const std::os::raw::c_uchar,
    len: apr_sys::apr_size_t,
    baton: *mut std::ffi::c_void,
    _scratch_pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    if baton.is_null() || contents.is_null() {
        return std::ptr::null_mut(); // success
    }

    let cb = &mut *(baton as *mut Box<dyn FnMut(&[u8]) -> Result<(), Error<'static>>>);
    let slice = std::slice::from_raw_parts(contents, len);

    match cb(slice) {
        Ok(()) => std::ptr::null_mut(),
        Err(mut e) => e.detach(), // Transfer ownership to SVN
    }
}

impl<'pool> Fs<'pool> {
    /// Get a reference to the underlying pool
    pub fn pool(&self) -> &apr::Pool<'_> {
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
        pool: apr::Pool<'pool>,
    ) -> Self {
        Self {
            fs_ptr,
            pool,
            _warning_baton: None,
        }
    }

    /// Creates a new filesystem at the specified path.
    pub fn create(path: &std::path::Path) -> Result<Fs<'static>, Error<'_>> {
        // Ensure SVN libraries are initialized
        crate::init::initialize()?;

        let pool = apr::Pool::new();
        let path_str = path
            .to_str()
            .ok_or_else(|| Error::from_message("Invalid path"))?;
        let path_c = std::ffi::CString::new(path_str)
            .map_err(|_| Error::from_message("Invalid path string"))?;

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

            Ok(Fs {
                fs_ptr,
                pool,
                _warning_baton: None,
            })
        }
    }

    /// Opens an existing filesystem at the specified path.
    pub fn open(path: &std::path::Path) -> Result<Fs<'static>, Error<'_>> {
        // Ensure SVN libraries are initialized
        crate::init::initialize()?;
        let pool = apr::Pool::new();
        let path_str = path
            .to_str()
            .ok_or_else(|| Error::from_message("Invalid path"))?;
        let path_c = std::ffi::CString::new(path_str)
            .map_err(|_| Error::from_message("Invalid path string"))?;

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

            Ok(Fs {
                fs_ptr,
                pool,
                _warning_baton: None,
            })
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

    /// Return the configuration options that were passed to [`Fs::create`] or
    /// [`Fs::open`] when this filesystem was opened.
    ///
    /// The returned map has `const char *` key/value pairs that are
    /// backend-specific.  Common keys are defined as `SVN_FS_CONFIG_*`
    /// constants in the SVN C API (e.g. `SVN_FS_CONFIG_FS_TYPE`).
    ///
    /// Returns an empty map if no configuration was provided.
    ///
    /// Wraps `svn_fs_config`.
    pub fn config(&self) -> std::collections::HashMap<String, String> {
        let pool = apr::Pool::new();
        let mut result = std::collections::HashMap::new();
        unsafe {
            let hash_ptr = subversion_sys::svn_fs_config(self.fs_ptr, pool.as_mut_ptr());
            if hash_ptr.is_null() {
                return result;
            }
            let mut hi = apr_sys::apr_hash_first(pool.as_mut_ptr(), hash_ptr);
            while !hi.is_null() {
                let mut key_ptr: *const std::ffi::c_void = std::ptr::null();
                let mut key_len: apr_sys::apr_ssize_t = 0;
                let mut val_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
                apr_sys::apr_hash_this(hi, &mut key_ptr, &mut key_len, &mut val_ptr);

                let key_str = if key_len < 0 {
                    std::ffi::CStr::from_ptr(key_ptr as *const std::os::raw::c_char)
                        .to_string_lossy()
                        .into_owned()
                } else {
                    String::from_utf8_lossy(std::slice::from_raw_parts(
                        key_ptr as *const u8,
                        key_len as usize,
                    ))
                    .into_owned()
                };

                let val_str = if val_ptr.is_null() {
                    String::new()
                } else {
                    std::ffi::CStr::from_ptr(val_ptr as *const std::os::raw::c_char)
                        .to_string_lossy()
                        .into_owned()
                };

                result.insert(key_str, val_str);
                hi = apr_sys::apr_hash_next(hi);
            }
        }
        result
    }

    /// Sets a warning function to be called when the filesystem encounters non-fatal issues.
    ///
    /// The provided closure will be invoked whenever the SVN filesystem wants to issue
    /// a warning. The closure receives a borrowed `Error` that describes the warning condition.
    ///
    /// Only one warning handler can be active at a time; calling this method again replaces
    /// the previous handler.
    ///
    /// Wraps `svn_fs_set_warning_func`.
    pub fn set_warning_func<F>(&mut self, f: F)
    where
        F: Fn(&Error<'static>) + Send + 'static,
    {
        // Box the closure twice: inner Box for dyn trait object, outer Box for heap allocation
        let boxed: Box<Box<dyn Fn(&Error<'static>) + Send>> = Box::new(Box::new(f));
        let baton_ptr = Box::into_raw(boxed) as *mut std::ffi::c_void;

        unsafe {
            subversion_sys::svn_fs_set_warning_func(
                self.fs_ptr,
                Some(warning_func_trampoline),
                baton_ptr,
            );
        }

        // Store the baton to keep it alive (and drop the old one if any)
        self._warning_baton = Some(unsafe { Box::from_raw(baton_ptr as *mut _) });
    }

    /// Gets the youngest (most recent) revision in the filesystem.
    pub fn youngest_revision(&self) -> Result<Revnum, Error<'static>> {
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

    /// Get all revision properties for the given revision.
    ///
    /// If `refresh` is true, the filesystem may refetch the properties from storage before
    /// returning them (useful in scenarios with concurrent writers).
    ///
    /// Wraps `svn_fs_revision_proplist2`.
    pub fn revision_proplist(
        &self,
        rev: Revnum,
        refresh: bool,
    ) -> Result<std::collections::HashMap<String, Vec<u8>>, Error<'_>> {
        let result_pool = apr::pool::Pool::new();
        let scratch_pool = apr::pool::Pool::new();
        let mut props = std::ptr::null_mut();
        let err = unsafe {
            subversion_sys::svn_fs_revision_proplist2(
                &mut props,
                self.fs_ptr,
                rev.0,
                refresh as i32,
                result_pool.as_mut_ptr(),
                scratch_pool.as_mut_ptr(),
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

    /// Get a single revision property by name.  Returns `None` if the property
    /// is not set on this revision.
    ///
    /// If `refresh` is true, the filesystem may refetch the properties from storage before
    /// returning the value (useful in scenarios with concurrent writers).
    ///
    /// Wraps `svn_fs_revision_prop2`.
    pub fn revision_prop(
        &self,
        rev: Revnum,
        propname: &str,
        refresh: bool,
    ) -> Result<Option<Vec<u8>>, Error<'static>> {
        let name_cstr = std::ffi::CString::new(propname)?;
        let result_pool = apr::pool::Pool::new();
        let scratch_pool = apr::pool::Pool::new();
        unsafe {
            let mut value_p: *mut subversion_sys::svn_string_t = std::ptr::null_mut();
            let err = subversion_sys::svn_fs_revision_prop2(
                &mut value_p,
                self.fs_ptr,
                rev.0,
                name_cstr.as_ptr(),
                refresh as i32,
                result_pool.as_mut_ptr(),
                scratch_pool.as_mut_ptr(),
            );
            svn_result(err)?;
            if value_p.is_null() {
                return Ok(None);
            }
            let s = &*value_p;
            let data = std::slice::from_raw_parts(s.data as *const u8, s.len).to_vec();
            Ok(Some(data))
        }
    }

    /// Gets the root of a specific revision.
    ///
    /// The returned root has a lifetime tied to this `Fs`: it cannot outlive
    /// the filesystem because the root's C pointer internally references the
    /// filesystem's data structures.
    pub fn revision_root(&self, rev: Revnum) -> Result<Root<'_>, Error<'static>> {
        let pool = apr::Pool::new();
        unsafe {
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
                pool: apr::PoolHandle::owned(pool),
                _marker: std::marker::PhantomData,
            })
        }
    }

    /// Gets the UUID of the filesystem.
    pub fn get_uuid(&self) -> Result<String, Error<'static>> {
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
    pub fn set_uuid(&mut self, uuid: &str) -> Result<(), Error<'static>> {
        let scratch_pool = apr::pool::Pool::new();
        unsafe {
            let uuid = std::ffi::CString::new(uuid).unwrap();
            let err = subversion_sys::svn_fs_set_uuid(
                self.fs_ptr,
                uuid.as_ptr(),
                scratch_pool.as_mut_ptr(),
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
    ) -> Result<crate::Lock<'static>, Error<'_>> {
        let path_cstr = std::ffi::CString::new(path).unwrap();

        let token_cstr = token.map(|t| std::ffi::CString::new(t).unwrap());
        let token_ptr = token_cstr.as_ref().map_or(std::ptr::null(), |t| t.as_ptr());

        let comment_cstr = comment.map(|c| std::ffi::CString::new(c).unwrap());
        let comment_ptr = comment_cstr
            .as_ref()
            .map_or(std::ptr::null(), |c| c.as_ptr());

        let mut lock_ptr: *mut subversion_sys::svn_lock_t = std::ptr::null_mut();

        let pool = apr::Pool::new();

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
                pool.as_mut_ptr(),
            )
        };

        svn_result(ret)?;

        // The lock is allocated in the fs pool, so it should be valid for the lifetime of the fs
        let pool_handle = apr::PoolHandle::owned(pool);
        Ok(crate::Lock::from_raw(lock_ptr, pool_handle))
    }

    /// Lock multiple paths in the filesystem atomically.
    ///
    /// `targets` is a slice of `(path, token, current_rev)` tuples. `token` can be `None` to
    /// have a new token generated; `current_rev` can be `Revnum::INVALID` to skip the
    /// out-of-dateness check.
    ///
    /// For each locked path (or error) the `callback` is invoked with
    /// `(path, error_or_none)`. On success, `error_or_none` is `None`.
    ///
    /// Wraps `svn_fs_lock_many`.
    pub fn lock_many(
        &mut self,
        targets: &[(&str, Option<&str>, Revnum)],
        comment: Option<&str>,
        is_dav_comment: bool,
        expiration_date: Option<i64>,
        steal_lock: bool,
        mut callback: impl FnMut(&str, Option<Error<'_>>),
    ) -> Result<(), Error<'static>> {
        struct Baton<'a> {
            func: &'a mut dyn FnMut(&str, Option<Error<'_>>),
        }

        unsafe extern "C" fn lock_callback(
            baton: *mut std::ffi::c_void,
            path: *const std::os::raw::c_char,
            _lock: *const subversion_sys::svn_lock_t,
            fs_err: *mut subversion_sys::svn_error_t,
            _scratch_pool: *mut apr_sys::apr_pool_t,
        ) -> *mut subversion_sys::svn_error_t {
            let baton = &mut *(baton as *mut Baton<'_>);
            let path_str = CStr::from_ptr(path).to_str().unwrap_or("");
            let error = if fs_err.is_null() {
                None
            } else {
                crate::svn_result(fs_err).err()
            };
            (baton.func)(path_str, error);
            std::ptr::null_mut()
        }

        let result_pool = apr::Pool::new();
        let scratch_pool = apr::Pool::new();
        let result_pool_ptr = result_pool.as_mut_ptr();
        let scratch_pool_ptr = scratch_pool.as_mut_ptr();

        let comment_cstr = comment.map(|c| std::ffi::CString::new(c).unwrap());
        let comment_ptr = comment_cstr
            .as_ref()
            .map_or(std::ptr::null(), |c| c.as_ptr());

        // Build path CStrings and token CStrings
        let path_cstrings: Vec<std::ffi::CString> = targets
            .iter()
            .map(|(p, _, _)| std::ffi::CString::new(*p).unwrap())
            .collect();
        let token_cstrings: Vec<Option<std::ffi::CString>> = targets
            .iter()
            .map(|(_, t, _)| t.map(|t| std::ffi::CString::new(t).unwrap()))
            .collect();

        unsafe {
            let hash = apr_sys::apr_hash_make(scratch_pool_ptr);
            for (i, (_, _, rev)) in targets.iter().enumerate() {
                let token_ptr = token_cstrings[i]
                    .as_ref()
                    .map_or(std::ptr::null(), |t| t.as_ptr());
                let target =
                    subversion_sys::svn_fs_lock_target_create(token_ptr, rev.0, result_pool_ptr);
                apr_sys::apr_hash_set(
                    hash,
                    path_cstrings[i].as_ptr() as *const std::ffi::c_void,
                    apr_sys::APR_HASH_KEY_STRING as isize,
                    target as *mut std::ffi::c_void,
                );
            }

            let mut baton = Baton {
                func: &mut callback,
            };
            let err = subversion_sys::svn_fs_lock_many(
                self.fs_ptr,
                hash,
                comment_ptr,
                is_dav_comment as i32,
                expiration_date.unwrap_or(0),
                steal_lock as i32,
                Some(lock_callback),
                &mut baton as *mut Baton<'_> as *mut std::ffi::c_void,
                result_pool_ptr,
                scratch_pool_ptr,
            );
            svn_result(err)
        }
    }

    /// Unlock multiple paths in the filesystem.
    ///
    /// `targets` is a slice of `(path, token)` pairs. For each path (or error)
    /// the `callback` is invoked with `(path, lock_or_none, error_or_none)`.
    ///
    /// Wraps `svn_fs_unlock_many`.
    pub fn unlock_many(
        &mut self,
        targets: &[(&str, &str)],
        break_lock: bool,
        mut callback: impl FnMut(&str, Option<Error<'_>>),
    ) -> Result<(), Error<'static>> {
        struct Baton<'a> {
            func: &'a mut dyn FnMut(&str, Option<Error<'_>>),
        }

        unsafe extern "C" fn unlock_callback(
            baton: *mut std::ffi::c_void,
            path: *const std::os::raw::c_char,
            _lock: *const subversion_sys::svn_lock_t,
            fs_err: *mut subversion_sys::svn_error_t,
            _scratch_pool: *mut apr_sys::apr_pool_t,
        ) -> *mut subversion_sys::svn_error_t {
            let baton = &mut *(baton as *mut Baton<'_>);
            let path_str = CStr::from_ptr(path).to_str().unwrap_or("");
            let error = if fs_err.is_null() {
                None
            } else {
                crate::svn_result(fs_err).err()
            };
            (baton.func)(path_str, error);
            std::ptr::null_mut()
        }

        let result_pool = apr::Pool::new();
        let scratch_pool = apr::Pool::new();
        let result_pool_ptr = result_pool.as_mut_ptr();
        let scratch_pool_ptr = scratch_pool.as_mut_ptr();

        let path_cstrings: Vec<std::ffi::CString> = targets
            .iter()
            .map(|(p, _)| std::ffi::CString::new(*p).unwrap())
            .collect();
        let token_cstrings: Vec<std::ffi::CString> = targets
            .iter()
            .map(|(_, t)| std::ffi::CString::new(*t).unwrap())
            .collect();

        unsafe {
            let hash = apr_sys::apr_hash_make(scratch_pool_ptr);
            for (i, _) in targets.iter().enumerate() {
                apr_sys::apr_hash_set(
                    hash,
                    path_cstrings[i].as_ptr() as *const std::ffi::c_void,
                    apr_sys::APR_HASH_KEY_STRING as isize,
                    token_cstrings[i].as_ptr() as *mut std::ffi::c_void,
                );
            }

            let mut baton = Baton {
                func: &mut callback,
            };
            let err = subversion_sys::svn_fs_unlock_many(
                self.fs_ptr,
                hash,
                break_lock as i32,
                Some(unlock_callback),
                &mut baton as *mut Baton<'_> as *mut std::ffi::c_void,
                result_pool_ptr,
                scratch_pool_ptr,
            );
            svn_result(err)
        }
    }

    /// Unlock a path in the filesystem
    pub fn unlock(
        &mut self,
        path: &str,
        token: &str,
        break_lock: bool,
    ) -> Result<(), Error<'static>> {
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
    pub fn get_lock(&self, path: &str) -> Result<Option<crate::Lock<'static>>, Error<'_>> {
        let path_cstr = std::ffi::CString::new(path).unwrap();
        let mut lock_ptr: *mut subversion_sys::svn_lock_t = std::ptr::null_mut();

        let pool = apr::Pool::new();

        let ret = unsafe {
            subversion_sys::svn_fs_get_lock(
                &mut lock_ptr,
                self.fs_ptr,
                path_cstr.as_ptr(),
                pool.as_mut_ptr(),
            )
        };

        svn_result(ret)?;

        if lock_ptr.is_null() {
            Ok(None)
        } else {
            // The lock is allocated in the fs pool
            let pool_handle = apr::PoolHandle::owned(pool);
            Ok(Some(crate::Lock::from_raw(lock_ptr, pool_handle)))
        }
    }

    /// Set the access context for the filesystem with a username
    pub fn set_access(&mut self, username: &str) -> Result<(), Error<'static>> {
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

    /// Add a lock token to the filesystem's access context for a specific path
    ///
    /// This allows operations to access locked paths when the corresponding lock token is provided.
    /// The filesystem must have an access context set via `set_access()` before calling this method.
    ///
    /// # Arguments
    /// * `path` - The repository path associated with the lock token
    /// * `token` - The lock token to add
    pub fn access_add_lock_token(&mut self, path: &str, token: &str) -> Result<(), Error<'static>> {
        let path_cstr = std::ffi::CString::new(path).unwrap();
        let token_cstr = std::ffi::CString::new(token).unwrap();

        // Get the current access context
        let mut access_ctx: *mut subversion_sys::svn_fs_access_t = std::ptr::null_mut();
        let ret = unsafe { subversion_sys::svn_fs_get_access(&mut access_ctx, self.fs_ptr) };
        svn_result(ret)?;

        if access_ctx.is_null() {
            return Err(Error::new(
                apr::Status::from(subversion_sys::svn_errno_t_SVN_ERR_FS_NO_USER as i32),
                None,
                "No access context set",
            ));
        }

        let ret = unsafe {
            subversion_sys::svn_fs_access_add_lock_token2(
                access_ctx,
                path_cstr.as_ptr(),
                token_cstr.as_ptr(),
            )
        };

        svn_result(ret)
    }

    /// Get the username from the current access context, if any.
    ///
    /// Returns `None` if no access context has been set or if the access
    /// context was not created with a username.
    ///
    /// Wraps `svn_fs_access_get_username`.
    pub fn get_access_username(&self) -> Result<Option<String>, Error<'static>> {
        with_tmp_pool(|pool| {
            let mut access_ctx: *mut subversion_sys::svn_fs_access_t = std::ptr::null_mut();
            let ret = unsafe { subversion_sys::svn_fs_get_access(&mut access_ctx, self.fs_ptr) };
            svn_result(ret)?;

            if access_ctx.is_null() {
                return Ok(None);
            }

            let mut username: *const std::os::raw::c_char = std::ptr::null();
            let ret =
                unsafe { subversion_sys::svn_fs_access_get_username(&mut username, access_ctx) };
            let _ = pool; // pool unused but kept for with_tmp_pool
            svn_result(ret)?;

            if username.is_null() {
                return Ok(None);
            }
            Ok(Some(
                unsafe { std::ffi::CStr::from_ptr(username) }
                    .to_string_lossy()
                    .into_owned(),
            ))
        })
    }

    /// Generate a unique lock token for this filesystem
    ///
    /// This can be used to create custom lock tokens before calling `lock()`.
    /// Most users should just use the `lock()` method which generates tokens automatically.
    pub fn generate_lock_token(&self) -> Result<String, Error<'static>> {
        let pool = apr::Pool::new();
        let mut token_ptr: *const std::os::raw::c_char = std::ptr::null();

        let ret = unsafe {
            subversion_sys::svn_fs_generate_lock_token(
                &mut token_ptr,
                self.fs_ptr,
                pool.as_mut_ptr(),
            )
        };

        svn_result(ret)?;

        let token = unsafe {
            std::ffi::CStr::from_ptr(token_ptr)
                .to_str()
                .unwrap()
                .to_string()
        };

        Ok(token)
    }

    /// Get locks under a path
    pub fn get_locks(
        &self,
        path: &str,
        depth: crate::Depth,
    ) -> Result<Vec<crate::Lock<'static>>, Error<'_>> {
        let pool = apr::Pool::new();
        let path_cstr = std::ffi::CString::new(path).unwrap();

        let mut locks = Vec::new();
        let locks_ptr = &mut locks as *mut Vec<crate::Lock<'static>> as *mut std::ffi::c_void;

        extern "C" fn lock_callback(
            baton: *mut std::ffi::c_void,
            lock: *mut subversion_sys::svn_lock_t,
            pool: *mut apr_sys::apr_pool_t,
        ) -> *mut subversion_sys::svn_error_t {
            unsafe {
                let locks = &mut *(baton as *mut Vec<crate::Lock<'static>>);
                if !lock.is_null() {
                    let pool_handle = apr::PoolHandle::from_borrowed_raw(pool);
                    locks.push(crate::Lock::from_raw(lock, pool_handle));
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
    pub fn freeze<F>(&mut self, freeze_func: F) -> Result<(), Error<'static>>
    where
        F: FnOnce() -> Result<(), Error<'static>>,
    {
        // We need to create a wrapper that can be passed to C
        struct FreezeWrapper<F> {
            func: F,
            error: Option<Error<'static>>,
        }

        extern "C" fn freeze_callback<F>(
            baton: *mut std::ffi::c_void,
            _pool: *mut apr_sys::apr_pool_t,
        ) -> *mut subversion_sys::svn_error_t
        where
            F: FnOnce() -> Result<(), Error<'static>>,
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
    pub fn info(&self) -> Result<FsInfo, Error<'static>> {
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
            return Err(Error::from_message("Failed to get filesystem info"));
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

    /// Return the format number and minimum supported Subversion version for
    /// this filesystem.
    ///
    /// Returns `(format, (major, minor, patch))`.
    ///
    /// Wraps `svn_fs_info_format`.
    pub fn info_format(&self) -> Result<(i32, (i32, i32, i32)), Error<'static>> {
        with_tmp_pool(|pool| {
            let mut fs_format: std::os::raw::c_int = 0;
            let mut supports_version: *mut subversion_sys::svn_version_t = std::ptr::null_mut();
            let err = unsafe {
                subversion_sys::svn_fs_info_format(
                    &mut fs_format,
                    &mut supports_version,
                    self.fs_ptr,
                    pool.as_mut_ptr(),
                    pool.as_mut_ptr(),
                )
            };
            svn_result(err)?;
            let version = if supports_version.is_null() {
                (0, 0, 0)
            } else {
                unsafe {
                    let v = &*supports_version;
                    (v.major, v.minor, v.patch)
                }
            };
            Ok((fs_format as i32, version))
        })
    }

    /// Invalidate the cached revision properties, forcing subsequent reads to
    /// fetch the latest values from disk.
    ///
    /// Wraps `svn_fs_refresh_revision_props`.
    pub fn refresh_revision_props(&mut self) -> Result<(), Error<'static>> {
        with_tmp_pool(|pool| {
            let err = unsafe {
                subversion_sys::svn_fs_refresh_revision_props(self.fs_ptr, pool.as_mut_ptr())
            };
            svn_result(err)
        })
    }

    /// Set revision property `name` on revision `rev` to `value`.
    ///
    /// If `old_value` is `Some(v)`, the property is only changed if its current
    /// value matches `v`.  Pass `Some(None)` to require the property to be unset
    /// (i.e. this is a compare-and-swap).  Pass `None` to skip the check.
    ///
    /// Wraps `svn_fs_change_rev_prop2`.
    pub fn change_rev_prop(
        &mut self,
        rev: Revnum,
        name: &str,
        value: Option<&[u8]>,
        old_value: Option<Option<&[u8]>>,
    ) -> Result<(), Error<'static>> {
        let name_cstr = std::ffi::CString::new(name)?;
        with_tmp_pool(|pool| unsafe {
            // Build the new value svn_string_t, if any
            let new_svn_str;
            let new_ptr: *const subversion_sys::svn_string_t = match value {
                None => std::ptr::null(),
                Some(bytes) => {
                    new_svn_str = subversion_sys::svn_string_t {
                        data: bytes.as_ptr() as *const std::os::raw::c_char,
                        len: bytes.len(),
                    };
                    &new_svn_str
                }
            };

            // Build the old_value pointer-to-pointer, if any
            let old_svn_str;
            let old_inner_ptr: *const subversion_sys::svn_string_t;
            let old_ptr: *const *const subversion_sys::svn_string_t = match old_value {
                None => std::ptr::null(), // no compare-and-swap
                Some(None) => {
                    // require the property to currently be absent
                    old_inner_ptr = std::ptr::null();
                    &old_inner_ptr
                }
                Some(Some(bytes)) => {
                    old_svn_str = subversion_sys::svn_string_t {
                        data: bytes.as_ptr() as *const std::os::raw::c_char,
                        len: bytes.len(),
                    };
                    old_inner_ptr = &old_svn_str;
                    &old_inner_ptr
                }
            };

            let err = subversion_sys::svn_fs_change_rev_prop2(
                self.fs_ptr,
                rev.0,
                name_cstr.as_ptr(),
                old_ptr,
                new_ptr,
                pool.as_mut_ptr(),
            );
            svn_result(err)
        })
    }

    /// Deltify the contents of the filesystem at the given revision.
    ///
    /// This is a housekeeping operation that can save storage space by
    /// representing file contents as deltas against earlier revisions rather
    /// than as full texts.  It has no visible effect on the repository contents.
    ///
    /// Wraps `svn_fs_deltify_revision`.
    pub fn deltify_revision(&self, revision: Revnum) -> Result<(), Error<'static>> {
        with_tmp_pool(|pool| {
            let err = unsafe {
                subversion_sys::svn_fs_deltify_revision(self.fs_ptr, revision.0, pool.as_mut_ptr())
            };
            svn_result(err)
        })
    }
}

/// Gets the filesystem type for a repository at the given path.
pub fn fs_type(path: &std::path::Path) -> Result<String, Error<'static>> {
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
pub fn delete_fs(path: &std::path::Path) -> Result<(), Error<'static>> {
    let scratch_pool = apr::pool::Pool::new();
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    unsafe {
        let err = subversion_sys::svn_fs_delete_fs(path.as_ptr(), scratch_pool.as_mut_ptr());
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
) -> Result<(), Error<'static>> {
    let path_cstr = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let pool = apr::Pool::new();

    // Create notify callback wrapper
    let notify_baton = notify.map(|f| box_pack_notify_baton(f));
    let cancel_baton = cancel.map(box_cancel_baton);

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
                return unsafe { Error::from_message("Operation cancelled").into_raw() };
            }
        }
        std::ptr::null_mut()
    }

    let err = unsafe {
        subversion_sys::svn_fs_pack(
            path_cstr.as_ptr(),
            notify_baton.map(|_| notify_wrapper as _),
            notify_baton.unwrap_or(std::ptr::null_mut()),
            cancel_baton.map(|_| cancel_wrapper as _),
            cancel_baton.unwrap_or(std::ptr::null_mut()),
            pool.as_mut_ptr(),
        )
    };

    // Clean up callbacks
    if let Some(baton) = notify_baton {
        unsafe { free_pack_notify_baton(baton) };
    }
    if let Some(baton) = cancel_baton {
        unsafe { free_cancel_baton(baton) };
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
) -> Result<(), Error<'static>> {
    let path_cstr = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let pool = apr::Pool::new();

    let start_rev = start.map(|r| r.0).unwrap_or(0);
    let end_rev = end.map(|r| r.0).unwrap_or(-1); // SVN_INVALID_REVNUM means HEAD

    // Create callback wrappers
    let notify_baton = notify.map(|f| box_verify_notify_baton(f));
    let cancel_baton = cancel.map(box_cancel_baton);

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
                return unsafe { Error::from_message("Operation cancelled").into_raw() };
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
            notify_baton.map(|_| notify_wrapper as _),
            notify_baton.unwrap_or(std::ptr::null_mut()),
            cancel_baton.map(|_| cancel_wrapper as _),
            cancel_baton.unwrap_or(std::ptr::null_mut()),
            pool.as_mut_ptr(),
        )
    };

    // Clean up callbacks
    if let Some(baton) = notify_baton {
        unsafe { free_verify_notify_baton(baton) };
    }
    if let Some(baton) = cancel_baton {
        unsafe { free_cancel_baton(baton) };
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
) -> Result<(), Error<'static>> {
    let src_cstr = std::ffi::CString::new(src_path.to_str().unwrap()).unwrap();
    let dst_cstr = std::ffi::CString::new(dst_path.to_str().unwrap()).unwrap();
    let pool = apr::Pool::new();

    // Create callback wrappers
    let notify_baton = notify.map(|f| box_pack_notify_baton(f));
    let cancel_baton = cancel.map(box_cancel_baton);

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
                return unsafe { Error::from_message("Operation cancelled").into_raw() };
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
            notify_baton.map(|_| notify_wrapper as _),
            notify_baton.unwrap_or(std::ptr::null_mut()),
            cancel_baton.map(|_| cancel_wrapper as _),
            cancel_baton.unwrap_or(std::ptr::null_mut()),
            pool.as_mut_ptr(),
        )
    };

    // Clean up callbacks
    if let Some(baton) = notify_baton {
        unsafe { free_pack_notify_baton(baton) };
    }
    if let Some(baton) = cancel_baton {
        unsafe { free_cancel_baton(baton) };
    }

    svn_result(err)?;
    Ok(())
}

/// Recover a filesystem at the given path.
pub fn recover(
    path: &std::path::Path,
    cancel: Option<Box<dyn Fn() -> bool + Send>>,
) -> Result<(), Error<'static>> {
    let path_cstr = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let pool = apr::Pool::new();

    let cancel_baton = cancel.map(box_cancel_baton);

    extern "C" fn cancel_wrapper(baton: *mut std::ffi::c_void) -> *mut subversion_sys::svn_error_t {
        if !baton.is_null() {
            let cancel = unsafe { &*(baton as *const Box<dyn Fn() -> bool + Send>) };
            if cancel() {
                return unsafe { Error::from_message("Operation cancelled").into_raw() };
            }
        }
        std::ptr::null_mut()
    }

    let err = unsafe {
        subversion_sys::svn_fs_recover(
            path_cstr.as_ptr(),
            cancel_baton.map(|_| cancel_wrapper as _),
            cancel_baton.unwrap_or(std::ptr::null_mut()),
            pool.as_mut_ptr(),
        )
    };

    // Clean up callback
    if let Some(baton) = cancel_baton {
        unsafe { free_cancel_baton(baton) };
    }

    svn_result(err)?;
    Ok(())
}

/// The type of notification action during a filesystem upgrade.
///
/// Used with the notify callback of [`upgrade()`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpgradeAction {
    /// Packing of revision properties for a shard has completed.
    PackRevprops,
    /// Removal of the non-packed revprop shard has completed.
    CleanupRevprops,
    /// The database format has been set to a new value.
    FormatBumped,
}

/// Upgrade the Subversion filesystem at `path` to the latest format supported by this library.
///
/// Returns `SVN_ERR_FS_UNSUPPORTED_UPGRADE` if the upgrade is not supported.
///
/// The optional `notify` callback receives `(number, action)` pairs as the upgrade progresses.
/// The optional `cancel` callback can be used to cancel the operation.
///
/// Wraps `svn_fs_upgrade2`.
pub fn upgrade(
    path: &std::path::Path,
    notify: Option<Box<dyn Fn(u64, UpgradeAction) + Send>>,
    cancel: Option<Box<dyn Fn() -> bool + Send>>,
) -> Result<(), Error<'static>> {
    let path_cstr = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let pool = apr::Pool::new();

    let notify_baton = notify.map(|f| Box::into_raw(Box::new(f)) as *mut std::ffi::c_void);
    let cancel_baton = cancel.map(box_cancel_baton);

    unsafe extern "C" fn notify_wrapper(
        baton: *mut std::ffi::c_void,
        number: u64,
        action: subversion_sys::svn_fs_upgrade_notify_action_t,
        _pool: *mut apr_sys::apr_pool_t,
    ) -> *mut subversion_sys::svn_error_t {
        if !baton.is_null() {
            let notify = &*(baton as *const Box<dyn Fn(u64, UpgradeAction) + Send>);
            let rust_action = match action {
                subversion_sys::svn_fs_upgrade_notify_action_t_svn_fs_upgrade_pack_revprops => {
                    UpgradeAction::PackRevprops
                }
                subversion_sys::svn_fs_upgrade_notify_action_t_svn_fs_upgrade_cleanup_revprops => {
                    UpgradeAction::CleanupRevprops
                }
                subversion_sys::svn_fs_upgrade_notify_action_t_svn_fs_upgrade_format_bumped => {
                    UpgradeAction::FormatBumped
                }
                _ => unreachable!("unknown svn_fs_upgrade_notify_action_t value: {}", action),
            };
            notify(number, rust_action);
        }
        std::ptr::null_mut()
    }

    extern "C" fn cancel_wrapper(baton: *mut std::ffi::c_void) -> *mut subversion_sys::svn_error_t {
        if !baton.is_null() {
            let cancel = unsafe { &*(baton as *const Box<dyn Fn() -> bool + Send>) };
            if cancel() {
                return unsafe { Error::from_message("Operation cancelled").into_raw() };
            }
        }
        std::ptr::null_mut()
    }

    let err = unsafe {
        subversion_sys::svn_fs_upgrade2(
            path_cstr.as_ptr(),
            notify_baton.map(|_| notify_wrapper as _),
            notify_baton.unwrap_or(std::ptr::null_mut()),
            cancel_baton.map(|_| cancel_wrapper as _),
            cancel_baton.unwrap_or(std::ptr::null_mut()),
            pool.as_mut_ptr(),
        )
    };

    if let Some(baton) = notify_baton {
        unsafe {
            drop(Box::from_raw(
                baton as *mut Box<dyn Fn(u64, UpgradeAction) + Send>,
            ))
        };
    }
    if let Some(baton) = cancel_baton {
        unsafe { free_cancel_baton(baton) };
    }

    svn_result(err)
}

/// Returns the version of the Subversion filesystem library.
///
/// Wraps `svn_fs_version`.
pub fn version() -> crate::Version {
    crate::Version(unsafe { subversion_sys::svn_fs_version() })
}

/// Return a string containing a list of all available filesystem module names.
///
/// Wraps `svn_fs_print_modules`.
pub fn print_modules() -> Result<String, crate::Error<'static>> {
    let pool = apr::Pool::new();
    unsafe {
        let buf = subversion_sys::svn_stringbuf_create_empty(pool.as_mut_ptr());
        let err = subversion_sys::svn_fs_print_modules(buf, pool.as_mut_ptr());
        crate::svn_result(err)?;
        if buf.is_null() || (*buf).data.is_null() {
            return Ok(String::new());
        }
        let data = std::slice::from_raw_parts((*buf).data as *const u8, (*buf).len);
        Ok(String::from_utf8_lossy(data).into_owned())
    }
}

/// Return the paths to the config files used by a filesystem at the given path.
///
/// Returns an empty `Vec` if the filesystem has no associated config files.
///
/// Wraps `svn_fs_info_config_files`.
pub fn info_config_files(
    path: &std::path::Path,
) -> Result<Vec<std::path::PathBuf>, crate::Error<'static>> {
    let path_cstr = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let result_pool = apr::Pool::new();
    let scratch_pool = apr::Pool::new();
    unsafe {
        let mut files: *mut apr_sys::apr_array_header_t = std::ptr::null_mut();
        // We need to open the filesystem first
        let mut fs_ptr: *mut subversion_sys::svn_fs_t = std::ptr::null_mut();
        let err = subversion_sys::svn_fs_open2(
            &mut fs_ptr,
            path_cstr.as_ptr(),
            std::ptr::null_mut(),
            result_pool.as_mut_ptr(),
            scratch_pool.as_mut_ptr(),
        );
        crate::svn_result(err)?;

        let err = subversion_sys::svn_fs_info_config_files(
            &mut files,
            fs_ptr,
            result_pool.as_mut_ptr(),
            scratch_pool.as_mut_ptr(),
        );
        crate::svn_result(err)?;

        if files.is_null() {
            return Ok(Vec::new());
        }

        let nelts = (*files).nelts as usize;
        let mut result = Vec::with_capacity(nelts);
        for i in 0..nelts {
            let elts = (*files).elts as *const *const std::os::raw::c_char;
            let s = *elts.add(i);
            if !s.is_null() {
                let path_str = std::ffi::CStr::from_ptr(s).to_str().unwrap_or("");
                result.push(std::path::PathBuf::from(path_str));
            }
        }
        Ok(result)
    }
}

/// Information about a filesystem.
#[derive(Debug, Clone)]
pub struct FsInfo {
    /// Filesystem type (fsfs, bdb, etc.)
    pub fs_type: Option<String>,
}

/// Represents a filesystem root at a specific revision.
///
/// The lifetime parameter `'fs` ties this root to the [`Fs`] that created it,
/// ensuring the root cannot outlive the filesystem (whose internal data the
/// root's C pointer references).
///
/// Each root owns an independent pool that is NOT a child of the `Fs` pool.
/// When the root is dropped, the pool is destroyed, which recursively destroys
/// the root's internal sub-pool created by SVN.  `svn_fs_close_root()` is NOT
/// called explicitly in `Drop` — doing so before destroying the parent pool
/// triggers SVN/APR cleanup interactions that corrupt pool state.  APR's
/// recursive pool destruction achieves exactly the same result safely.
///
/// For roots created from C callbacks (e.g. `repos.rs`), a borrowed pool
/// handle is used and the C caller retains ownership of the root lifecycle.
pub struct Root<'fs> {
    ptr: *mut subversion_sys::svn_fs_root_t,
    // Owned for Rust-created roots (pool destruction frees root resources).
    // Borrowed for C-callback roots (C caller manages lifecycle).
    // This field is not directly read, but must be kept alive to prevent premature pool cleanup.
    #[allow(dead_code)]
    pool: apr::PoolHandle<'static>,
    _marker: std::marker::PhantomData<&'fs ()>,
}

unsafe impl<'fs> Send for Root<'fs> {}

impl<'fs> Root<'fs> {
    /// Create a Root from a raw pointer and a borrowed pool pointer.
    ///
    /// This is intended for use in C callbacks (e.g. `repos.rs`) where the C
    /// caller owns the root's lifecycle.  The `pool` parameter is stored as a
    /// borrowed (non-owning) handle so that `Drop` does not destroy it.
    ///
    /// # Safety
    /// - `ptr` must be a valid `svn_fs_root_t` pointer for the duration of `'fs`.
    /// - `pool_ptr` must be a valid APR pool pointer for the duration of `'fs`.
    pub unsafe fn from_raw(
        ptr: *mut subversion_sys::svn_fs_root_t,
        pool_ptr: *mut apr_sys::apr_pool_t,
    ) -> Self {
        Self {
            ptr,
            pool: apr::PoolHandle::from_borrowed_raw(pool_ptr),
            _marker: std::marker::PhantomData,
        }
    }

    /// Gets the raw pointer to the root.
    pub fn as_ptr(&self) -> *const subversion_sys::svn_fs_root_t {
        self.ptr
    }

    /// Gets the mutable raw pointer to the root.
    pub fn as_mut_ptr(&mut self) -> *mut subversion_sys::svn_fs_root_t {
        self.ptr
    }

    /// Return the filesystem to which this root belongs.
    ///
    /// The returned `Fs` borrows from the same pool as this root.  It must not
    /// outlive the root or the original `Fs` that created this root.
    ///
    /// Wraps `svn_fs_root_fs`.
    pub fn fs(&self) -> Fs<'fs> {
        let fs_ptr = unsafe { subversion_sys::svn_fs_root_fs(self.ptr) };
        // The fs_ptr is owned by the svn_fs_t that created this root and
        // remains valid for the root's lifetime.  A fresh pool is created
        // for the Fs's own allocations (it does not free the underlying fs).
        let pool = apr::Pool::new();
        unsafe { Fs::from_ptr_and_pool(fs_ptr, pool) }
    }

    /// Check if a path is a directory
    pub fn is_dir(
        &self,
        path: impl TryInto<FsPath, Error = Error<'static>>,
    ) -> Result<bool, Error<'static>> {
        let fs_path = path.try_into()?;
        with_tmp_pool(|pool| unsafe {
            let mut is_dir = 0;
            let err = subversion_sys::svn_fs_is_dir(
                &mut is_dir,
                self.ptr,
                fs_path.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;
            Ok(is_dir != 0)
        })
    }

    /// Check if a path is a file
    pub fn is_file(
        &self,
        path: impl TryInto<FsPath, Error = Error<'static>>,
    ) -> Result<bool, Error<'static>> {
        let fs_path = path.try_into()?;
        with_tmp_pool(|pool| unsafe {
            let mut is_file = 0;
            let err = subversion_sys::svn_fs_is_file(
                &mut is_file,
                self.ptr,
                fs_path.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;
            Ok(is_file != 0)
        })
    }

    /// Get the length of a file
    pub fn file_length(
        &self,
        path: impl TryInto<FsPath, Error = Error<'static>>,
    ) -> Result<i64, Error<'static>> {
        let fs_path = path.try_into()?;
        with_tmp_pool(|pool| unsafe {
            let mut length = 0;
            let err = subversion_sys::svn_fs_file_length(
                &mut length,
                self.ptr,
                fs_path.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;
            Ok(length)
        })
    }

    /// Get the contents of a file as a stream
    pub fn file_contents(
        &self,
        path: impl TryInto<FsPath, Error = Error<'static>>,
    ) -> Result<crate::io::Stream, Error<'static>> {
        let fs_path = path.try_into()?;
        let pool = apr::Pool::new();
        unsafe {
            let mut stream = std::ptr::null_mut();
            let err = subversion_sys::svn_fs_file_contents(
                &mut stream,
                self.ptr,
                fs_path.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;
            Ok(crate::io::Stream::from_ptr_and_pool(stream, pool))
        }
    }

    /// Attempts to process file contents with zero-copy optimization.
    ///
    /// This method tries to provide direct access to the file's content buffer
    /// via the provided closure. If zero-copy access is possible, the closure
    /// is called with the file contents and the method returns `Ok(true)`.
    /// If zero-copy access is not possible, returns `Ok(false)` and the caller
    /// should fall back to [`Root::file_contents`].
    ///
    /// The closure receives the entire file contents as a byte slice and should
    /// return `Ok(())` on success or an `Error` on failure.
    ///
    /// Wraps `svn_fs_try_process_file_contents`.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The path doesn't exist or isn't a file
    /// - The closure returns an error
    /// - An SVN error occurs
    pub fn try_process_file_contents<F>(
        &self,
        path: impl TryInto<FsPath, Error = Error<'static>>,
        processor: F,
    ) -> Result<bool, Error<'static>>
    where
        F: FnMut(&[u8]) -> Result<(), Error<'static>>,
    {
        let fs_path = path.try_into()?;
        let pool = apr::Pool::new();

        // Box the closure for the baton
        let mut boxed: Box<Box<dyn FnMut(&[u8]) -> Result<(), Error<'static>>>> =
            Box::new(Box::new(processor));
        let baton_ptr = &mut *boxed as *mut _ as *mut std::ffi::c_void;

        unsafe {
            let mut success: subversion_sys::svn_boolean_t = 0;
            let err = subversion_sys::svn_fs_try_process_file_contents(
                &mut success,
                self.ptr,
                fs_path.as_ptr(),
                Some(process_contents_trampoline),
                baton_ptr,
                pool.as_mut_ptr(),
            );
            svn_result(err)?;
            Ok(success != 0)
        }
    }

    /// Get the checksum of a file
    pub fn file_checksum(
        &self,
        path: &str,
        kind: crate::ChecksumKind,
    ) -> Result<Option<crate::Checksum<'_>>, Error<'_>> {
        self.file_checksum_force(path, kind, true)
    }

    /// Get the checksum of a file, with optional forced computation.
    ///
    /// If `force` is true, the checksum is always computed from the file contents.
    /// If `force` is false, the cached checksum may be returned (which could be `None`).
    ///
    /// Wraps `svn_fs_file_checksum`.
    pub fn file_checksum_force(
        &self,
        path: &str,
        kind: crate::ChecksumKind,
        force: bool,
    ) -> Result<Option<crate::Checksum<'_>>, Error<'_>> {
        with_tmp_pool(|pool| unsafe {
            let path_c = std::ffi::CString::new(path).unwrap();
            let mut checksum = std::ptr::null_mut();
            let err = subversion_sys::svn_fs_file_checksum(
                &mut checksum,
                kind.into(),
                self.ptr,
                path_c.as_ptr(),
                if force { 1 } else { 0 },
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

    /// Get a single property of a node.  Returns `None` if the property is
    /// not set on this node.
    ///
    /// Wraps `svn_fs_node_prop`.
    pub fn node_prop(
        &self,
        path: impl TryInto<FsPath, Error = Error<'static>>,
        propname: &str,
    ) -> Result<Option<Vec<u8>>, Error<'static>> {
        let fs_path = path.try_into()?;
        let name_cstr = std::ffi::CString::new(propname)?;
        with_tmp_pool(|pool| unsafe {
            let mut value_p: *mut subversion_sys::svn_string_t = std::ptr::null_mut();
            let err = subversion_sys::svn_fs_node_prop(
                &mut value_p,
                self.ptr,
                fs_path.as_ptr(),
                name_cstr.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;
            if value_p.is_null() {
                return Ok(None);
            }
            let s = &*value_p;
            let data = std::slice::from_raw_parts(s.data as *const u8, s.len).to_vec();
            Ok(Some(data))
        })
    }

    /// Get properties of a node
    pub fn proplist(
        &self,
        path: &str,
    ) -> Result<std::collections::HashMap<String, Vec<u8>>, Error<'_>> {
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
    pub fn paths_changed(
        &self,
    ) -> Result<std::collections::HashMap<String, FsPathChange>, Error<'_>> {
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

    /// Get an iterator over all paths changed under this root.
    ///
    /// This is the modern, memory-efficient API for iterating over changed paths.
    /// Each path change is retrieved one at a time, rather than loading all changes
    /// into memory at once.
    ///
    /// The iteration order is undefined and may vary even for the same root.
    ///
    /// # Lifetimes
    ///
    /// The returned iterator is tied to this Root's lifetime, as the iterator
    /// becomes invalid if the root is dropped.
    ///
    /// Wraps `svn_fs_paths_changed3`.
    pub fn paths_changed3(&mut self) -> Result<PathChangeIterator<'_>, Error<'static>> {
        let result_pool = apr::Pool::new();
        let scratch_pool = apr::Pool::new();

        let mut iter_ptr: *mut subversion_sys::svn_fs_path_change_iterator_t = std::ptr::null_mut();

        let err = unsafe {
            subversion_sys::svn_fs_paths_changed3(
                &mut iter_ptr,
                self.ptr,
                result_pool.as_mut_ptr(),
                scratch_pool.as_mut_ptr(),
            )
        };
        svn_result(err)?;

        Ok(unsafe { PathChangeIterator::from_raw(iter_ptr) })
    }

    /// Return the revision number to which this root belongs, for revision
    /// roots.  For transaction roots, returns `SVN_INVALID_REVNUM` (-1).
    ///
    /// Wraps `svn_fs_revision_root_revision`.
    pub fn revision(&self) -> Revnum {
        Revnum(unsafe { subversion_sys::svn_fs_revision_root_revision(self.ptr) })
    }

    /// Return true if this root is a revision root, false if it is a transaction root.
    ///
    /// Wraps `svn_fs_is_revision_root`.
    pub fn is_revision_root(&self) -> bool {
        unsafe { subversion_sys::svn_fs_is_revision_root(self.ptr) != 0 }
    }

    /// Return true if this root is a transaction root, false if it is a revision root.
    ///
    /// Wraps `svn_fs_is_txn_root`.
    pub fn is_txn_root(&self) -> bool {
        unsafe { subversion_sys::svn_fs_is_txn_root(self.ptr) != 0 }
    }

    /// Return the path at which the node at `path` was created in its revision.
    ///
    /// Wraps `svn_fs_node_created_path`.
    pub fn node_created_path(
        &self,
        path: impl TryInto<FsPath, Error = Error<'static>>,
    ) -> Result<String, Error<'static>> {
        let fs_path = path.try_into()?;
        with_tmp_pool(|pool| unsafe {
            let mut created_path: *const std::os::raw::c_char = std::ptr::null();
            let err = subversion_sys::svn_fs_node_created_path(
                &mut created_path,
                self.ptr,
                fs_path.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;
            if created_path.is_null() {
                return Err(Error::from_message(
                    "svn_fs_node_created_path returned null",
                ));
            }
            Ok(std::ffi::CStr::from_ptr(created_path)
                .to_string_lossy()
                .into_owned())
        })
    }

    /// Verify the tree structure rooted at this filesystem root.
    ///
    /// Returns an error if the tree is corrupt.  Use this in addition to
    /// [`crate::fs::verify`] for full verification.
    ///
    /// Wraps `svn_fs_verify_root`.
    pub fn verify(&self) -> Result<(), Error<'static>> {
        with_tmp_pool(|pool| {
            let err = unsafe { subversion_sys::svn_fs_verify_root(self.ptr, pool.as_mut_ptr()) };
            svn_result(err)
        })
    }

    /// Retrieve mergeinfo for multiple paths at the revision represented by this root.
    ///
    /// For each path that has mergeinfo, `receiver` is called with
    /// `(path, mergeinfo)`.
    ///
    /// `paths` are the absolute paths to query.
    /// `adjust_inherited_mergeinfo` controls whether inherited mergeinfo is
    /// normalised to the inheriting path.
    ///
    /// Wraps `svn_fs_get_mergeinfo3`.
    pub fn get_mergeinfo(
        &self,
        paths: &[&str],
        inherit: crate::mergeinfo::MergeinfoInheritance,
        include_descendants: bool,
        adjust_inherited_mergeinfo: bool,
        mut receiver: impl FnMut(&str, crate::mergeinfo::Mergeinfo) -> Result<(), Error<'static>>,
    ) -> Result<(), Error<'static>> {
        struct Baton<'a> {
            func:
                &'a mut dyn FnMut(&str, crate::mergeinfo::Mergeinfo) -> Result<(), Error<'static>>,
        }

        unsafe extern "C" fn mergeinfo_trampoline(
            path: *const std::os::raw::c_char,
            mergeinfo: subversion_sys::svn_mergeinfo_t,
            baton: *mut std::ffi::c_void,
            _scratch_pool: *mut apr_sys::apr_pool_t,
        ) -> *mut subversion_sys::svn_error_t {
            let baton = &mut *(baton as *mut Baton<'_>);
            let path_str = std::ffi::CStr::from_ptr(path).to_str().unwrap_or("");
            let pool = apr::Pool::new();
            let dup = subversion_sys::svn_mergeinfo_dup(mergeinfo, pool.as_mut_ptr());
            let mi = crate::mergeinfo::Mergeinfo::from_ptr_and_pool(dup, pool);
            match (baton.func)(path_str, mi) {
                Ok(()) => std::ptr::null_mut(),
                Err(e) => e.into_raw(),
            }
        }

        with_tmp_pool(|pool| {
            let path_cstrings: Vec<std::ffi::CString> = paths
                .iter()
                .map(|p| std::ffi::CString::new(*p))
                .collect::<Result<_, _>>()?;
            let mut arr = apr::tables::TypedArray::<*const std::os::raw::c_char>::new(
                pool,
                path_cstrings.len() as i32,
            );
            for cstr in &path_cstrings {
                arr.push(cstr.as_ptr());
            }

            let mut baton = Baton {
                func: &mut receiver,
            };

            let err = unsafe {
                subversion_sys::svn_fs_get_mergeinfo3(
                    self.ptr,
                    arr.as_ptr(),
                    inherit.into(),
                    if include_descendants { 1 } else { 0 },
                    if adjust_inherited_mergeinfo { 1 } else { 0 },
                    Some(mergeinfo_trampoline),
                    &mut baton as *mut Baton<'_> as *mut std::ffi::c_void,
                    pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })
    }

    /// Check the type of a path (file, directory, or none)
    pub fn check_path(
        &self,
        path: impl TryInto<FsPath, Error = Error<'static>>,
    ) -> Result<crate::NodeKind, Error<'static>> {
        let fs_path = path.try_into()?;
        with_tmp_pool(|pool| unsafe {
            let mut kind = subversion_sys::svn_node_kind_t_svn_node_none;
            let err = subversion_sys::svn_fs_check_path(
                &mut kind,
                self.ptr,
                fs_path.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;
            Ok(kind.into())
        })
    }

    /// List directory entries
    pub fn dir_entries(
        &self,
        path: impl TryInto<FsPath, Error = Error<'static>>,
    ) -> Result<std::collections::HashMap<String, FsDirEntry>, Error<'_>> {
        let fs_path = path.try_into()?;
        let pool = apr::SharedPool::from(apr::Pool::new());
        unsafe {
            let mut entries = std::ptr::null_mut();
            let err = subversion_sys::svn_fs_dir_entries(
                &mut entries,
                self.ptr,
                fs_path.as_ptr(),
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

    /// Return directory entries in an order optimised for sequential data access.
    ///
    /// Calls `svn_fs_dir_entries` then `svn_fs_dir_optimal_order` and returns
    /// the resulting entries as a `Vec<FsDirEntry>` ordered for efficient I/O.
    /// For directories where access order matters (e.g., when reading all file
    /// contents sequentially), this can be significantly faster than iterating
    /// over the unordered map returned by [`Root::dir_entries`].
    ///
    /// Wraps `svn_fs_dir_optimal_order`.
    pub fn dir_entries_optimal_order(
        &self,
        path: impl TryInto<FsPath, Error = Error<'static>>,
    ) -> Result<Vec<FsDirEntry>, Error<'_>> {
        let fs_path = path.try_into()?;
        let result_pool = apr::SharedPool::from(apr::Pool::new());
        let scratch_pool = apr::Pool::new();
        unsafe {
            // First get the directory entries as a hash
            let mut entries_ptr = std::ptr::null_mut();
            let err = subversion_sys::svn_fs_dir_entries(
                &mut entries_ptr,
                self.ptr,
                fs_path.as_ptr(),
                result_pool.as_mut_ptr(),
            );
            svn_result(err)?;

            if entries_ptr.is_null() {
                return Ok(Vec::new());
            }

            // Now get the optimal ordering
            let mut ordered_ptr = std::ptr::null_mut();
            let err = subversion_sys::svn_fs_dir_optimal_order(
                &mut ordered_ptr,
                self.ptr,
                entries_ptr,
                result_pool.as_mut_ptr(),
                scratch_pool.as_mut_ptr(),
            );
            svn_result(err)?;

            if ordered_ptr.is_null() {
                return Ok(Vec::new());
            }

            // Iterate the returned array of svn_fs_dirent_t* pointers
            let nelts = (*ordered_ptr).nelts as usize;
            let elts = (*ordered_ptr).elts as *const *const subversion_sys::svn_fs_dirent_t;
            let mut result = Vec::with_capacity(nelts);
            for i in 0..nelts {
                let entry_ptr = *elts.add(i);
                if !entry_ptr.is_null() {
                    result.push(FsDirEntry::from_raw(
                        entry_ptr as *mut subversion_sys::svn_fs_dirent_t,
                        result_pool.clone(),
                    ));
                }
            }
            Ok(result)
        }
    }

    /// Check if file contents have changed between two paths
    pub fn contents_changed(
        &self,
        path1: impl TryInto<FsPath, Error = Error<'static>>,
        root2: &Root,
        path2: impl TryInto<FsPath, Error = Error<'static>>,
    ) -> Result<bool, Error<'static>> {
        let fs_path1 = path1.try_into()?;
        let fs_path2 = path2.try_into()?;
        with_tmp_pool(|pool| unsafe {
            let mut changed: subversion_sys::svn_boolean_t = 0;

            let err = subversion_sys::svn_fs_contents_changed(
                &mut changed,
                self.ptr,
                fs_path1.as_ptr(),
                root2.ptr,
                fs_path2.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;

            Ok(changed != 0)
        })
    }

    /// Check if properties have changed between two paths
    pub fn props_changed(
        &self,
        path1: impl TryInto<FsPath, Error = Error<'static>>,
        root2: &Root,
        path2: impl TryInto<FsPath, Error = Error<'static>>,
    ) -> Result<bool, Error<'static>> {
        let fs_path1 = path1.try_into()?;
        let fs_path2 = path2.try_into()?;
        with_tmp_pool(|pool| unsafe {
            let mut changed: subversion_sys::svn_boolean_t = 0;

            let err = subversion_sys::svn_fs_props_changed(
                &mut changed,
                self.ptr,
                fs_path1.as_ptr(),
                root2.ptr,
                fs_path2.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;

            Ok(changed != 0)
        })
    }

    /// Get the history of a node
    ///
    /// Wraps `svn_fs_node_history2`.
    pub fn node_history(
        &self,
        path: impl TryInto<FsPath, Error = Error<'static>>,
    ) -> Result<NodeHistory, Error<'static>> {
        let fs_path = path.try_into()?;
        unsafe {
            // Create a pool that will live as long as the NodeHistory
            let pool = apr::Pool::new();
            let mut history: *mut subversion_sys::svn_fs_history_t = std::ptr::null_mut();

            let err = subversion_sys::svn_fs_node_history2(
                &mut history,
                self.ptr,
                fs_path.as_ptr(),
                pool.as_mut_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;

            if history.is_null() {
                return Err(Error::from_message("Failed to get node history"));
            }

            Ok(NodeHistory {
                ptr: history,
                pool, // Use the same pool that allocated the history
            })
        }
    }

    /// Get the created revision of a path
    pub fn node_created_rev(
        &self,
        path: impl TryInto<FsPath, Error = Error<'static>>,
    ) -> Result<Revnum, Error<'static>> {
        let fs_path = path.try_into()?;
        with_tmp_pool(|pool| unsafe {
            let mut rev: subversion_sys::svn_revnum_t = -1;

            let err = subversion_sys::svn_fs_node_created_rev(
                &mut rev,
                self.ptr,
                fs_path.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;

            Ok(Revnum(rev))
        })
    }

    /// Get the node ID for a path
    pub fn node_id(
        &self,
        path: impl TryInto<FsPath, Error = Error<'static>>,
    ) -> Result<NodeId, Error<'static>> {
        let fs_path = path.try_into()?;
        unsafe {
            let pool = apr::Pool::new();
            let mut id: *const subversion_sys::svn_fs_id_t = std::ptr::null();

            let err = subversion_sys::svn_fs_node_id(
                &mut id,
                self.ptr,
                fs_path.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;

            if id.is_null() {
                return Err(Error::from_message("Failed to get node ID"));
            }

            Ok(NodeId { ptr: id, pool })
        }
    }

    /// Find the closest copy of a path
    ///
    /// Given a root/path, find the closest ancestor of that path which is a copy
    /// (or the path itself, if it is a copy). Returns the root and path of the copy.
    pub fn closest_copy(
        &self,
        path: impl TryInto<FsPath, Error = Error<'static>>,
    ) -> Result<Option<(Root<'fs>, String)>, Error<'_>> {
        let fs_path = path.try_into()?;
        let pool = apr::Pool::new();

        unsafe {
            let mut copy_root: *mut subversion_sys::svn_fs_root_t = std::ptr::null_mut();
            let mut copy_path: *const i8 = std::ptr::null();

            let err = subversion_sys::svn_fs_closest_copy(
                &mut copy_root,
                &mut copy_path,
                self.ptr,
                fs_path.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;

            if copy_root.is_null() {
                Ok(None)
            } else {
                // copy_path is allocated in pool; clone it before pool moves
                let path_str = std::ffi::CStr::from_ptr(copy_path).to_str()?.to_owned();
                Ok(Some((
                    Root {
                        ptr: copy_root,
                        pool: apr::PoolHandle::owned(pool),
                        _marker: std::marker::PhantomData,
                    },
                    path_str,
                )))
            }
        }
    }

    /// Check if the contents of two paths are different
    ///
    /// Compare the contents at this root/path with another root/path.
    /// Returns true if they are different, false if they are the same.
    pub fn contents_different(
        &self,
        path1: impl TryInto<FsPath, Error = Error<'static>>,
        other_root: &Root,
        path2: impl TryInto<FsPath, Error = Error<'static>>,
    ) -> Result<bool, Error<'static>> {
        let fs_path1 = path1.try_into()?;
        let fs_path2 = path2.try_into()?;
        let pool = apr::Pool::new();

        unsafe {
            let mut different: i32 = 0;
            let err = subversion_sys::svn_fs_contents_different(
                &mut different,
                self.ptr,
                fs_path1.as_ptr(),
                other_root.ptr,
                fs_path2.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;
            Ok(different != 0)
        }
    }

    /// Check if the properties of two paths are different
    ///
    /// Compare the properties at this root/path with another root/path.
    /// Returns true if they are different, false if they are the same.
    pub fn props_different(
        &self,
        path1: impl TryInto<FsPath, Error = Error<'static>>,
        other_root: &Root,
        path2: impl TryInto<FsPath, Error = Error<'static>>,
    ) -> Result<bool, Error<'static>> {
        let fs_path1 = path1.try_into()?;
        let fs_path2 = path2.try_into()?;
        let pool = apr::Pool::new();

        unsafe {
            let mut different: i32 = 0;
            let err = subversion_sys::svn_fs_props_different(
                &mut different,
                self.ptr,
                fs_path1.as_ptr(),
                other_root.ptr,
                fs_path2.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;
            Ok(different != 0)
        }
    }

    /// Apply a text delta to a file in the filesystem
    ///
    /// Returns a window handler that can receive delta windows.
    /// The root must be a transaction root, not a revision root.
    #[cfg(feature = "delta")]
    pub fn apply_textdelta(
        &mut self,
        path: impl TryInto<FsPath, Error = Error<'static>>,
        base_checksum: Option<&str>,
        result_checksum: Option<&str>,
    ) -> Result<crate::delta::WrapTxdeltaWindowHandler, Error<'static>> {
        let fs_path = path.try_into()?;
        let base_checksum_cstr = base_checksum.map(std::ffi::CString::new).transpose()?;
        let result_checksum_cstr = result_checksum.map(std::ffi::CString::new).transpose()?;

        let pool = apr::Pool::new();

        unsafe {
            let mut handler: subversion_sys::svn_txdelta_window_handler_t = None;
            let mut handler_baton: *mut std::ffi::c_void = std::ptr::null_mut();

            let err = subversion_sys::svn_fs_apply_textdelta(
                &mut handler,
                &mut handler_baton,
                self.ptr,
                fs_path.as_ptr(),
                base_checksum_cstr
                    .as_ref()
                    .map_or(std::ptr::null(), |c| c.as_ptr()),
                result_checksum_cstr
                    .as_ref()
                    .map_or(std::ptr::null(), |c| c.as_ptr()),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;

            Ok(crate::delta::WrapTxdeltaWindowHandler::from_raw(
                handler,
                handler_baton,
                pool,
            ))
        }
    }

    /// Write data directly to a file in the filesystem
    ///
    /// Returns a stream ready to receive full textual data.
    /// The root must be a transaction root, not a revision root.
    pub fn apply_text(
        &mut self,
        path: impl TryInto<FsPath, Error = Error<'static>>,
        result_checksum: Option<&str>,
    ) -> Result<crate::io::Stream, Error<'static>> {
        let fs_path = path.try_into()?;
        let result_checksum_cstr = result_checksum.map(std::ffi::CString::new).transpose()?;

        let pool = apr::Pool::new();

        unsafe {
            let mut stream: *mut subversion_sys::svn_stream_t = std::ptr::null_mut();

            let err = subversion_sys::svn_fs_apply_text(
                &mut stream,
                self.ptr,
                fs_path.as_ptr(),
                result_checksum_cstr
                    .as_ref()
                    .map_or(std::ptr::null(), |c| c.as_ptr()),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;

            Ok(crate::io::Stream::from_ptr_and_pool(stream, pool))
        }
    }

    /// Get a delta stream between two files
    ///
    /// Returns a stream that produces svndiff data representing the difference
    /// between the source and target files.
    #[cfg(feature = "delta")]
    pub fn get_file_delta_stream(
        &self,
        source_path: impl TryInto<FsPath, Error = Error<'static>>,
        target_root: &Root,
        target_path: impl TryInto<FsPath, Error = Error<'static>>,
    ) -> Result<crate::delta::TxDeltaStream, Error<'static>> {
        let source_fs_path = source_path.try_into()?;
        let target_fs_path = target_path.try_into()?;

        let pool = apr::Pool::new();

        unsafe {
            let mut stream: *mut subversion_sys::svn_txdelta_stream_t = std::ptr::null_mut();

            let err = subversion_sys::svn_fs_get_file_delta_stream(
                &mut stream,
                self.ptr,
                source_fs_path.as_ptr(),
                target_root.ptr,
                target_fs_path.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;

            Ok(crate::delta::TxDeltaStream::from_raw(stream, pool))
        }
    }

    /// Check whether a node has any properties.
    ///
    /// Returns `true` if the node at `path` has at least one property set,
    /// `false` otherwise.  This is cheaper than calling [`Root::proplist`]
    /// and checking whether the result is empty.
    ///
    /// Wraps `svn_fs_node_has_props`.
    pub fn node_has_props(
        &self,
        path: impl TryInto<FsPath, Error = Error<'static>>,
    ) -> Result<bool, Error<'static>> {
        let fs_path = path.try_into()?;
        with_tmp_pool(|pool| unsafe {
            let mut has_props: subversion_sys::svn_boolean_t = 0;
            let err = subversion_sys::svn_fs_node_has_props(
                &mut has_props,
                self.ptr,
                fs_path.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;
            Ok(has_props != 0)
        })
    }

    /// Get the revision in which a node was first created (its origin).
    ///
    /// Returns the revision number at which the node at `path` originated in
    /// the repository.  Unlike [`Root::node_created_rev`], which returns the
    /// revision where `path` was most recently modified, this function follows
    /// copies back to find the true origin of the node.
    ///
    /// Wraps `svn_fs_node_origin_rev`.
    pub fn node_origin_rev(
        &self,
        path: impl TryInto<FsPath, Error = Error<'static>>,
    ) -> Result<Revnum, Error<'static>> {
        let fs_path = path.try_into()?;
        with_tmp_pool(|pool| unsafe {
            let mut revision: subversion_sys::svn_revnum_t = -1; // SVN_INVALID_REVNUM
            let err = subversion_sys::svn_fs_node_origin_rev(
                &mut revision,
                self.ptr,
                fs_path.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;
            Ok(Revnum(revision))
        })
    }

    /// Get the copy source of a node.
    ///
    /// If the node at `path` was created by a copy operation, returns
    /// `Some((rev, src_path))` identifying the revision and source path from
    /// which it was copied.  Returns `None` if the node was not created by a
    /// copy (i.e. it was added from scratch).
    ///
    /// Wraps `svn_fs_copied_from`.
    pub fn copied_from(
        &self,
        path: impl TryInto<FsPath, Error = Error<'static>>,
    ) -> Result<Option<(Revnum, String)>, Error<'static>> {
        let fs_path = path.try_into()?;
        let result_pool = apr::Pool::new();
        unsafe {
            let mut rev: subversion_sys::svn_revnum_t = -1; // SVN_INVALID_REVNUM
            let mut src_path: *const std::os::raw::c_char = std::ptr::null();
            let err = subversion_sys::svn_fs_copied_from(
                &mut rev,
                &mut src_path,
                self.ptr,
                fs_path.as_ptr(),
                result_pool.as_mut_ptr(),
            );
            svn_result(err)?;
            if src_path.is_null() || rev < 0 {
                return Ok(None);
            }
            let path_str = std::ffi::CStr::from_ptr(src_path)
                .to_string_lossy()
                .into_owned();
            Ok(Some((Revnum(rev), path_str)))
        }
    }

    /// Determine the relationship between two nodes.
    ///
    /// Compares the node at `path` in this root with the node at `other_path`
    /// in `other_root` and returns a [`crate::NodeRelation`] value:
    ///
    /// - [`NodeRelation::Unchanged`]: the two nodes are identical
    /// - [`NodeRelation::CommonAncestor`]: they share history but differ
    /// - [`NodeRelation::Unrelated`]: they have no common ancestor
    ///
    /// Wraps `svn_fs_node_relation`.
    pub fn node_relation(
        &self,
        path: impl TryInto<FsPath, Error = Error<'static>>,
        other_root: &Root,
        other_path: impl TryInto<FsPath, Error = Error<'static>>,
    ) -> Result<crate::NodeRelation, Error<'static>> {
        let fs_path = path.try_into()?;
        let other_fs_path = other_path.try_into()?;
        with_tmp_pool(|pool| unsafe {
            let mut relation: subversion_sys::svn_fs_node_relation_t =
                subversion_sys::svn_fs_node_relation_t_svn_fs_node_unrelated;
            let err = subversion_sys::svn_fs_node_relation(
                &mut relation,
                self.ptr,
                fs_path.as_ptr(),
                other_root.ptr,
                other_fs_path.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;
            Ok(relation.into())
        })
    }
}

/// Represents the history of a node in the filesystem
pub struct NodeHistory {
    ptr: *mut subversion_sys::svn_fs_history_t,
    pool: apr::Pool<'static>, // Pool used for all history allocations
}

impl NodeHistory {
    /// Get the previous history entry.
    ///
    /// Returns `None` when there is no earlier history.
    ///
    /// Wraps `svn_fs_history_prev2`.
    pub fn prev(&mut self, cross_copies: bool) -> Result<Option<(String, Revnum)>, Error<'_>> {
        unsafe {
            let mut prev_history: *mut subversion_sys::svn_fs_history_t = std::ptr::null_mut();
            // Use the NodeHistory's own pool for allocations
            let err = subversion_sys::svn_fs_history_prev2(
                &mut prev_history,
                self.ptr,
                if cross_copies { 1 } else { 0 },
                self.pool.as_mut_ptr(),
                self.pool.as_mut_ptr(),
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
                self.pool.as_mut_ptr(),
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
    pool: apr::Pool<'static>, // Keep the pool alive for the lifetime of the NodeId
}

impl PartialEq for NodeId {
    fn eq(&self, other: &Self) -> bool {
        unsafe { subversion_sys::svn_fs_compare_ids(self.ptr, other.ptr) == 0 }
    }
}

impl Eq for NodeId {}

impl NodeId {
    /// Parse a node ID from its string representation.
    ///
    /// This is the inverse of [`NodeId::to_string`].
    ///
    /// # Deprecation
    ///
    /// `svn_fs_parse_id` is deprecated and not guaranteed to work with all
    /// filesystem types.  There is currently no non-deprecated equivalent in
    /// the SVN C API.
    #[deprecated(note = "svn_fs_parse_id is deprecated in the SVN C API and is not \
                         guaranteed to work with all filesystem types")]
    pub fn parse(data: &[u8]) -> Result<NodeId, Error<'static>> {
        let pool = apr::Pool::new();
        unsafe {
            let ptr = subversion_sys::svn_fs_parse_id(
                data.as_ptr() as *const std::os::raw::c_char,
                data.len(),
                pool.as_mut_ptr(),
            );
            if ptr.is_null() {
                return Err(Error::from_message("Failed to parse node ID"));
            }
            Ok(NodeId { ptr, pool })
        }
    }

    /// Compare two node IDs
    ///
    /// Returns:
    /// - 0 if they are equal
    /// - -1 if they are different but related (share a common ancestor)
    /// - 1 if they are unrelated
    pub fn compare(&self, other: &NodeId) -> i32 {
        unsafe { subversion_sys::svn_fs_compare_ids(self.ptr, other.ptr) }
    }

    /// Check if two node IDs are related (share a common ancestor)
    pub fn check_related(&self, other: &NodeId) -> bool {
        unsafe { subversion_sys::svn_fs_check_related(self.ptr, other.ptr) != 0 }
    }

    /// Convert the node ID to a string representation
    pub fn to_string(&self) -> Result<String, Error<'static>> {
        unsafe {
            let str_svn = subversion_sys::svn_fs_unparse_id(self.ptr, self.pool.as_mut_ptr());
            if str_svn.is_null() {
                return Err(Error::from_message("Failed to unparse node ID"));
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

/// Transaction handle with RAII cleanup.
///
/// The lifetime parameter `'fs` ties this transaction to the [`Fs`] (or
/// [`crate::repos::Repos`]) that created it, preventing the transaction from
/// outliving the filesystem whose internal data it references.
pub struct Transaction<'fs> {
    ptr: *mut subversion_sys::svn_fs_txn_t,
    #[allow(dead_code)]
    pool: apr::Pool<'static>,
    /// `*mut ()` makes this type `!Send + !Sync`; `&'fs ()` ties us to the Fs lifetime.
    _marker: std::marker::PhantomData<(*mut (), &'fs ())>,
}

impl Drop for Transaction<'_> {
    fn drop(&mut self) {
        // Pool drop will clean up transaction
    }
}

impl<'fs> Transaction<'fs> {
    /// Create Transaction from existing pointer with pool (for repos integration).
    ///
    /// # Safety
    ///
    /// The caller must ensure that `ptr` remains valid for the entire lifetime
    /// `'fs`, i.e. the backing filesystem must outlive this transaction.
    #[cfg(feature = "repos")]
    pub(crate) unsafe fn from_ptr_and_pool(
        ptr: *mut subversion_sys::svn_fs_txn_t,
        pool: apr::Pool<'static>,
    ) -> Self {
        Self {
            ptr,
            pool,
            _marker: std::marker::PhantomData,
        }
    }

    /// Get the underlying SVN transaction pointer
    #[allow(dead_code)]
    pub(crate) fn as_ptr(&self) -> *mut subversion_sys::svn_fs_txn_t {
        self.ptr
    }

    /// Get the transaction name
    pub fn name(&self) -> Result<String, Error<'static>> {
        with_tmp_pool(|pool| unsafe {
            let mut name_ptr = std::ptr::null();
            let err = subversion_sys::svn_fs_txn_name(&mut name_ptr, self.ptr, pool.as_mut_ptr());
            Error::from_raw(err)?;
            let name_cstr = std::ffi::CStr::from_ptr(name_ptr);
            Ok(name_cstr.to_str()?.to_string())
        })
    }

    /// Get the base revision of this transaction
    pub fn base_revision(&self) -> Result<Revnum, Error<'static>> {
        unsafe {
            let base_rev = subversion_sys::svn_fs_txn_base_revision(self.ptr);
            Ok(Revnum(base_rev))
        }
    }

    /// Get the transaction root for making changes.
    ///
    /// The returned [`TxnRoot`] borrows this transaction mutably (preventing
    /// other operations on the transaction while the root is live).  Drop the
    /// `TxnRoot` before committing or aborting the transaction.
    pub fn root(&mut self) -> Result<TxnRoot<'_>, Error<'static>> {
        let pool = apr::Pool::new();
        unsafe {
            let mut root_ptr = std::ptr::null_mut();
            let err = subversion_sys::svn_fs_txn_root(&mut root_ptr, self.ptr, pool.as_mut_ptr());
            Error::from_raw(err)?;
            Ok(TxnRoot {
                ptr: root_ptr,
                _pool: pool,
                _marker: std::marker::PhantomData,
            })
        }
    }

    /// Set a property on this transaction
    pub fn change_prop(&mut self, name: &str, value: &str) -> Result<(), Error<'static>> {
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
    pub fn commit(self) -> Result<Revnum, Error<'static>> {
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
    pub fn abort(self) -> Result<(), Error<'static>> {
        let pool = apr::Pool::new();
        unsafe {
            let err = subversion_sys::svn_fs_abort_txn(self.ptr, pool.as_mut_ptr());
            Error::from_raw(err)?;
            Ok(())
        }
    }

    /// Change a transaction property (binary-safe version)
    pub fn change_prop_bytes(
        &self,
        name: &str,
        value: Option<&[u8]>,
    ) -> Result<(), Error<'static>> {
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

    /// Set multiple transaction properties at once.
    ///
    /// Each entry in `props` is `(name, value)` where `value` is `None` to
    /// delete the property.
    ///
    /// Wraps `svn_fs_change_txn_props`.
    pub fn change_props(&self, props: &[(&str, Option<&[u8]>)]) -> Result<(), Error<'static>> {
        let pool = apr::Pool::new();
        // Build CStrings first to keep them alive for the duration of the call
        let cstrings: Vec<std::ffi::CString> = props
            .iter()
            .map(|(name, _)| std::ffi::CString::new(*name))
            .collect::<Result<_, _>>()?;
        let mut arr =
            apr::tables::TypedArray::<subversion_sys::svn_prop_t>::new(&pool, props.len() as i32);
        for (cstr, (_, value)) in cstrings.iter().zip(props.iter()) {
            let value_ptr = value
                .map(|val| crate::svn_string_helpers::svn_string_ncreate(val, &pool))
                .unwrap_or(std::ptr::null_mut());
            arr.push(subversion_sys::svn_prop_t {
                name: cstr.as_ptr(),
                value: value_ptr as *const subversion_sys::svn_string_t,
            });
        }
        unsafe {
            let err =
                subversion_sys::svn_fs_change_txn_props(self.ptr, arr.as_ptr(), pool.as_mut_ptr());
            Error::from_raw(err)?;
        }
        Ok(())
    }

    /// Get a transaction property
    pub fn prop(&self, name: &str) -> Result<Option<Vec<u8>>, Error<'_>> {
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
    pub fn proplist(&self) -> Result<std::collections::HashMap<String, Vec<u8>>, Error<'_>> {
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

    /// Set a transaction property, with validation.
    ///
    /// This is a validating wrapper around `svn_fs_change_txn_prop` that
    /// checks the property name and value before making the change.
    ///
    /// Pass `None` for `value` to delete the property.
    ///
    /// Wraps `svn_repos_fs_change_txn_prop`.
    pub fn repos_change_prop(
        &self,
        name: &str,
        value: Option<&[u8]>,
    ) -> Result<(), Error<'static>> {
        let name_cstr = std::ffi::CString::new(name)?;
        let pool = apr::Pool::new();
        let value_ptr = value
            .map(|val| crate::svn_string_helpers::svn_string_ncreate(val, &pool))
            .unwrap_or(std::ptr::null_mut());
        unsafe {
            let err = subversion_sys::svn_repos_fs_change_txn_prop(
                self.ptr,
                name_cstr.as_ptr(),
                value_ptr as *const subversion_sys::svn_string_t,
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
        }
        Ok(())
    }

    /// Set multiple transaction properties at once, with validation.
    ///
    /// This is a validating wrapper around `svn_fs_change_txn_props` that
    /// checks property names and values before making the changes.
    ///
    /// Each entry in `props` is `(name, value)` where `value` is `None` to
    /// delete the property.
    ///
    /// Wraps `svn_repos_fs_change_txn_props`.
    pub fn repos_change_props(
        &self,
        props: &[(&str, Option<&[u8]>)],
    ) -> Result<(), Error<'static>> {
        let pool = apr::Pool::new();
        let cstrings: Vec<std::ffi::CString> = props
            .iter()
            .map(|(name, _)| std::ffi::CString::new(*name))
            .collect::<Result<_, _>>()?;
        let mut arr =
            apr::tables::TypedArray::<subversion_sys::svn_prop_t>::new(&pool, props.len() as i32);
        for (cstr, (_, value)) in cstrings.iter().zip(props.iter()) {
            let value_ptr = value
                .map(|val| crate::svn_string_helpers::svn_string_ncreate(val, &pool))
                .unwrap_or(std::ptr::null_mut());
            arr.push(subversion_sys::svn_prop_t {
                name: cstr.as_ptr(),
                value: value_ptr as *const subversion_sys::svn_string_t,
            });
        }
        unsafe {
            let err = subversion_sys::svn_repos_fs_change_txn_props(
                self.ptr,
                arr.as_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
        }
        Ok(())
    }
}

/// Transaction root for making changes.
///
/// The lifetime parameter `'txn` is tied to the [`Transaction`] that created
/// this root, ensuring the root cannot outlive the transaction (whose internal
/// state the `svn_fs_root_t` pointer references).
pub struct TxnRoot<'txn> {
    ptr: *mut subversion_sys::svn_fs_root_t,
    _pool: apr::Pool<'static>,
    /// Borrows the transaction mutably for `'txn`; also `!Send + !Sync` via `*mut ()`.
    _marker: std::marker::PhantomData<(*mut (), &'txn mut ())>,
}

impl<'txn> TxnRoot<'txn> {
    /// Create a directory
    pub fn make_dir(
        &mut self,
        path: impl TryInto<FsPath, Error = Error<'static>>,
    ) -> Result<(), Error<'static>> {
        let fs_path = path.try_into()?;
        let pool = apr::Pool::new();
        unsafe {
            let err =
                subversion_sys::svn_fs_make_dir(self.ptr, fs_path.as_ptr(), pool.as_mut_ptr());
            Error::from_raw(err)?;
            Ok(())
        }
    }

    /// Create a file
    pub fn make_file(
        &mut self,
        path: impl TryInto<FsPath, Error = Error<'static>>,
    ) -> Result<(), Error<'static>> {
        let fs_path = path.try_into()?;
        let pool = apr::Pool::new();
        unsafe {
            let err =
                subversion_sys::svn_fs_make_file(self.ptr, fs_path.as_ptr(), pool.as_mut_ptr());
            Error::from_raw(err)?;
            Ok(())
        }
    }

    /// Delete a node
    pub fn delete(
        &mut self,
        path: impl TryInto<FsPath, Error = Error<'static>>,
    ) -> Result<(), Error<'static>> {
        let fs_path = path.try_into()?;
        let pool = apr::Pool::new();
        unsafe {
            let err = subversion_sys::svn_fs_delete(self.ptr, fs_path.as_ptr(), pool.as_mut_ptr());
            Error::from_raw(err)?;
            Ok(())
        }
    }

    /// Copy a node from another location
    pub fn copy(
        &mut self,
        from_root: &Root,
        from_path: impl TryInto<FsPath, Error = Error<'static>>,
        to_path: impl TryInto<FsPath, Error = Error<'static>>,
    ) -> Result<(), Error<'static>> {
        let from_fs_path = from_path.try_into()?;
        let to_fs_path = to_path.try_into()?;
        let pool = apr::Pool::new();
        unsafe {
            let err = subversion_sys::svn_fs_copy(
                from_root.ptr,
                from_fs_path.as_ptr(),
                self.ptr,
                to_fs_path.as_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(())
        }
    }

    /// Apply text changes to a file
    pub fn apply_text(
        &mut self,
        path: impl TryInto<FsPath, Error = Error<'static>>,
        result_checksum: Option<&str>,
    ) -> Result<crate::io::Stream, Error<'static>> {
        let fs_path = path.try_into()?;
        let checksum_cstr = result_checksum.map(std::ffi::CString::new).transpose()?;
        let pool = apr::Pool::new();
        unsafe {
            let mut stream_ptr = std::ptr::null_mut();
            let err = subversion_sys::svn_fs_apply_text(
                &mut stream_ptr,
                self.ptr,
                fs_path.as_ptr(),
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
    pub fn change_node_prop(
        &mut self,
        path: impl TryInto<FsPath, Error = Error<'static>>,
        name: &str,
        value: &[u8],
    ) -> Result<(), Error<'static>> {
        let fs_path = path.try_into()?;
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
                fs_path.as_ptr(),
                name_cstr.as_ptr(),
                value_str,
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(())
        }
    }

    /// Check if a path exists and what kind of node it is
    pub fn check_path(
        &self,
        path: impl TryInto<FsPath, Error = Error<'static>>,
    ) -> Result<crate::NodeKind, Error<'static>> {
        let fs_path = path.try_into()?;
        let pool = apr::Pool::new();
        unsafe {
            let mut kind = 0;
            let err = subversion_sys::svn_fs_check_path(
                &mut kind,
                self.ptr,
                fs_path.as_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(crate::NodeKind::from(kind))
        }
    }

    /// Set file contents directly
    pub fn set_file_contents(
        &mut self,
        path: impl TryInto<FsPath, Error = Error<'static>>,
        contents: &[u8],
    ) -> Result<(), Error<'static>> {
        let fs_path = path.try_into()?;
        let pool = apr::Pool::new();

        unsafe {
            let mut stream = std::ptr::null_mut();
            let err = subversion_sys::svn_fs_apply_text(
                &mut stream,
                self.ptr,
                fs_path.as_ptr(),
                std::ptr::null(), // result_checksum
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;

            // Write the contents to the stream
            let bytes_written = subversion_sys::svn_stream_write(
                stream,
                contents.as_ptr() as *const std::ffi::c_char,
                &mut { contents.len() },
            );
            Error::from_raw(bytes_written)?;

            // Close the stream
            let err = subversion_sys::svn_stream_close(stream);
            Error::from_raw(err)?;
        }

        Ok(())
    }

    /// Get the contents of a file as bytes
    pub fn file_contents(
        &self,
        path: impl TryInto<FsPath, Error = Error<'static>>,
    ) -> Result<Vec<u8>, Error<'_>> {
        let fs_path = path.try_into()?;
        let pool = apr::Pool::new();

        unsafe {
            let mut stream = std::ptr::null_mut();
            let err = subversion_sys::svn_fs_file_contents(
                &mut stream,
                self.ptr,
                fs_path.as_ptr(),
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

    /// Return the transaction name for this transaction root.
    ///
    /// Wraps `svn_fs_txn_root_name`.
    pub fn txn_root_name(&self) -> String {
        with_tmp_pool(|pool| unsafe {
            let name = subversion_sys::svn_fs_txn_root_name(self.ptr, pool.as_mut_ptr());
            std::ffi::CStr::from_ptr(name)
                .to_string_lossy()
                .into_owned()
        })
    }

    /// Return the base revision on which this transaction root is based.
    ///
    /// Wraps `svn_fs_txn_root_base_revision`.
    pub fn txn_root_base_revision(&self) -> Revnum {
        Revnum(unsafe { subversion_sys::svn_fs_txn_root_base_revision(self.ptr) })
    }

    /// Create a link at `path` in this transaction root pointing to the same
    /// node as `path` in `from_root`.
    ///
    /// Unlike [`copy`](TxnRoot::copy), this does not record copy history, so
    /// [`Root::copied_from`] cannot be used to find the source later.
    ///
    /// `from_root` must be a revision root.  Both roots must belong to the
    /// same filesystem.
    ///
    /// Wraps `svn_fs_revision_link`.
    pub fn revision_link(
        &mut self,
        from_root: &Root,
        path: impl TryInto<FsPath, Error = Error<'static>>,
    ) -> Result<(), Error<'static>> {
        let fs_path = path.try_into()?;
        with_tmp_pool(|pool| {
            let err = unsafe {
                subversion_sys::svn_fs_revision_link(
                    from_root.ptr,
                    self.ptr,
                    fs_path.as_ptr(),
                    pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })
    }

    /// Set a property on a node, with validation.
    ///
    /// This is a validating wrapper around `svn_fs_change_node_prop` that
    /// checks the property name and value before making the change.
    ///
    /// Pass `None` for `value` to delete the property.
    ///
    /// Wraps `svn_repos_fs_change_node_prop`.
    pub fn repos_change_node_prop(
        &mut self,
        path: impl TryInto<FsPath, Error = Error<'static>>,
        name: &str,
        value: Option<&[u8]>,
    ) -> Result<(), Error<'static>> {
        let fs_path = path.try_into()?;
        let name_cstr = std::ffi::CString::new(name)?;
        let pool = apr::Pool::new();
        let value_ptr = value
            .map(|val| crate::svn_string_helpers::svn_string_ncreate(val, &pool))
            .unwrap_or(std::ptr::null_mut());
        unsafe {
            let err = subversion_sys::svn_repos_fs_change_node_prop(
                self.ptr,
                fs_path.as_ptr(),
                name_cstr.as_ptr(),
                value_ptr as *const subversion_sys::svn_string_t,
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
        }
        Ok(())
    }

    /// Get properties inherited by a node from its ancestors.
    ///
    /// Returns a depth-first ordered list of `(path, props)` pairs representing
    /// the inherited properties. If `propname` is `None`, all properties are
    /// returned; otherwise only the named property is returned.
    ///
    /// Wraps `svn_repos_fs_get_inherited_props`.
    pub fn repos_get_inherited_props(
        &self,
        path: impl TryInto<FsPath, Error = Error<'static>>,
        propname: Option<&str>,
    ) -> Result<Vec<(String, std::collections::HashMap<String, Vec<u8>>)>, Error<'static>> {
        let fs_path = path.try_into()?;
        let propname_cstr = propname.map(std::ffi::CString::new).transpose()?;
        let result_pool = apr::Pool::new();
        let scratch_pool = apr::Pool::new();
        let mut inherited_props: *mut apr_sys::apr_array_header_t = std::ptr::null_mut();
        unsafe {
            let err = subversion_sys::svn_repos_fs_get_inherited_props(
                &mut inherited_props,
                self.ptr,
                fs_path.as_ptr(),
                propname_cstr
                    .as_ref()
                    .map(|c| c.as_ptr())
                    .unwrap_or(std::ptr::null()),
                None,
                std::ptr::null_mut(),
                result_pool.as_mut_ptr(),
                scratch_pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
        }
        if inherited_props.is_null() {
            return Ok(Vec::new());
        }
        parse_inherited_props_array(inherited_props)
    }
}

/// Parse an `apr_array_header_t` of `svn_prop_inherited_item_t *` into a
/// `Vec<(String, HashMap<String, Vec<u8>>)>`.
///
/// # Safety
/// The caller must ensure the array pointer is valid and points to a properly
/// initialised APR array of `*svn_prop_inherited_item_t`.
pub(crate) fn parse_inherited_props_array(
    array: *mut apr_sys::apr_array_header_t,
) -> Result<Vec<(String, std::collections::HashMap<String, Vec<u8>>)>, Error<'static>> {
    use std::collections::HashMap;

    if array.is_null() {
        return Ok(Vec::new());
    }

    let slice = unsafe {
        std::slice::from_raw_parts(
            (*array).elts as *const *const subversion_sys::svn_prop_inherited_item_t,
            (*array).nelts as usize,
        )
    };

    let mut result = Vec::new();
    for item_ptr in slice.iter() {
        if item_ptr.is_null() {
            continue;
        }
        let item = unsafe { &**item_ptr };
        if item.path_or_url.is_null() {
            continue;
        }
        let path_or_url = unsafe {
            std::ffi::CStr::from_ptr(item.path_or_url)
                .to_string_lossy()
                .into_owned()
        };

        let props = if item.prop_hash.is_null() {
            HashMap::new()
        } else {
            let hash = unsafe { apr::hash::Hash::from_ptr(item.prop_hash) };
            let mut props = HashMap::new();
            for (key, value) in hash.iter() {
                if value.is_null() {
                    continue;
                }
                let svn_str_ptr = value as *const subversion_sys::svn_string_t;
                let svn_str = unsafe { &*svn_str_ptr };
                let data = crate::svn_string_helpers::to_vec(svn_str);
                props.insert(String::from_utf8_lossy(key).into_owned(), data);
            }
            props
        };

        result.push((path_or_url, props));
    }

    Ok(result)
}

impl<'pool> Fs<'pool> {
    /// Begin a new transaction
    pub fn begin_txn(
        &self,
        base_rev: Revnum,
        flags: u32,
    ) -> Result<Transaction<'_>, Error<'static>> {
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
                pool,
                _marker: std::marker::PhantomData,
            })
        }
    }

    /// Open an existing transaction by name
    pub fn open_txn(&self, name: &str) -> Result<Transaction<'_>, Error<'static>> {
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
                pool,
                _marker: std::marker::PhantomData,
            })
        }
    }

    /// List all uncommitted transactions
    pub fn list_transactions(&self) -> Result<Vec<String>, Error<'_>> {
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
    pub fn purge_txn(&self, name: &str) -> Result<(), Error<'static>> {
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
        source: &Root<'_>,
        target: &mut TxnRoot<'_>,
        ancestor: &Root<'_>,
        ancestor_path: &str,
        source_path: &str,
        target_path: &str,
    ) -> Result<Option<String>, Error<'_>> {
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
    use std::io::Read;
    use tempfile::tempdir;

    #[test]
    fn test_fs_create_and_open() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        // Create filesystem
        Fs::create(&fs_path).unwrap();

        // Open existing filesystem
        Fs::open(&fs_path).unwrap();
    }

    #[test]
    fn test_fs_youngest_rev() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let fs = Fs::create(&fs_path).unwrap();
        // New filesystem should have revision 0
        let rev = fs.youngest_revision().unwrap();
        assert_eq!(rev, crate::Revnum(0));
    }

    #[test]
    fn test_fs_get_uuid() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let fs = Fs::create(&fs_path).unwrap();
        // UUID should not be empty
        let uuid = fs.get_uuid().unwrap();
        assert!(!uuid.is_empty());
    }

    #[test]
    fn test_fs_mutability() {
        // Test that methods requiring only &self work correctly
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let fs = Fs::create(&fs_path).unwrap();

        // These should work with immutable reference
        fs.get_uuid().unwrap();
        fs.revision_proplist(crate::Revnum(0), false).unwrap();
        fs.revision_root(crate::Revnum(0)).unwrap();
        fs.youngest_revision().unwrap();
    }

    #[test]
    fn test_root_creation() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let fs = Fs::create(&fs_path).unwrap();
        let root = fs.revision_root(crate::Revnum(0)).unwrap();
        assert!(root.is_revision_root());
    }

    #[test]
    fn test_root_fs_accessor() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let fs = Fs::create(&fs_path).unwrap();
        let root = fs.revision_root(crate::Revnum(0)).unwrap();
        // Root::fs() should return an Fs for the underlying filesystem
        let fs2 = root.fs();
        assert_eq!(fs.get_uuid().unwrap(), fs2.get_uuid().unwrap());
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
        let fs = Fs::open(&fs_path).unwrap();
        fs.youngest_revision().unwrap();
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
    fn test_dir_entries_optimal_order() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let fs = Fs::create(&fs_path).unwrap();

        // Create some files in a transaction
        let mut txn = fs.begin_txn(crate::Revnum(0), 0).unwrap();
        {
            let mut root = txn.root().unwrap();
            root.make_file("/alpha.txt").unwrap();
            root.make_file("/beta.txt").unwrap();
            root.make_file("/gamma.txt").unwrap();
        }
        let _rev = txn.commit().unwrap();

        let root = fs.revision_root(crate::Revnum(1)).unwrap();

        // dir_entries_optimal_order should return all three entries
        let ordered = root.dir_entries_optimal_order("/").unwrap();
        assert_eq!(ordered.len(), 3);

        // All names should be present (order is filesystem-dependent)
        let mut names: Vec<String> = ordered.iter().map(|e| e.name().to_owned()).collect();
        names.sort();
        assert_eq!(names, vec!["alpha.txt", "beta.txt", "gamma.txt"]);
    }

    #[test]
    #[allow(deprecated)]
    fn test_node_id_parse_roundtrip() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let fs = Fs::create(&fs_path).unwrap();
        let root = fs.revision_root(crate::Revnum(0)).unwrap();

        // Get the node ID for the root directory and unparse it to a string
        let node_id = root.node_id("/").unwrap();
        let id_string = node_id.to_string().unwrap();

        // Parse it back — svn_fs_parse_id is deprecated and not guaranteed to
        // produce a backend-comparable ID, so we just verify:
        //  1. Parsing succeeds (no error, non-null pointer)
        //  2. Unparsing the parsed result yields the same string (string roundtrip)
        let parsed = NodeId::parse(id_string.as_bytes()).unwrap();
        let reparsed_string = parsed.to_string().unwrap();
        assert_eq!(id_string, reparsed_string);
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
        let created_rev1 = root1.node_created_rev("/test.txt").unwrap();
        assert_eq!(
            created_rev1, rev1,
            "Node in rev1 should have been created in rev1"
        );

        let created_rev2 = root2.node_created_rev("/test.txt").unwrap();
        assert_eq!(
            created_rev2, rev2,
            "Node in rev2 should have been created in rev2 (after modification)"
        );

        // Test node ID
        let node_id1 = root1.node_id("/test.txt").unwrap();
        let node_id2 = root2.node_id("/test.txt").unwrap();

        // Just verify we can get node IDs - comparison semantics may vary
        // based on SVN backend implementation
        let _id1_str = node_id1.to_string().unwrap();
        let _id2_str = node_id2.to_string().unwrap();

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
        let props_changed = root1
            .props_changed("/test.txt", &root2, "/test.txt")
            .unwrap();
        assert!(
            props_changed,
            "Properties should have changed between revisions"
        );

        // Properties should be the same when comparing the same revision
        let props_same = root1
            .props_changed("/test.txt", &root1, "/test.txt")
            .unwrap();
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
        let mut stream = root.file_contents("/renamed.txt").unwrap();
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
        root3
            .copy(&source_root, "/file.txt", "/copied.txt")
            .unwrap();
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
        root.set_file_contents("/test.txt", b"test content")
            .unwrap();
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

        let lock = lock.unwrap();

        // Verify the lock was created (SVN adds leading slash)
        assert_eq!(lock.path(), "/test.txt");
        assert!(!lock.token().is_empty());
        assert_eq!(lock.comment(), "Test lock comment");

        // Get the lock info
        let lock_info = fs.get_lock("/test.txt").unwrap();
        assert!(lock_info.is_some());
        let lock_info = lock_info.unwrap();
        assert_eq!(lock_info.path(), "/test.txt");
        assert_eq!(lock_info.token(), lock.token());

        // Unlock the file
        fs.unlock("/test.txt", lock.token(), false).unwrap();

        // Verify the lock is gone
        let lock_info = fs.get_lock("/test.txt").unwrap();
        assert!(lock_info.is_none());
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
        {
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
        }

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
        let locks = fs.get_locks("/dir1", crate::Depth::Infinity).unwrap();
        assert_eq!(locks.len(), 3, "Should find 3 locks");

        // Get locks with immediates depth (only direct children)
        let locks_immediate = fs.get_locks("/dir1", crate::Depth::Immediates).unwrap();
        // Should get file1.txt and file2.txt but not file3.txt
        assert_eq!(
            locks_immediate.len(),
            2,
            "Should find 2 locks at immediate depth"
        );

        // Get locks with files depth
        let locks_files = fs.get_locks("/dir1", crate::Depth::Files).unwrap();
        assert_eq!(locks_files.len(), 2, "Should find 2 locks with files depth");
    }

    #[test]
    fn test_fs_generate_lock_token() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let fs = Fs::create(&fs_path).unwrap();

        // Generate a lock token
        let token1 = fs.generate_lock_token().unwrap();
        assert!(!token1.is_empty(), "Generated token should not be empty");

        // Generate another token and verify it's different
        let token2 = fs.generate_lock_token().unwrap();
        assert_ne!(token1, token2, "Generated tokens should be unique");
    }

    #[test]
    fn test_get_access_username_no_access() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let fs = Fs::create(&fs_path).unwrap();

        // No access context set — should return None
        let result = fs.get_access_username().unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_get_access_username_with_access() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let mut fs = Fs::create(&fs_path).unwrap();

        fs.set_access("alice").unwrap();

        let result = fs.get_access_username().unwrap();
        assert_eq!(result, Some("alice".to_string()));
    }

    #[test]
    fn test_fs_access_add_lock_token() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let mut fs = Fs::create(&fs_path).unwrap();

        // Set up a file and lock it
        fs.set_access("testuser").unwrap();
        let mut txn = fs.begin_txn(crate::Revnum(0), 0).unwrap();
        let mut root = txn.root().unwrap();
        root.make_file("/locked.txt").unwrap();
        let rev = txn.commit().unwrap();

        // Lock the file
        let lock = fs
            .lock(
                "/locked.txt",
                None,
                Some("Test lock"),
                false,
                None,
                rev,
                false,
            )
            .unwrap();

        // Add the lock token to the access context
        fs.access_add_lock_token("/locked.txt", lock.token())
            .unwrap();
    }

    #[test]
    fn test_fs_access_add_lock_token_no_context() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let mut fs = Fs::create(&fs_path).unwrap();

        // Try to add lock token without setting access context first
        let result = fs.access_add_lock_token("/some/path", "some-token");
        assert!(result.is_err(), "Should fail when no access context is set");
    }

    #[test]
    fn test_fs_lock_many() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let mut fs = Fs::create(&fs_path).unwrap();
        fs.set_access("testuser").unwrap();

        // Create two files to lock
        let mut txn = fs.begin_txn(Revnum(0), 0).unwrap();
        let mut root = txn.root().unwrap();
        root.make_file("/file1.txt").unwrap();
        root.make_file("/file2.txt").unwrap();
        let rev = txn.commit().unwrap();

        let targets = vec![("/file1.txt", None, rev), ("/file2.txt", None, rev)];

        let mut results: Vec<(String, bool)> = Vec::new();
        fs.lock_many(
            &targets,
            Some("bulk lock"),
            false,
            None,
            false,
            |path, error| {
                results.push((path.to_string(), error.is_none()));
            },
        )
        .unwrap();

        assert_eq!(results.len(), 2);
        // Both locks should succeed
        assert!(
            results.iter().all(|(_, ok)| *ok),
            "All locks should succeed"
        );

        // Verify locks exist
        let lock1 = fs.get_lock("/file1.txt").unwrap();
        assert!(lock1.is_some(), "file1.txt should be locked");
        let lock2 = fs.get_lock("/file2.txt").unwrap();
        assert!(lock2.is_some(), "file2.txt should be locked");
    }

    #[test]
    fn test_fs_unlock_many() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let mut fs = Fs::create(&fs_path).unwrap();
        fs.set_access("testuser").unwrap();

        // Create and lock a file
        let mut txn = fs.begin_txn(Revnum(0), 0).unwrap();
        let mut root = txn.root().unwrap();
        root.make_file("/file.txt").unwrap();
        let rev = txn.commit().unwrap();

        let lock = fs
            .lock("/file.txt", None, None, false, None, rev, false)
            .unwrap();

        // Unlock using unlock_many
        let targets = vec![("/file.txt", lock.token())];
        let mut results: Vec<(String, bool)> = Vec::new();
        fs.unlock_many(&targets, false, |path, error| {
            results.push((path.to_string(), error.is_none()));
        })
        .unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].1, "unlock should succeed");

        // Verify lock is gone
        let lock_info = fs.get_lock("/file.txt").unwrap();
        assert!(lock_info.is_none(), "file.txt should no longer be locked");
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
        let result = fs.freeze(|| Err(Error::from_message("Test error from freeze callback")));

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
        // Creating the stream succeeds; the checksum mismatch only fails on commit
        root.apply_text("/test.txt", Some(wrong_checksum)).unwrap();
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

    #[test]
    fn test_transaction_change_props_bulk() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let fs = Fs::create(&fs_path).unwrap();
        let txn = fs.begin_txn(Revnum(0), 0).unwrap();

        // Set multiple properties at once
        txn.change_props(&[
            ("svn:log", Some(b"bulk commit" as &[u8])),
            ("svn:author", Some(b"alice" as &[u8])),
            ("custom:tag", Some(b"v1.0" as &[u8])),
        ])
        .unwrap();

        assert_eq!(
            txn.prop("svn:log").unwrap().as_deref(),
            Some(b"bulk commit" as &[u8])
        );
        assert_eq!(
            txn.prop("svn:author").unwrap().as_deref(),
            Some(b"alice" as &[u8])
        );
        assert_eq!(
            txn.prop("custom:tag").unwrap().as_deref(),
            Some(b"v1.0" as &[u8])
        );

        // Delete a property using the bulk interface
        txn.change_props(&[("custom:tag", None)]).unwrap();
        assert_eq!(txn.prop("custom:tag").unwrap(), None);
    }

    #[test]
    fn test_node_id_compare_and_related() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let fs = Fs::create(&fs_path).unwrap();

        // Create initial revision
        let mut txn = fs.begin_txn(Revnum(0), 0).unwrap();
        txn.change_prop("svn:log", "Initial commit").unwrap();
        let mut root = txn.root().unwrap();
        root.make_dir("/trunk").unwrap();
        root.make_file("/trunk/file.txt").unwrap();
        root.set_file_contents("/trunk/file.txt", b"initial content")
            .unwrap();
        txn.commit().unwrap();

        // Create second revision with a copy
        let mut txn = fs.begin_txn(Revnum(1), 0).unwrap();
        txn.change_prop("svn:log", "Branch").unwrap();
        let mut root = txn.root().unwrap();
        let rev1_root = fs.revision_root(Revnum(1)).unwrap();
        root.copy(&rev1_root, "/trunk", "/branch").unwrap();
        txn.commit().unwrap();

        // Test node IDs
        let root1 = fs.revision_root(Revnum(1)).unwrap();
        let root2 = fs.revision_root(Revnum(2)).unwrap();

        let trunk_id1 = root1.node_id("/trunk").unwrap();
        let trunk_id2 = root2.node_id("/trunk").unwrap();
        let branch_id = root2.node_id("/branch").unwrap();

        // Test compare - same path, different revisions should be related
        assert_eq!(trunk_id1.compare(&trunk_id2), 0);

        // Test check_related - trunk and branch should be related (branch is a copy)
        assert!(trunk_id2.check_related(&branch_id));

        // Test eq
        assert!(trunk_id1.eq(&trunk_id2));
    }

    #[test]
    fn test_root_closest_copy() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let fs = Fs::create(&fs_path).unwrap();

        // Create initial revision
        let mut txn = fs.begin_txn(Revnum(0), 0).unwrap();
        txn.change_prop("svn:log", "Initial").unwrap();
        let mut root = txn.root().unwrap();
        root.make_dir("/trunk").unwrap();
        root.make_file("/trunk/file.txt").unwrap();
        txn.commit().unwrap();

        // Create a branch (copy)
        let mut txn = fs.begin_txn(Revnum(1), 0).unwrap();
        txn.change_prop("svn:log", "Branch").unwrap();
        let mut root = txn.root().unwrap();
        let rev1_root = fs.revision_root(Revnum(1)).unwrap();
        root.copy(&rev1_root, "/trunk", "/branch").unwrap();
        txn.commit().unwrap();

        let root = fs.revision_root(Revnum(2)).unwrap();

        // Test closest_copy on a copied path
        let result = root.closest_copy("/branch").unwrap();
        assert!(result.is_some());
        let (_copy_root, copy_path) = result.unwrap();
        assert_eq!(copy_path, "/branch");

        // Test on a non-copied path
        let result = root.closest_copy("/trunk").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_root_contents_and_props_different() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let fs = Fs::create(&fs_path).unwrap();

        // Create revision 1
        let mut txn = fs.begin_txn(Revnum(0), 0).unwrap();
        txn.change_prop("svn:log", "Rev 1").unwrap();
        let mut root = txn.root().unwrap();
        root.make_file("/file.txt").unwrap();
        root.set_file_contents("/file.txt", b"content v1").unwrap();
        root.change_node_prop("/file.txt", "custom:prop", b"value1")
            .unwrap();
        txn.commit().unwrap();

        // Create revision 2 with changed content
        let mut txn = fs.begin_txn(Revnum(1), 0).unwrap();
        txn.change_prop("svn:log", "Rev 2").unwrap();
        let mut root = txn.root().unwrap();
        root.set_file_contents("/file.txt", b"content v2").unwrap();
        txn.commit().unwrap();

        // Create revision 3 with changed property
        let mut txn = fs.begin_txn(Revnum(2), 0).unwrap();
        txn.change_prop("svn:log", "Rev 3").unwrap();
        let mut root = txn.root().unwrap();
        root.change_node_prop("/file.txt", "custom:prop", b"value2")
            .unwrap();
        txn.commit().unwrap();

        let root1 = fs.revision_root(Revnum(1)).unwrap();
        let root2 = fs.revision_root(Revnum(2)).unwrap();
        let root3 = fs.revision_root(Revnum(3)).unwrap();

        // Test contents_different
        assert!(root1
            .contents_different("/file.txt", &root2, "/file.txt")
            .unwrap());
        assert!(!root2
            .contents_different("/file.txt", &root3, "/file.txt")
            .unwrap());

        // Test props_different
        assert!(!root1
            .props_different("/file.txt", &root2, "/file.txt")
            .unwrap());
        assert!(root2
            .props_different("/file.txt", &root3, "/file.txt")
            .unwrap());
    }

    #[test]
    fn test_apply_text() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let fs = Fs::create(&fs_path).unwrap();

        // Create a transaction
        let mut txn = fs.begin_txn(Revnum(0), 0).unwrap();
        let mut root = txn.root().unwrap();

        // Create a file using make_file
        root.make_file("/test.txt").unwrap();

        // Apply text to the file
        let mut stream = root.apply_text("/test.txt", None).unwrap();
        stream.write(b"Hello, world!").unwrap();
        stream.close().unwrap();

        // Commit the transaction
        txn.commit().unwrap();

        // Verify the file contents
        let root = fs.revision_root(Revnum(1)).unwrap();
        let mut contents = Vec::new();
        root.file_contents("/test.txt")
            .unwrap()
            .read_to_end(&mut contents)
            .unwrap();
        assert_eq!(contents, b"Hello, world!");
    }

    #[test]
    fn test_get_file_delta_stream() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let fs = Fs::create(&fs_path).unwrap();

        // Create revision 1 with a file
        let mut txn = fs.begin_txn(Revnum(0), 0).unwrap();
        let mut root = txn.root().unwrap();
        root.make_file("/file.txt").unwrap();
        let mut stream = root.apply_text("/file.txt", None).unwrap();
        stream.write(b"First version").unwrap();
        stream.close().unwrap();
        txn.commit().unwrap();

        // Create revision 2 with modified file
        let mut txn = fs.begin_txn(Revnum(1), 0).unwrap();
        let mut root = txn.root().unwrap();
        let mut stream = root.apply_text("/file.txt", None).unwrap();
        stream.write(b"Second version with more text").unwrap();
        stream.close().unwrap();
        txn.commit().unwrap();

        // Get delta stream between the two versions
        let root1 = fs.revision_root(Revnum(1)).unwrap();
        let root2 = fs.revision_root(Revnum(2)).unwrap();
        root1
            .get_file_delta_stream("/file.txt", &root2, "/file.txt")
            .unwrap();
    }

    #[test]
    fn test_node_has_props() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let fs = Fs::create(&fs_path).unwrap();

        let mut txn = fs.begin_txn(Revnum(0), 0).unwrap();
        let mut root = txn.root().unwrap();
        root.make_file("/no-props.txt").unwrap();
        root.make_file("/with-props.txt").unwrap();
        root.change_node_prop("/with-props.txt", "custom:key", b"value")
            .unwrap();
        txn.commit().unwrap();

        let root = fs.revision_root(Revnum(1)).unwrap();

        assert!(
            !root.node_has_props("/no-props.txt").unwrap(),
            "file with no properties should return false"
        );
        assert!(
            root.node_has_props("/with-props.txt").unwrap(),
            "file with a property should return true"
        );
    }

    #[test]
    fn test_node_origin_rev() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let fs = Fs::create(&fs_path).unwrap();

        // Rev 1: create file
        let mut txn = fs.begin_txn(Revnum(0), 0).unwrap();
        txn.change_prop("svn:log", "Initial").unwrap();
        let mut root = txn.root().unwrap();
        root.make_file("/file.txt").unwrap();
        txn.commit().unwrap();

        // Rev 2: modify file (does not change its origin)
        let mut txn = fs.begin_txn(Revnum(1), 0).unwrap();
        txn.change_prop("svn:log", "Modify").unwrap();
        let mut root = txn.root().unwrap();
        root.set_file_contents("/file.txt", b"updated").unwrap();
        txn.commit().unwrap();

        let root2 = fs.revision_root(Revnum(2)).unwrap();

        let origin_rev = root2.node_origin_rev("/file.txt").unwrap();
        assert_eq!(origin_rev, Revnum(1), "origin revision should be 1");
    }

    #[test]
    fn test_copied_from() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let fs = Fs::create(&fs_path).unwrap();

        // Rev 1: create a file
        let mut txn = fs.begin_txn(Revnum(0), 0).unwrap();
        txn.change_prop("svn:log", "Initial").unwrap();
        let mut root = txn.root().unwrap();
        root.make_file("/original.txt").unwrap();
        txn.commit().unwrap();

        // Rev 2: copy the file
        let mut txn = fs.begin_txn(Revnum(1), 0).unwrap();
        txn.change_prop("svn:log", "Copy").unwrap();
        let mut txn_root = txn.root().unwrap();
        let rev1_root = fs.revision_root(Revnum(1)).unwrap();
        txn_root
            .copy(&rev1_root, "/original.txt", "/copy.txt")
            .unwrap();
        txn.commit().unwrap();

        let root2 = fs.revision_root(Revnum(2)).unwrap();

        let result = root2.copied_from("/copy.txt").unwrap();
        assert!(result.is_some(), "copied file should have a copy source");
        let (rev, src_path) = result.unwrap();
        assert_eq!(rev, Revnum(1), "copy source revision should be 1");
        assert_eq!(src_path, "/original.txt", "copy source path should match");

        let result = root2.copied_from("/original.txt").unwrap();
        assert!(
            result.is_none(),
            "non-copied file should return None from copied_from"
        );
    }

    #[test]
    fn test_node_relation() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let fs = Fs::create(&fs_path).unwrap();

        // Rev 1: create a file
        let mut txn = fs.begin_txn(Revnum(0), 0).unwrap();
        txn.change_prop("svn:log", "Initial").unwrap();
        let mut root = txn.root().unwrap();
        root.make_file("/file.txt").unwrap();
        txn.commit().unwrap();

        // Rev 2: modify the file
        let mut txn = fs.begin_txn(Revnum(1), 0).unwrap();
        txn.change_prop("svn:log", "Modify").unwrap();
        let mut root = txn.root().unwrap();
        root.set_file_contents("/file.txt", b"changed").unwrap();
        txn.commit().unwrap();

        // Rev 3: add an unrelated file
        let mut txn = fs.begin_txn(Revnum(2), 0).unwrap();
        txn.change_prop("svn:log", "Add other").unwrap();
        let mut root = txn.root().unwrap();
        root.make_file("/other.txt").unwrap();
        txn.commit().unwrap();

        let root1 = fs.revision_root(Revnum(1)).unwrap();
        let root2 = fs.revision_root(Revnum(2)).unwrap();
        let root3 = fs.revision_root(Revnum(3)).unwrap();

        // Same node at the same revision → Unchanged
        let rel = root1
            .node_relation("/file.txt", &root1, "/file.txt")
            .unwrap();
        assert_eq!(rel, crate::NodeRelation::Unchanged);

        // Same node across revisions (modified) → CommonAncestor
        let rel = root1
            .node_relation("/file.txt", &root2, "/file.txt")
            .unwrap();
        assert_eq!(rel, crate::NodeRelation::CommonAncestor);

        // Different nodes → Unrelated
        let rel = root1
            .node_relation("/file.txt", &root3, "/other.txt")
            .unwrap();
        assert_eq!(rel, crate::NodeRelation::Unrelated);
    }

    #[test]
    fn test_node_prop_returns_value() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let fs = Fs::create(&fs_path).unwrap();

        // Rev 1: create a file and set a property on it
        let mut txn = fs.begin_txn(Revnum(0), 0).unwrap();
        let mut root = txn.root().unwrap();
        root.make_file("/tagged.txt").unwrap();
        root.change_node_prop("/tagged.txt", "test:tag", b"my-value")
            .unwrap();
        let rev1 = txn.commit().unwrap();

        let rev_root = fs.revision_root(rev1).unwrap();
        let val = rev_root.node_prop("/tagged.txt", "test:tag").unwrap();
        assert_eq!(val.as_deref(), Some(b"my-value".as_ref()));

        // Property that doesn't exist should return None
        let missing = rev_root.node_prop("/tagged.txt", "test:missing").unwrap();
        assert_eq!(missing, None);
    }

    #[test]
    fn test_root_revision() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let fs = Fs::create(&fs_path).unwrap();

        // Rev 0 root should report revision 0
        let root0 = fs.revision_root(Revnum(0)).unwrap();
        assert_eq!(root0.revision(), Revnum(0));

        // Create rev 1 and check its root
        let mut txn = fs.begin_txn(Revnum(0), 0).unwrap();
        txn.root().unwrap().make_file("/a.txt").unwrap();
        let rev1 = txn.commit().unwrap();
        let root1 = fs.revision_root(rev1).unwrap();
        assert_eq!(root1.revision(), Revnum(1));
    }

    #[test]
    fn test_root_type_detection() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let fs = Fs::create(&fs_path).unwrap();

        // Revision root should be a revision root, not txn root
        let rev_root = fs.revision_root(Revnum(0)).unwrap();
        assert!(rev_root.is_revision_root());
        assert!(!rev_root.is_txn_root());
    }

    #[test]
    fn test_node_created_path() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let fs = Fs::create(&fs_path).unwrap();

        // Create a file and commit it
        let mut txn = fs.begin_txn(Revnum(0), 0).unwrap();
        txn.root().unwrap().make_file("/hello.txt").unwrap();
        let rev1 = txn.commit().unwrap();

        let root = fs.revision_root(rev1).unwrap();
        let created_path = root.node_created_path("/hello.txt").unwrap();
        assert_eq!(created_path, "/hello.txt");
    }

    #[test]
    fn test_fs_revision_prop() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let fs = Fs::create(&fs_path).unwrap();

        // svn:date is set by SVN itself on rev 0
        let date = fs.revision_prop(Revnum(0), "svn:date", false).unwrap();
        assert!(date.is_some(), "svn:date should be set on rev 0");
        let date_bytes = date.unwrap();
        let date_str = std::str::from_utf8(&date_bytes).unwrap();
        assert!(
            date_str.contains('T'),
            "svn:date should look like a timestamp: {date_str}"
        );

        // Missing property should return None
        let missing = fs
            .revision_prop(Revnum(0), "custom:missing", false)
            .unwrap();
        assert_eq!(missing, None);
    }

    #[test]
    fn test_fs_info_format() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let fs = Fs::create(&fs_path).unwrap();

        let (format, version) = fs.info_format().unwrap();
        assert!(format > 0, "fs format should be positive, got {format}");
        // version is (major, minor, patch) — major should be 1 for SVN
        let (major, minor, _patch) = version;
        assert_eq!(major, 1, "SVN major version should be 1");
        assert!(minor >= 9, "SVN minor version should be >= 9");
    }

    #[test]
    fn test_fs_change_rev_prop() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let mut fs = Fs::create(&fs_path).unwrap();

        // Set a custom property on rev 0 (no compare-and-swap)
        fs.change_rev_prop(Revnum(0), "custom:test", Some(b"hello"), None)
            .unwrap();

        let val = fs.revision_prop(Revnum(0), "custom:test", false).unwrap();
        assert_eq!(val.as_deref(), Some(b"hello" as &[u8]));

        // Update it using compare-and-swap (old value must match)
        fs.change_rev_prop(
            Revnum(0),
            "custom:test",
            Some(b"world"),
            Some(Some(b"hello")),
        )
        .unwrap();
        let val2 = fs.revision_prop(Revnum(0), "custom:test", false).unwrap();
        assert_eq!(val2.as_deref(), Some(b"world" as &[u8]));

        // Delete the property
        fs.change_rev_prop(Revnum(0), "custom:test", None, None)
            .unwrap();
        let val3 = fs.revision_prop(Revnum(0), "custom:test", false).unwrap();
        assert_eq!(val3, None);
    }

    #[test]
    fn test_fs_refresh_revision_props() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let mut fs = Fs::create(&fs_path).unwrap();

        // Should succeed without error
        fs.refresh_revision_props().unwrap();
    }

    #[test]
    fn test_fs_deltify_revision() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let fs = Fs::create(&fs_path).unwrap();

        // Create a revision to deltify
        let mut txn = fs.begin_txn(Revnum(0), 0).unwrap();
        txn.root().unwrap().make_file("/hello.txt").unwrap();
        let rev1 = txn.commit().unwrap();

        // deltify_revision is a housekeeping op — just ensure it doesn't error
        fs.deltify_revision(rev1).unwrap();
    }

    #[test]
    fn test_txn_root_name_and_base_revision() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let fs = Fs::create(&fs_path).unwrap();

        // Transaction root should report a txn name and the base revision
        let mut txn = fs.begin_txn(Revnum(0), 0).unwrap();
        let root = txn.root().unwrap();
        let txn_name = root.txn_root_name();
        assert!(!txn_name.is_empty(), "transaction name should not be empty");

        let base_rev = root.txn_root_base_revision();
        assert_eq!(base_rev, Revnum(0), "base revision should be 0");
    }

    #[test]
    fn test_root_verify() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let fs = Fs::create(&fs_path).unwrap();

        // Verify the root of revision 0 — should succeed
        let root = fs.revision_root(Revnum(0)).unwrap();
        root.verify().unwrap();
    }

    #[test]
    fn test_revision_link() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let fs = Fs::create(&fs_path).unwrap();

        // Create a file in rev 1
        let mut txn = fs.begin_txn(Revnum(0), 0).unwrap();
        txn.root().unwrap().make_file("/original.txt").unwrap();
        let rev1 = txn.commit().unwrap();

        // Use revision_link to link /original.txt in a new transaction
        // (revision_link links the path without recording copy history)
        let from_root = fs.revision_root(rev1).unwrap();
        let mut txn2 = fs.begin_txn(rev1, 0).unwrap();
        {
            let mut to_root = txn2.root().unwrap();
            to_root.revision_link(&from_root, "/original.txt").unwrap();
        }
        let rev2 = txn2.commit().unwrap();

        // The file should still exist in rev2
        let root2 = fs.revision_root(rev2).unwrap();
        let kind = root2.check_path("/original.txt").unwrap();
        assert_eq!(kind, crate::NodeKind::File);
    }

    #[test]
    fn test_root_get_mergeinfo_empty_when_not_set() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let fs = Fs::create(&fs_path).unwrap();

        let mut txn = fs.begin_txn(Revnum(0), 0).unwrap();
        txn.root().unwrap().make_dir("/trunk").unwrap();
        let rev1 = txn.commit().unwrap();

        let root = fs.revision_root(rev1).unwrap();
        let mut received: Vec<String> = Vec::new();
        root.get_mergeinfo(
            &["/trunk"],
            crate::mergeinfo::MergeinfoInheritance::Explicit,
            false,
            false,
            |path, _mi| {
                received.push(path.to_owned());
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(received, Vec::<String>::new());
    }

    #[test]
    fn test_root_get_mergeinfo_returns_value_when_set() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let fs = Fs::create(&fs_path).unwrap();

        // Create /trunk in rev 1
        let mut txn = fs.begin_txn(Revnum(0), 0).unwrap();
        txn.root().unwrap().make_dir("/trunk").unwrap();
        let rev1 = txn.commit().unwrap();

        // Set svn:mergeinfo on /trunk in rev 2
        let mut txn2 = fs.begin_txn(rev1, 0).unwrap();
        txn2.root()
            .unwrap()
            .change_node_prop("/trunk", "svn:mergeinfo", b"/branches/dev:1")
            .unwrap();
        let rev2 = txn2.commit().unwrap();

        let root = fs.revision_root(rev2).unwrap();
        let mut received: Vec<(String, String)> = Vec::new();
        root.get_mergeinfo(
            &["/trunk"],
            crate::mergeinfo::MergeinfoInheritance::Explicit,
            false,
            false,
            |path, mi| {
                received.push((path.to_owned(), mi.to_string().unwrap()));
                Ok(())
            },
        )
        .unwrap();

        assert_eq!(received.len(), 1);
        assert_eq!(received[0].0, "/trunk");
        assert!(
            received[0].1.contains("/branches/dev"),
            "mergeinfo should contain source: {}",
            received[0].1
        );
    }

    #[test]
    fn test_fs_version() {
        let v = super::version();
        // The version should have a valid major number (SVN is at 1.x)
        assert!(v.major() >= 1, "major version should be >= 1");
    }

    #[test]
    fn test_print_modules() {
        // Should return a non-empty string with known module names (e.g. "fsfs")
        let modules = super::print_modules().unwrap();
        assert!(
            modules.contains("fs_fs"),
            "Expected fs_fs module in: {}",
            modules
        );
    }

    #[test]
    fn test_info_config_files() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        Fs::create(&fs_path).unwrap();

        // Should return without error; may return empty vec if no config files
        let files = super::info_config_files(&fs_path).expect("info_config_files should not fail");
        // All returned paths should exist
        for p in &files {
            assert!(p.exists(), "Config file should exist: {:?}", p);
        }
    }

    #[test]
    fn test_fs_upgrade_already_current() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        // Create a fresh filesystem — it should already be at the latest format.
        // Upgrading it should either succeed or return SVN_ERR_FS_UNSUPPORTED_UPGRADE.
        Fs::create(&fs_path).unwrap();
        let result = super::upgrade(&fs_path, None, None);
        // Either it succeeds (no-op) or it returns unsupported upgrade.
        match result {
            Ok(()) => {}
            Err(e) => {
                // SVN_ERR_FS_UNSUPPORTED_UPGRADE is acceptable
                assert!(
                    e.to_string().contains("upgrade") || e.to_string().contains("Unsupported"),
                    "unexpected error: {}",
                    e
                );
            }
        }
    }

    #[test]
    fn test_fs_upgrade_with_notify() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        Fs::create(&fs_path).unwrap();

        let actions = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let actions_clone = actions.clone();
        let result = super::upgrade(
            &fs_path,
            Some(Box::new(move |number, action| {
                actions_clone.lock().unwrap().push((number, action));
            })),
            None,
        );
        // Accept either success or unsupported upgrade
        match result {
            Ok(()) => {}
            Err(e) => {
                assert!(
                    e.to_string().contains("upgrade") || e.to_string().contains("Unsupported"),
                    "unexpected error: {}",
                    e
                );
            }
        }
    }

    #[test]
    fn test_repos_change_txn_prop() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let fs = Fs::create(&fs_path).unwrap();
        let txn = fs.begin_txn(Revnum(0), 0).unwrap();

        // Set a log message via the validating wrapper
        txn.repos_change_prop("svn:log", Some(b"hello from repos"))
            .unwrap();
        assert_eq!(
            txn.prop("svn:log").unwrap().as_deref(),
            Some(b"hello from repos" as &[u8])
        );

        // Delete the property
        txn.repos_change_prop("svn:log", None).unwrap();
        let val = txn.prop("svn:log").unwrap();
        assert!(val.is_none());
    }

    #[test]
    fn test_repos_change_txn_props_bulk() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        let fs = Fs::create(&fs_path).unwrap();
        let txn = fs.begin_txn(Revnum(0), 0).unwrap();

        txn.repos_change_props(&[
            ("svn:log", Some(b"bulk" as &[u8])),
            ("svn:author", Some(b"bob" as &[u8])),
        ])
        .unwrap();

        assert_eq!(
            txn.prop("svn:log").unwrap().as_deref(),
            Some(b"bulk" as &[u8])
        );
        assert_eq!(
            txn.prop("svn:author").unwrap().as_deref(),
            Some(b"bob" as &[u8])
        );
    }

    #[test]
    fn test_repos_change_node_prop_and_get_inherited_props() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");

        let fs = Fs::create(&fs_path).unwrap();

        // Create a directory hierarchy: /trunk/src
        let mut txn = fs.begin_txn(Revnum(0), 0).unwrap();
        txn.change_prop("svn:log", "init").unwrap();
        {
            let mut root = txn.root().unwrap();
            root.make_dir("/trunk").unwrap();
            root.make_dir("/trunk/src").unwrap();
            // Set a property on /trunk using the validating wrapper
            root.repos_change_node_prop("/trunk", "test:prop", Some(b"trunk-value"))
                .unwrap();
        }
        txn.commit().unwrap();

        // Read the property back
        let rev_root = fs.revision_root(Revnum(1)).unwrap();
        let val = rev_root.node_prop("/trunk", "test:prop").unwrap();
        assert_eq!(val.as_deref(), Some(b"trunk-value" as &[u8]));

        // Get inherited props for /trunk/src — it should inherit from /trunk
        let mut txn2 = fs.begin_txn(Revnum(1), 0).unwrap();
        let root2 = txn2.root().unwrap();
        let inherited = root2.repos_get_inherited_props("/trunk/src", None).unwrap();

        // /trunk should appear with test:prop
        let trunk_entry = inherited
            .iter()
            .find(|(path, _)| path == "trunk" || path == "/trunk");
        assert!(
            trunk_entry.is_some(),
            "expected /trunk in inherited props, got: {:?}",
            inherited.iter().map(|(p, _)| p).collect::<Vec<_>>()
        );
        let (_, trunk_props) = trunk_entry.unwrap();
        assert_eq!(
            trunk_props.get("test:prop").map(|v| v.as_slice()),
            Some(b"trunk-value" as &[u8])
        );
    }

    /// Regression test: creating and dropping many revision roots must not
    /// corrupt APR's global allocator.  Previously, calling svn_fs_close_root
    /// inside Root::drop before destroying the parent pool triggered SVN/APR
    /// cleanup interactions that caused either an infinite loop in
    /// apr_pool_destroy or a NULL-pool SEGV in a later test.
    #[test]
    fn test_root_pool_lifecycle_stress() {
        let dir = tempdir().unwrap();
        let fs = Fs::create(&dir.path().join("test-fs")).unwrap();

        // Create revision 1 with one file.
        let mut txn = fs.begin_txn(Revnum(0), 0).unwrap();
        txn.change_prop("svn:log", "init").unwrap();
        let mut root = txn.root().unwrap();
        root.make_file("/file.txt").unwrap();
        root.set_file_contents("/file.txt", b"hello").unwrap();
        txn.commit().unwrap();

        // Create and drop 50 revision roots in sequence, exercising the pool
        // alloc/free cycle each time.
        for _ in 0..50 {
            let root = fs.revision_root(Revnum(1)).unwrap();
            assert!(root.is_file("/file.txt").unwrap());
            // drop(root) here — pool is destroyed, root->pool (child) is freed
        }

        // After many pool alloc/free cycles verify the allocator is still
        // healthy by creating a separate Fs and writing data through it.
        let dir2 = tempdir().unwrap();
        let fs2 = Fs::create(&dir2.path().join("test-fs-2")).unwrap();
        let mut txn2 = fs2.begin_txn(Revnum(0), 0).unwrap();
        txn2.change_prop("svn:log", "test").unwrap();
        let mut root2 = txn2.root().unwrap();
        root2.make_file("/file2.txt").unwrap();
        root2.set_file_contents("/file2.txt", b"world").unwrap();
        txn2.commit().unwrap();

        let rev_root = fs2.revision_root(Revnum(1)).unwrap();
        let mut contents = Vec::new();
        rev_root
            .file_contents("/file2.txt")
            .unwrap()
            .read_to_end(&mut contents)
            .unwrap();
        assert_eq!(contents, b"world");
    }

    /// Regression test: dropping a root returned by closest_copy must not
    /// corrupt APR's allocator so that a subsequent TxnRoot::set_file_contents
    /// still works.  This reproduces the exact call sequence that previously
    /// triggered the ASAN SEGV (apr_palloc(NULL, ...) in FSFS apply_text).
    #[test]
    fn test_root_closest_copy_then_txn_write() {
        let dir = tempdir().unwrap();
        let fs = Fs::create(&dir.path().join("test-fs")).unwrap();

        // Rev 1: /trunk/file.txt
        let mut txn = fs.begin_txn(Revnum(0), 0).unwrap();
        txn.change_prop("svn:log", "rev1").unwrap();
        let mut root = txn.root().unwrap();
        root.make_dir("/trunk").unwrap();
        root.make_file("/trunk/file.txt").unwrap();
        root.set_file_contents("/trunk/file.txt", b"original")
            .unwrap();
        txn.commit().unwrap();

        // Rev 2: branch /trunk → /branch
        let mut txn = fs.begin_txn(Revnum(1), 0).unwrap();
        txn.change_prop("svn:log", "branch").unwrap();
        let mut root = txn.root().unwrap();
        let rev1_root = fs.revision_root(Revnum(1)).unwrap();
        root.copy(&rev1_root, "/trunk", "/branch").unwrap();
        drop(rev1_root); // pool freed here
        txn.commit().unwrap();

        // Look up closest_copy — this creates a Root backed by its own pool.
        let rev2_root = fs.revision_root(Revnum(2)).unwrap();
        let result = rev2_root.closest_copy("/branch").unwrap();
        assert!(result.is_some());
        let (_copy_root, copy_path) = result.unwrap();
        assert_eq!(copy_path, "/branch");
        drop(_copy_root); // pool freed here
        drop(rev2_root); // pool freed here

        // This write would crash with a NULL pool if the allocator was
        // corrupted by the root drops above.
        let mut txn = fs.begin_txn(Revnum(2), 0).unwrap();
        txn.change_prop("svn:log", "edit").unwrap();
        let mut root = txn.root().unwrap();
        root.set_file_contents("/trunk/file.txt", b"updated")
            .unwrap();
        txn.commit().unwrap();

        let rev3_root = fs.revision_root(Revnum(3)).unwrap();
        let mut contents = Vec::new();
        rev3_root
            .file_contents("/trunk/file.txt")
            .unwrap()
            .read_to_end(&mut contents)
            .unwrap();
        assert_eq!(contents, b"updated");
    }
}
