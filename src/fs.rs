use crate::{svn_result, with_tmp_pool, Error, Revnum};

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
        &mut self,
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
        let mut revprops =
            apr::hash::Hash::<&str, *const subversion_sys::svn_string_t>::from_ptr(props);

        let revprops = revprops
            .iter(&pool)
            .map(|(k, v)| {
                (
                    std::str::from_utf8(k).unwrap().to_string(),
                    Vec::from(unsafe {
                        std::slice::from_raw_parts((**v).data as *const u8, (**v).len)
                    }),
                )
            })
            .collect();

        Ok(revprops)
    }

    pub fn revision_root(&mut self, rev: Revnum) -> Result<Root, Error> {
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

    pub fn get_uuid(&mut self) -> Result<String, Error> {
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
        
        let mut fs = Fs::create(&fs_path).unwrap();
        let rev = fs.youngest_revision();
        assert!(rev.is_ok());
        // New filesystem should have revision 0
        assert_eq!(rev.unwrap(), crate::Revnum(0));
    }

    #[test]
    fn test_fs_get_uuid() {
        let dir = tempdir().unwrap();
        let fs_path = dir.path().join("test-fs");
        
        let mut fs = Fs::create(&fs_path).unwrap();
        let uuid = fs.get_uuid();
        assert!(uuid.is_ok());
        // UUID should not be empty
        assert!(!uuid.unwrap().is_empty());
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
}
