use crate::{svn_result, with_tmp_pool, Error, Revnum};
use std::ffi::{CStr, CString};

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
    pub fn from_canonical(path: &str) -> Result<Self, Error> {
        // Handle empty path as root
        let path = if path.is_empty() { "/" } else { path };

        // Ensure path is absolute for filesystem operations
        if !path.starts_with('/') {
            return Err(Error::from_str(&format!(
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
                Err(Error::from_str(&format!("Path is not canonical: {}", path)))
            }
        })
    }

    /// Create an FsPath by canonicalizing the input path.
    ///
    /// This will canonicalize the path using SVN's canonicalization rules.
    /// Returns an error if the path cannot be made canonical.
    pub fn canonicalize(path: &str) -> Result<Self, Error> {
        // Handle empty path as root
        let path = if path.is_empty() { "/" } else { path };

        // Ensure path is absolute for filesystem operations
        if !path.starts_with('/') {
            return Err(Error::from_str(&format!(
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
                    return Err(Error::from_str(&format!(
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
    type Error = Error;

    fn try_from(path: &str) -> Result<Self, Self::Error> {
        FsPath::canonicalize(path)
    }
}

impl TryFrom<String> for FsPath {
    type Error = Error;

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
    pub fn from_raw(ptr: *mut subversion_sys::svn_fs_path_change2_t) -> Self {
        Self { ptr }
    }

    pub fn change_kind(&self) -> crate::FsPathChangeKind {
        unsafe { (*self.ptr).change_kind.into() }
    }

    pub fn node_kind(&self) -> crate::NodeKind {
        unsafe { (*self.ptr).node_kind.into() }
    }

    pub fn text_modified(&self) -> bool {
        unsafe { (*self.ptr).text_mod != 0 }
    }

    pub fn props_modified(&self) -> bool {
        unsafe { (*self.ptr).prop_mod != 0 }
    }

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
}

impl FsDirEntry {
    pub fn from_raw(ptr: *mut subversion_sys::svn_fs_dirent_t) -> Self {
        Self { ptr }
    }

    pub fn name(&self) -> &str {
        unsafe { std::ffi::CStr::from_ptr((*self.ptr).name).to_str().unwrap() }
    }

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

    pub fn kind(&self) -> crate::NodeKind {
        unsafe { (*self.ptr).kind.into() }
    }
}

/// Filesystem handle with RAII cleanup
pub struct Fs {
    fs_ptr: *mut subversion_sys::svn_fs_t,
    pool: apr::Pool, // Pool for allocations (and fs lifetime if owned)
    owned: bool,     // Whether we own the fs_ptr or just borrow it
}

unsafe impl Send for Fs {}

impl Drop for Fs {
    fn drop(&mut self) {
        // Only owned fs needs cleanup via pool
        // Borrowed fs is cleaned up by its owner
        if self.owned {
            // Pool drop will clean up fs
        }
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
    /// This creates a borrowed Fs that doesn't own the fs_ptr
    pub(crate) unsafe fn from_ptr_and_pool(
        fs_ptr: *mut subversion_sys::svn_fs_t,
        pool: apr::Pool,
    ) -> Self {
        Self {
            fs_ptr,
            pool,
            owned: false, // This is a borrowed fs from repos
        }
    }

    pub fn create(path: &std::path::Path) -> Result<Fs, Error> {
        // Ensure SVN libraries are initialized
        crate::init::initialize()?;

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

            Ok(Fs {
                fs_ptr,
                pool,
                owned: true, // We created this fs
            })
        }
    }

    pub fn open(path: &std::path::Path) -> Result<Fs, Error> {
        // Ensure SVN libraries are initialized
        crate::init::initialize()?;

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

            Ok(Fs {
                fs_ptr,
                pool,
                owned: true, // We created this fs
            })
        }
    }

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

    pub fn unlock(
        &mut self,
        path: &std::path::Path,
        token: &str,
        break_lock: bool,
    ) -> Result<(), Error> {
        unsafe {
            let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
            let token = std::ffi::CString::new(token).unwrap();
            let err = subversion_sys::svn_fs_unlock(
                self.fs_ptr,
                path.as_ptr(),
                token.as_ptr(),
                break_lock as i32,
                apr::pool::Pool::new().as_mut_ptr(),
            );
            svn_result(err)?;
            Ok(())
        }
    }
}

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

pub fn delete_fs(path: &std::path::Path) -> Result<(), Error> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    unsafe {
        let err =
            subversion_sys::svn_fs_delete_fs(path.as_ptr(), apr::pool::Pool::new().as_mut_ptr());
        svn_result(err)?;
        Ok(())
    }
}

#[allow(dead_code)]
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
    pub fn as_ptr(&self) -> *const subversion_sys::svn_fs_root_t {
        self.ptr
    }

    pub fn as_mut_ptr(&mut self) -> *mut subversion_sys::svn_fs_root_t {
        self.ptr
    }

    /// Check if a path is a directory
    pub fn is_dir(&self, path: impl TryInto<FsPath, Error = Error>) -> Result<bool, Error> {
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
    pub fn is_file(&self, path: impl TryInto<FsPath, Error = Error>) -> Result<bool, Error> {
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
    pub fn file_length(&self, path: impl TryInto<FsPath, Error = Error>) -> Result<i64, Error> {
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
        path: impl TryInto<FsPath, Error = Error>,
    ) -> Result<crate::io::Stream, Error> {
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

    /// Get the checksum of a file
    pub fn file_checksum(
        &self,
        path: &str,
        kind: crate::ChecksumKind,
    ) -> Result<Option<crate::Checksum<'_>>, Error> {
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
    pub fn check_path(
        &self,
        path: impl TryInto<FsPath, Error = Error>,
    ) -> Result<crate::NodeKind, Error> {
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
        path: impl TryInto<FsPath, Error = Error>,
    ) -> Result<std::collections::HashMap<String, FsDirEntry>, Error> {
        let fs_path = path.try_into()?;
        with_tmp_pool(|pool| unsafe {
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
                Ok(hash.to_hashmap())
            }
        })
    }
    /// Check if file contents have changed between two paths
    pub fn contents_changed(
        &self,
        path1: impl TryInto<FsPath, Error = Error>,
        root2: &Root,
        path2: impl TryInto<FsPath, Error = Error>,
    ) -> Result<bool, Error> {
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
        path1: impl TryInto<FsPath, Error = Error>,
        root2: &Root,
        path2: impl TryInto<FsPath, Error = Error>,
    ) -> Result<bool, Error> {
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
    pub fn node_history(
        &self,
        path: impl TryInto<FsPath, Error = Error>,
    ) -> Result<NodeHistory, Error> {
        let fs_path = path.try_into()?;
        unsafe {
            // Create a pool that will live as long as the NodeHistory
            let pool = apr::Pool::new();
            let mut history: *mut subversion_sys::svn_fs_history_t = std::ptr::null_mut();

            let err = subversion_sys::svn_fs_node_history(
                &mut history,
                self.ptr,
                fs_path.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;

            if history.is_null() {
                return Err(Error::from_str("Failed to get node history"));
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
        path: impl TryInto<FsPath, Error = Error>,
    ) -> Result<Revnum, Error> {
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
    pub fn node_id(&self, path: impl TryInto<FsPath, Error = Error>) -> Result<NodeId, Error> {
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
                return Err(Error::from_str("Failed to get node ID"));
            }

            Ok(NodeId { ptr: id, pool })
        }
    }
}

/// Represents the history of a node in the filesystem
pub struct NodeHistory {
    ptr: *mut subversion_sys::svn_fs_history_t,
    pool: apr::Pool, // Pool used for all history allocations
}

impl NodeHistory {
    /// Get the previous history entry
    pub fn prev(&mut self, cross_copies: bool) -> Result<Option<(String, Revnum)>, Error> {
        unsafe {
            let mut prev_history: *mut subversion_sys::svn_fs_history_t = std::ptr::null_mut();
            // Use the NodeHistory's own pool for allocations
            let err = subversion_sys::svn_fs_history_prev(
                &mut prev_history,
                self.ptr,
                if cross_copies { 1 } else { 0 },
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
    pool: apr::Pool, // Keep the pool alive for the lifetime of the NodeId
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
    pub fn to_string(&self) -> Result<String, Error> {
        unsafe {
            let str_svn = subversion_sys::svn_fs_unparse_id(self.ptr, self.pool.as_mut_ptr());
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
    #[allow(dead_code)]
    pool: apr::Pool,
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
            pool,
            _phantom: std::marker::PhantomData,
        }
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
}

/// Transaction root for making changes
pub struct TxnRoot {
    ptr: *mut subversion_sys::svn_fs_root_t,
    _pool: apr::Pool,
}

impl TxnRoot {
    /// Create a directory
    pub fn make_dir(&mut self, path: impl TryInto<FsPath, Error = Error>) -> Result<(), Error> {
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
    pub fn make_file(&mut self, path: impl TryInto<FsPath, Error = Error>) -> Result<(), Error> {
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
    pub fn delete(&mut self, path: impl TryInto<FsPath, Error = Error>) -> Result<(), Error> {
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
        from_path: impl TryInto<FsPath, Error = Error>,
        to_path: impl TryInto<FsPath, Error = Error>,
    ) -> Result<(), Error> {
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
        path: impl TryInto<FsPath, Error = Error>,
    ) -> Result<crate::io::Stream, Error> {
        let fs_path = path.try_into()?;
        let pool = apr::Pool::new();
        unsafe {
            let mut stream_ptr = std::ptr::null_mut();
            let err = subversion_sys::svn_fs_apply_text(
                &mut stream_ptr,
                self.ptr,
                fs_path.as_ptr(),
                std::ptr::null(), // result_checksum - we ignore for now
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
        path: impl TryInto<FsPath, Error = Error>,
        name: &str,
        value: &[u8],
    ) -> Result<(), Error> {
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
        path: impl TryInto<FsPath, Error = Error>,
    ) -> Result<crate::NodeKind, Error> {
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
        path: impl TryInto<FsPath, Error = Error>,
        contents: &[u8],
    ) -> Result<(), Error> {
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
    pub fn file_contents(
        &self,
        path: impl TryInto<FsPath, Error = Error>,
    ) -> Result<Vec<u8>, Error> {
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
}

impl Fs {
    /// Begin a new transaction
    pub fn begin_txn(&self, base_rev: Revnum) -> Result<Transaction, Error> {
        let pool = apr::Pool::new();
        unsafe {
            let mut txn_ptr = std::ptr::null_mut();
            let err = subversion_sys::svn_fs_begin_txn2(
                &mut txn_ptr,
                self.as_ptr() as *mut _,
                base_rev.into(),
                0, // flags - 0 for now
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(Transaction {
                ptr: txn_ptr,
                pool,
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
                pool,
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
        let mut txn = fs.begin_txn(crate::Revnum::from(0u32)).unwrap();

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
        let mut stream = root.apply_text("/trunk/test.txt").unwrap();
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
        let mut txn = fs.begin_txn(crate::Revnum(0)).unwrap();
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
        let mut txn1 = fs.begin_txn(crate::Revnum(0)).unwrap();
        txn1.change_prop("svn:log", "Initial commit").unwrap();
        let mut root1 = txn1.root().unwrap();

        root1.make_dir("/original").unwrap();
        root1.make_file("/original/file.txt").unwrap();

        let mut stream = root1.apply_text("/original/file.txt").unwrap();
        use std::io::Write;
        stream.write_all(b"Original content\n").unwrap();
        drop(stream);

        let rev1 = txn1.commit().unwrap();

        // Now copy the content in a new transaction
        let mut txn2 = fs.begin_txn(rev1).unwrap();
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
        let mut txn1 = fs.begin_txn(crate::Revnum(0)).unwrap();
        txn1.change_prop("svn:log", "Create files").unwrap();
        let mut root1 = txn1.root().unwrap();

        root1.make_dir("/dir1").unwrap();
        root1.make_file("/dir1/file1.txt").unwrap();
        root1.make_file("/file2.txt").unwrap();

        let rev1 = txn1.commit().unwrap();

        // Now delete some content
        let mut txn2 = fs.begin_txn(rev1).unwrap();
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

        let mut txn = fs.begin_txn(crate::Revnum(0)).unwrap();
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
        let txn = fs.begin_txn(crate::Revnum(0)).unwrap();
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
        let mut txn = fs.begin_txn(Revnum(0)).unwrap();
        let mut txn_root = txn.root().unwrap();

        // Create a file
        txn_root.make_file("/test.txt").unwrap();
        let mut stream = txn_root.apply_text("/test.txt").unwrap();
        use std::io::Write;
        write!(stream, "Initial content").unwrap();
        stream.close().unwrap();

        // Commit the transaction
        let rev1 = txn.commit().unwrap();

        // Create second revision modifying the file
        let mut txn2 = fs.begin_txn(rev1).unwrap();
        let mut txn_root2 = txn2.root().unwrap();

        let mut stream2 = txn_root2.apply_text("/test.txt").unwrap();
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
        let mut txn = fs.begin_txn(Revnum(0)).unwrap();
        let mut txn_root = txn.root().unwrap();

        // Create a file with a property
        txn_root.make_file("/test.txt").unwrap();
        txn_root
            .change_node_prop("/test.txt", "custom:prop", b"value1")
            .unwrap();

        let rev1 = txn.commit().unwrap();

        // Create second revision changing the property
        let mut txn2 = fs.begin_txn(rev1).unwrap();
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
        let txn = fs.begin_txn(Revnum(0)).unwrap();
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
        let mut txn1 = fs.begin_txn(Revnum(0)).unwrap();
        let mut txn_root1 = txn1.root().unwrap();
        txn_root1.make_file("/test.txt").unwrap();
        txn_root1
            .set_file_contents("/test.txt", b"Hello, World!")
            .unwrap();
        let rev1 = txn1.commit().unwrap();

        // Now test moving the file in a new transaction
        let mut txn2 = fs.begin_txn(rev1).unwrap();
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
        let mut txn1 = fs.begin_txn(Revnum(0)).unwrap();
        let mut root1 = txn1.root().unwrap();
        root1.make_file("/file.txt").unwrap();
        root1
            .set_file_contents("/file.txt", b"Initial content")
            .unwrap();
        let rev1 = txn1.commit().unwrap();

        // Create two divergent changes
        // Branch 1: modify the file
        let mut txn2 = fs.begin_txn(rev1).unwrap();
        let mut root2 = txn2.root().unwrap();
        root2
            .set_file_contents("/file.txt", b"Branch 1 content")
            .unwrap();
        let rev2 = txn2.commit().unwrap();

        // Branch 2: also modify the file (creating a conflict)
        let mut txn3 = fs.begin_txn(rev1).unwrap();
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
        let mut txn1 = fs.begin_txn(Revnum(0)).unwrap();
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
        let mut txn2 = fs.begin_txn(rev1).unwrap();
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
        let mut txn3 = fs.begin_txn(rev2).unwrap();
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
        let mut txn1 = fs.begin_txn(Revnum(0)).unwrap();
        let mut root1 = txn1.root().unwrap();
        root1.make_file("/file.txt").unwrap();
        root1.set_file_contents("/file.txt", b"Version 1").unwrap();
        let rev1 = txn1.commit().unwrap();

        // Modify the file in rev 2
        let mut txn2 = fs.begin_txn(rev1).unwrap();
        let mut root2 = txn2.root().unwrap();
        root2.set_file_contents("/file.txt", b"Version 2").unwrap();
        let rev2 = txn2.commit().unwrap();

        // Copy the file in rev 3
        let mut txn3 = fs.begin_txn(rev2).unwrap();
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
}
