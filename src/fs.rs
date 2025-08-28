use crate::{svn_result, with_tmp_pool, Error, Revnum};

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

        let mut revprops = apr::hash::Hash::<&[u8], subversion_sys::svn_string_t>::from_ptr(props);

        let revprops = revprops
            .iter(&pool)
            .map(|(k, v)| {
                (
                    String::from_utf8_lossy(k).into_owned(),
                    if v.data.is_null() || v.len == 0 {
                        Vec::new()
                    } else {
                        unsafe { std::slice::from_raw_parts(v.data as *const u8, v.len).to_vec() }
                    },
                )
            })
            .collect();

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

            let mut result = std::collections::HashMap::new();
            if !props.is_null() {
                let mut hash =
                    apr::hash::Hash::<&[u8], subversion_sys::svn_string_t>::from_ptr(props);
                for (k, v) in hash.iter(pool) {
                    let key = String::from_utf8_lossy(k).into_owned();
                    let value = if v.data.is_null() || v.len == 0 {
                        Vec::new()
                    } else {
                        unsafe { std::slice::from_raw_parts(v.data as *const u8, v.len).to_vec() }
                    };
                    result.insert(key, value);
                }
            }
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

            let mut result = std::collections::HashMap::new();
            if !changed_paths.is_null() {
                let mut hash =
                    apr::hash::Hash::<&[u8], *mut subversion_sys::svn_fs_path_change2_t>::from_ptr(
                        changed_paths,
                    );
                for (k, v) in hash.iter(pool) {
                    let path = String::from_utf8_lossy(k).into_owned();
                    let change = FsPathChange::from_raw(*v);
                    result.insert(path, change);
                }
            }
            Ok(result)
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
        with_tmp_pool(|pool| unsafe {
            let path_c = std::ffi::CString::new(path).unwrap();
            let mut entries = std::ptr::null_mut();
            let err = subversion_sys::svn_fs_dir_entries(
                &mut entries,
                self.ptr,
                path_c.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;

            let mut result = std::collections::HashMap::new();
            if !entries.is_null() {
                let mut hash =
                    apr::hash::Hash::<&[u8], *mut subversion_sys::svn_fs_dirent_t>::from_ptr(
                        entries,
                    );
                for (k, v) in hash.iter(pool) {
                    let name = String::from_utf8_lossy(k).into_owned();
                    let entry = FsDirEntry::from_raw(*v);
                    result.insert(name, entry);
                }
            }
            Ok(result)
        })
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

        let mut fs = Fs::create(&fs_path).unwrap();
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
        use super::FsPathChange;

        // We can at least test the structure exists and compiles
        // Real integration tests would require creating actual commits
    }

    #[test]
    fn test_fs_dir_entry_accessors() {
        // Test FsDirEntry accessors
        // Note: This is a basic test since we need actual directory entries
        use super::FsDirEntry;

        // We can at least test the structure exists and compiles
        // Real integration tests would require creating actual files/directories
    }
}
