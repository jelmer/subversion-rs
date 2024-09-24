use crate::{Error, Revnum};
use apr::pool::PooledPtr;
pub struct Fs(pub(crate) PooledPtr<crate::generated::svn_fs_t>);

impl Fs {
    pub fn create(path: &std::path::Path) -> Result<Fs, Error> {
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
                Error::from_raw(err)?;
                Ok::<_, Error>(fs_ptr)
            })?))
        }
    }

    pub fn open(path: &std::path::Path) -> Result<Fs, Error> {
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
                Error::from_raw(err)?;
                Ok::<_, Error>(fs_ptr)
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

    pub fn youngest_revision(&mut self) -> Result<Revnum, Error> {
        unsafe {
            let mut pool = apr::pool::Pool::new();
            let mut youngest = 0;
            let err = crate::generated::svn_fs_youngest_rev(
                &mut youngest,
                self.0.as_mut_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(Revnum::from_raw(youngest).unwrap())
        }
    }

    pub fn revision_proplist(
        &mut self,
        rev: Revnum,
    ) -> Result<std::collections::HashMap<String, Vec<u8>>, Error> {
        let mut pool = apr::pool::Pool::new();
        let mut props = std::ptr::null_mut();
        let err = unsafe {
            crate::generated::svn_fs_revision_proplist(
                &mut props,
                self.0.as_mut_ptr(),
                rev.0,
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        let mut revprops =
            apr::hash::Hash::<&str, *const crate::generated::svn_string_t>::from_raw(unsafe {
                PooledPtr::in_pool(std::rc::Rc::new(pool), props)
            });

        let revprops = revprops
            .iter()
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
            Ok(Root(PooledPtr::initialize(|pool| {
                let mut root_ptr = std::ptr::null_mut();
                let err = crate::generated::svn_fs_revision_root(
                    &mut root_ptr,
                    self.0.as_mut_ptr(),
                    rev.0,
                    pool.as_mut_ptr(),
                );
                Error::from_raw(err)?;
                Ok::<_, Error>(root_ptr)
            })?))
        }
    }

    pub fn get_uuid(&mut self) -> Result<String, Error> {
        unsafe {
            let mut pool = apr::pool::Pool::new();
            let mut uuid = std::ptr::null();
            let err = crate::generated::svn_fs_get_uuid(
                self.0.as_mut_ptr(),
                &mut uuid,
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(std::ffi::CStr::from_ptr(uuid)
                .to_string_lossy()
                .into_owned())
        }
    }

    pub fn set_uuid(&mut self, uuid: &str) -> Result<(), Error> {
        unsafe {
            let uuid = std::ffi::CString::new(uuid).unwrap();
            let err = crate::generated::svn_fs_set_uuid(
                self.0.as_mut_ptr(),
                uuid.as_ptr(),
                apr::pool::Pool::new().as_mut_ptr(),
            );
            Error::from_raw(err)?;
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
            let err = crate::generated::svn_fs_unlock(
                self.0.as_mut_ptr(),
                path.as_ptr(),
                token.as_ptr(),
                break_lock as i32,
                apr::pool::Pool::new().as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(())
        }
    }
}

pub fn fs_type(path: &std::path::Path) -> Result<String, Error> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    unsafe {
        let mut pool = apr::pool::Pool::new();
        let mut fs_type = std::ptr::null();
        let err = crate::generated::svn_fs_type(&mut fs_type, path.as_ptr(), pool.as_mut_ptr());
        Error::from_raw(err)?;
        Ok(std::ffi::CStr::from_ptr(fs_type)
            .to_string_lossy()
            .into_owned())
    }
}

pub fn delete_fs(path: &std::path::Path) -> Result<(), Error> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    unsafe {
        let err =
            crate::generated::svn_fs_delete_fs(path.as_ptr(), apr::pool::Pool::new().as_mut_ptr());
        Error::from_raw(err)?;
        Ok(())
    }
}

pub struct Root(pub(crate) PooledPtr<crate::generated::svn_fs_root_t>);
