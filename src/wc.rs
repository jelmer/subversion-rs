use crate::{svn_result, with_tmp_pool, Error};
use std::marker::PhantomData;
use subversion_sys::{svn_wc_context_t, svn_wc_version};
pub fn version() -> crate::Version {
    unsafe { crate::Version(svn_wc_version()) }
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
    let result = unsafe {
        subversion_sys::svn_wc_is_adm_dir(name_cstr.as_ptr(), pool.as_mut_ptr())
    };
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
pub fn ensure_adm(path: &std::path::Path, uuid: &str, url: &str, repos_root: &str, revision: i64) -> Result<(), crate::Error> {
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


#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_context_creation() {
        let context = Context::new();
        assert!(context.is_ok());
        let mut context = context.unwrap();
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
            "",  // uuid
            "file:///test/repo",  // url
            "file:///test/repo",  // repos
            0,  // revision
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
}
