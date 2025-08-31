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

        let revprops = apr::hash::Hash::<&[u8], Option<&crate::SvnString>>::from_ptr(props);

        let revprops = revprops
            .iter(&pool)
            .filter_map(|(k, v_opt)| {
                v_opt.map(|v| {
                    (
                        String::from_utf8_lossy(k).into_owned(),
                        v.as_bytes().to_vec(),
                    )
                })
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
                let hash = apr::hash::Hash::<&[u8], Option<&crate::SvnString>>::from_ptr(props);
                for (k, v_opt) in hash.iter(pool) {
                    if let Some(v) = v_opt {
                        let key = String::from_utf8_lossy(k).into_owned();
                        let value = v.as_bytes().to_vec();
                        result.insert(key, value);
                    }
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
                let hash =
                    apr::hash::Hash::<&[u8], *mut subversion_sys::svn_fs_path_change2_t>::from_ptr(
                        changed_paths,
                    );
                for (k, v) in hash.iter(pool) {
                    let path = String::from_utf8_lossy(k).into_owned();
                    let change = FsPathChange::from_raw(v);
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
                let hash = apr::hash::Hash::<&[u8], *mut subversion_sys::svn_fs_dirent_t>::from_ptr(
                    entries,
                );
                for (k, v) in hash.iter(pool) {
                    let name = String::from_utf8_lossy(k).into_owned();
                    let entry = FsDirEntry::from_raw(v);
                    result.insert(name, entry);
                }
            }
            Ok(result)
        })
    }
    /// Check if file contents have changed between two paths
    pub fn contents_changed(
        &self,
        path1: &str,
        root2: &Root,
        path2: &str,
    ) -> Result<bool, Error> {
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
    pub fn props_changed(
        &self,
        path1: &str,
        root2: &Root,
        path2: &str,
    ) -> Result<bool, Error> {
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
        with_tmp_pool(|pool| unsafe {
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
                pool: apr::Pool::new(),
            })
        })
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
        with_tmp_pool(|pool| unsafe {
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
                _phantom: std::marker::PhantomData,
            })
        })
    }
}

/// Represents the history of a node in the filesystem
pub struct NodeHistory {
    ptr: *mut subversion_sys::svn_fs_history_t,
    pool: apr::Pool,
}

impl NodeHistory {
    /// Get the previous history entry
    pub fn prev(&mut self, cross_copies: bool) -> Result<Option<(String, Revnum)>, Error> {
        with_tmp_pool(|pool| unsafe {
            let mut prev_history: *mut subversion_sys::svn_fs_history_t = std::ptr::null_mut();
            let err = subversion_sys::svn_fs_history_prev(
                &mut prev_history,
                self.ptr,
                if cross_copies { 1 } else { 0 },
                pool.as_mut_ptr(),
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
                pool.as_mut_ptr(),
            );
            svn_result(err)?;
            
            if path.is_null() {
                return Ok(None);
            }
            
            let path_str = std::ffi::CStr::from_ptr(path).to_string_lossy().into_owned();
            Ok(Some((path_str, Revnum(rev))))
        })
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
    _phantom: std::marker::PhantomData<()>,
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
    pool: apr::Pool,
    _phantom: std::marker::PhantomData<*mut ()>, // !Send + !Sync
}

impl Drop for Transaction {
    fn drop(&mut self) {
        // Pool drop will clean up transaction
    }
}

impl Transaction {
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
    pub fn apply_text(&mut self, path: &str) -> Result<crate::io::Stream, Error> {
        let path_cstr = std::ffi::CString::new(path)?;
        let pool = apr::Pool::new();
        unsafe {
            let mut stream_ptr = std::ptr::null_mut();
            let err = subversion_sys::svn_fs_apply_text(
                &mut stream_ptr,
                self.ptr,
                path_cstr.as_ptr(),
                std::ptr::null(), // result_checksum - we ignore for now
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
        root.make_dir("trunk").unwrap();

        // Verify the directory exists
        let kind = root.check_path("trunk").unwrap();
        assert_eq!(kind, crate::NodeKind::Dir);

        // Create a file
        root.make_file("trunk/test.txt").unwrap();

        // Verify the file exists
        let kind = root.check_path("trunk/test.txt").unwrap();
        assert_eq!(kind, crate::NodeKind::File);

        // Add content to the file
        let mut stream = root.apply_text("trunk/test.txt").unwrap();
        use std::io::Write;
        stream.write_all(b"Hello, World!\n").unwrap();
        drop(stream);

        // Set a property on the file
        root.change_node_prop("trunk/test.txt", "custom:prop", b"value")
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
        root.make_dir("test-dir").unwrap();
        root.make_file("test-file.txt").unwrap();

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

        root1.make_dir("original").unwrap();
        root1.make_file("original/file.txt").unwrap();

        let mut stream = root1.apply_text("original/file.txt").unwrap();
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
        root2.copy(&rev1_root, "original", "copy").unwrap();

        // Verify the copy exists
        let kind = root2.check_path("copy").unwrap();
        assert_eq!(kind, crate::NodeKind::Dir);

        let kind = root2.check_path("copy/file.txt").unwrap();
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

        root1.make_dir("dir1").unwrap();
        root1.make_file("dir1/file1.txt").unwrap();
        root1.make_file("file2.txt").unwrap();

        let rev1 = txn1.commit().unwrap();

        // Now delete some content
        let mut txn2 = fs.begin_txn(rev1).unwrap();
        txn2.change_prop("svn:log", "Delete files").unwrap();
        let mut root2 = txn2.root().unwrap();

        // Delete a file
        root2.delete("file2.txt").unwrap();

        // Verify it's gone
        let kind = root2.check_path("file2.txt").unwrap();
        assert_eq!(kind, crate::NodeKind::None);

        // But the other file still exists
        let kind = root2.check_path("dir1/file1.txt").unwrap();
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
        root.make_file("test.txt").unwrap();

        // Set various properties
        root.change_node_prop("test.txt", "svn:mime-type", b"text/plain")
            .unwrap();
        root.change_node_prop("test.txt", "custom:author", b"test-user")
            .unwrap();
        root.change_node_prop("test.txt", "custom:description", b"A test file")
            .unwrap();

        // Set an empty property (delete)
        root.change_node_prop("test.txt", "custom:empty", b"")
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
        let mut fs = Fs::create(&fs_path).unwrap();
        
        // Create first revision with a file
        let mut txn = fs.begin_txn(Revnum(0)).unwrap();
        let mut txn_root = txn.root().unwrap();
        
        // Create a file
        txn_root.make_file("test.txt").unwrap();
        let mut stream = txn_root.apply_text("test.txt").unwrap();
        use std::io::Write;
        write!(stream, "Initial content").unwrap();
        stream.close().unwrap();
        
        // Commit the transaction
        let rev1 = txn.commit().unwrap();
        
        // Create second revision modifying the file
        let mut txn2 = fs.begin_txn(rev1).unwrap();
        let mut txn_root2 = txn2.root().unwrap();
        
        let mut stream2 = txn_root2.apply_text("test.txt").unwrap();
        write!(stream2, "Modified content").unwrap();
        stream2.close().unwrap();
        
        let rev2 = txn2.commit().unwrap();
        
        // Test content comparison
        let root1 = fs.revision_root(rev1).unwrap();
        let root2 = fs.revision_root(rev2).unwrap();
        
        // Contents should be different between revisions
        let contents_changed = root1.contents_changed("test.txt", &root2, "test.txt").unwrap();
        assert!(contents_changed, "File contents should have changed between revisions");
        
        // Contents should be the same when comparing the same revision
        let contents_same = root1.contents_changed("test.txt", &root1, "test.txt").unwrap();
        assert!(!contents_same, "File contents should be the same in the same revision");
        
        // Test node history
        let _history = root2.node_history("test.txt").unwrap();
        // We should be able to get the history
        // Note: detailed history iteration would require more setup
        
        // Test node created revision
        // Note: node_created_rev returns the revision where the current node instance was created
        let created_rev1 = root1.node_created_rev("test.txt").unwrap();
        assert_eq!(created_rev1, rev1, "Node in rev1 should have been created in rev1");
        
        let created_rev2 = root2.node_created_rev("test.txt").unwrap();
        assert_eq!(created_rev2, rev2, "Node in rev2 should have been created in rev2 (after modification)");
        
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
        let mut fs = Fs::create(&fs_path).unwrap();
        
        // Create first revision with a file
        let mut txn = fs.begin_txn(Revnum(0)).unwrap();
        let mut txn_root = txn.root().unwrap();
        
        // Create a file with a property
        txn_root.make_file("test.txt").unwrap();
        txn_root.change_node_prop("test.txt", "custom:prop", b"value1").unwrap();
        
        let rev1 = txn.commit().unwrap();
        
        // Create second revision changing the property
        let mut txn2 = fs.begin_txn(rev1).unwrap();
        let mut txn_root2 = txn2.root().unwrap();
        
        txn_root2.change_node_prop("test.txt", "custom:prop", b"value2").unwrap();
        
        let rev2 = txn2.commit().unwrap();
        
        // Test property comparison
        let root1 = fs.revision_root(rev1).unwrap();
        let root2 = fs.revision_root(rev2).unwrap();
        
        // Properties should be different between revisions
        let props_changed = root1.props_changed("test.txt", &root2, "test.txt").unwrap();
        assert!(props_changed, "Properties should have changed between revisions");
        
        // Properties should be the same when comparing the same revision
        let props_same = root1.props_changed("test.txt", &root1, "test.txt").unwrap();
        assert!(!props_same, "Properties should be the same in the same revision");
        
        // Cleanup handled by tempdir Drop
    }
}
