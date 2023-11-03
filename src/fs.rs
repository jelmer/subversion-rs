use apr::pool::PooledPtr;
pub struct Fs<'pool>(pub(crate) PooledPtr<'pool, crate::generated::svn_fs_t>);

impl<'pool> Fs<'pool> {
    pub fn create(path: &'pool std::path::Path) -> Result<Fs, crate::Error> {
        unsafe {
            Ok(Self(PooledPtr::initialize(|pool| {
                let mut fs_ptr = std::ptr::null_mut();
                let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
                let err = crate::generated::svn_fs_create2(
                    &mut fs_ptr,
                    path.as_ptr(),
                    std::ptr::null_mut(),
                    pool.as_mut_ptr(),
                    apr::pool::Pool::new().as_mut_ptr(),
                );
                if err.is_null() {
                    Ok(fs_ptr)
                } else {
                    Err(crate::Error(err))
                }
            })?))
        }
    }

    pub fn open(path: &'pool std::path::Path) -> Result<Fs, crate::Error> {
        unsafe {
            Ok(Self(PooledPtr::initialize(|pool| {
                let mut fs_ptr = std::ptr::null_mut();
                let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
                let err = crate::generated::svn_fs_open2(
                    &mut fs_ptr,
                    path.as_ptr(),
                    std::ptr::null_mut(),
                    pool.as_mut_ptr(),
                    apr::pool::Pool::new().as_mut_ptr(),
                );
                if err.is_null() {
                    Ok(fs_ptr)
                } else {
                    Err(crate::Error(err))
                }
            })?))
        }
    }

    pub fn path(&mut self) -> std::path::PathBuf {
        unsafe {
            let mut pool = apr::pool::Pool::new();
            let path = crate::generated::svn_fs_path(self.0.as_mut_ptr(), pool.as_mut_ptr());
            std::ffi::CStr::from_ptr(path)
                .to_string_lossy()
                .into_owned()
                .into()
        }
    }
}

pub fn fs_type(path: &std::path::Path) -> Result<String, crate::Error> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    unsafe {
        let mut pool = apr::pool::Pool::new();
        let mut fs_type = std::ptr::null();
        let err = crate::generated::svn_fs_type(&mut fs_type, path.as_ptr(), pool.as_mut_ptr());
        if err.is_null() {
            Ok(std::ffi::CStr::from_ptr(fs_type)
                .to_string_lossy()
                .into_owned())
        } else {
            Err(crate::Error(err))
        }
    }
}

pub fn delete_fs(path: &std::path::Path) -> Result<(), crate::Error> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    unsafe {
        let err =
            crate::generated::svn_fs_delete_fs(path.as_ptr(), apr::pool::Pool::new().as_mut_ptr());
        if err.is_null() {
            Ok(())
        } else {
            Err(crate::Error(err))
        }
    }
}
