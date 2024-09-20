use crate::generated::svn_ra_session_t;
use crate::{Depth, Error, Revnum};
use apr::pool::{Pool, PooledPtr};
use crate::config::Config;

pub struct Session(PooledPtr<svn_ra_session_t>);

impl Session {
    pub fn open(url: &str, uuid: Option<&str>, mut callbacks: Option<&mut Callbacks>, mut config: Option<&mut Config>) -> Result<(Self, Option<String>, Option<String>), Error> {
        let url = std::ffi::CString::new(url).unwrap();
        let mut corrected_url = std::ptr::null();
        let mut redirect_url = std::ptr::null();
        let mut pool = Pool::new();
        let mut session = std::ptr::null_mut();
        let uuid = if let Some(uuid) = uuid {
            Some(std::ffi::CString::new(uuid).unwrap())
        } else {
            None
        };
        let err = unsafe {
            crate::generated::svn_ra_open5(
                &mut session,
                &mut corrected_url,
                &mut redirect_url,
                url.as_ptr(),
                if let Some(uuid) = uuid {
                    uuid.as_ptr()
                } else {
                    std::ptr::null()
                },
                if let Some(callbacks) = callbacks.as_mut() {
                    callbacks.as_mut_ptr()
                } else {
                    Callbacks::default().as_mut_ptr()
                },
                std::ptr::null_mut(),
                if let Some(config) = config.as_mut() {
                    config.as_mut_ptr()
                } else {
                    std::ptr::null_mut()
                },
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok((Self::from_raw(unsafe { PooledPtr::in_pool(std::rc::Rc::new(pool), session) }), if corrected_url.is_null() {
            None
        } else {
            Some(unsafe { std::ffi::CStr::from_ptr(corrected_url) }.to_str().unwrap().to_string())
        },
        if redirect_url.is_null() {
            None
        } else {
            Some(unsafe { std::ffi::CStr::from_ptr(redirect_url) }.to_str().unwrap().to_string())
        }))
    }

    pub fn reparent(&mut self, url: &str) -> Result<(), Error> {
        let url = std::ffi::CString::new(url).unwrap();
        let mut pool = Pool::new();
        let err = unsafe {
            crate::generated::svn_ra_reparent(self.0.as_mut_ptr(), url.as_ptr(), pool.as_mut_ptr())
        };
        Error::from_raw(err)?;
        Ok(())
    }

    pub fn from_raw(raw: PooledPtr<svn_ra_session_t>) -> Self {
        Self(raw)
    }

    pub fn get_session_url(&mut self) -> Result<String, Error> {
        let mut pool = Pool::new();
        let mut url = std::ptr::null();
        let err = unsafe {
            crate::generated::svn_ra_get_session_url(
                self.0.as_mut_ptr(),
                &mut url,
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        let url = unsafe { std::ffi::CStr::from_ptr(url) };
        Ok(url.to_string_lossy().into_owned())
    }

    pub fn get_path_relative_to_session(&mut self, url: &str) -> Result<String, Error> {
        let url = std::ffi::CString::new(url).unwrap();
        let mut pool = Pool::new();
        let mut path = std::ptr::null();
        let err = unsafe {
            crate::generated::svn_ra_get_path_relative_to_session(
                self.0.as_mut_ptr(),
                &mut path,
                url.as_ptr(),
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        let path = unsafe { std::ffi::CStr::from_ptr(path) };
        Ok(path.to_string_lossy().into_owned())
    }

    pub fn get_path_relative_to_root(&mut self, url: &str) -> String {
        let url = std::ffi::CString::new(url).unwrap();
        let mut pool = Pool::new();
        let mut path = std::ptr::null();
        unsafe {
            crate::generated::svn_ra_get_path_relative_to_root(
                self.0.as_mut_ptr(),
                &mut path,
                url.as_ptr(),
                pool.as_mut_ptr(),
            );
        }
        let path = unsafe { std::ffi::CStr::from_ptr(path) };
        path.to_string_lossy().into_owned()
    }

    pub fn get_latest_revnum(&mut self) -> Result<Revnum, Error> {
        let mut pool = Pool::new();
        let mut revnum = 0;
        let err = unsafe {
            crate::generated::svn_ra_get_latest_revnum(
                self.0.as_mut_ptr(),
                &mut revnum,
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(revnum)
    }

    pub fn get_dated_revision(&mut self, tm: impl apr::time::IntoTime) -> Result<Revnum, Error> {
        let mut pool = Pool::new();
        let mut revnum = 0;
        let err = unsafe {
            crate::generated::svn_ra_get_dated_revision(
                self.0.as_mut_ptr(),
                &mut revnum,
                tm.as_apr_time().into(),
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(revnum)
    }

    pub fn change_revprop(
        &mut self,
        rev: Revnum,
        name: &str,
        old_value: Option<&[u8]>,
        new_value: &[u8],
    ) -> Result<(), Error> {
        let name = std::ffi::CString::new(name).unwrap();
        let mut pool = Pool::new();
        let new_value = crate::generated::svn_string_t {
            data: new_value.as_ptr() as *mut _,
            len: new_value.len(),
        };
        let old_value = old_value.map(|v| crate::generated::svn_string_t {
            data: v.as_ptr() as *mut _,
            len: v.len(),
        });
        let err = unsafe {
            crate::generated::svn_ra_change_rev_prop2(
                self.0.as_mut_ptr(),
                rev,
                name.as_ptr(),
                &old_value.map_or(std::ptr::null(), |v| &v),
                &new_value,
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(())
    }

    pub fn rev_proplist(
        &mut self,
        rev: Revnum,
    ) -> Result<std::collections::HashMap<String, Vec<u8>>, Error> {
        let mut pool = Pool::new();
        let mut props = std::ptr::null_mut();
        let err = unsafe {
            crate::generated::svn_ra_rev_proplist(
                self.0.as_mut_ptr(),
                rev,
                &mut props,
                pool.as_mut_ptr(),
            )
        };
        let mut hash =
            apr::hash::Hash::<&str, *const crate::generated::svn_string_t>::from_raw(unsafe {
                PooledPtr::in_pool(std::rc::Rc::new(pool), props)
            });
        Error::from_raw(err)?;
        Ok(hash
            .iter()
            .map(|(k, v)| {
                (
                    String::from_utf8_lossy(k).into_owned(),
                    Vec::from(unsafe {
                        std::slice::from_raw_parts((**v).data as *const u8, (**v).len)
                    }),
                )
            })
            .collect())
    }

    pub fn rev_prop(&mut self, rev: Revnum, name: &str) -> Result<Option<Vec<u8>>, Error> {
        let name = std::ffi::CString::new(name).unwrap();
        let mut pool = Pool::new();
        let mut value = std::ptr::null_mut();
        let err = unsafe {
            crate::generated::svn_ra_rev_prop(
                self.0.as_mut_ptr(),
                rev,
                name.as_ptr(),
                &mut value,
                pool.as_mut_ptr(),
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
}

pub fn version() -> crate::Version {
    unsafe { crate::Version(crate::generated::svn_ra_version()) }
}

pub trait Reporter {
    fn set_path(&mut self, path: &str, rev: Revnum, depth: Depth, start_empty: bool, lock_token: &str) -> Result<(), Error>;

    fn delete_path(&mut self, path: &str) -> Result<(), Error>;

    fn link_path(&mut self, path: &str, url: &str, rev: Revnum, depth: Depth, start_empty: bool, lock_token: &str) -> Result<(), Error>;

    fn finish_report(&mut self) -> Result<(), Error>;

    fn abort_report(&mut self) -> Result<(), Error>;
}

pub struct Callbacks(PooledPtr<crate::generated::svn_ra_callbacks2_t>);

impl Default for Callbacks {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

impl Callbacks {
    pub fn new() -> Result<Callbacks, crate::Error> {
        Ok(Callbacks(PooledPtr::initialize(|pool| unsafe {
            let mut callbacks = std::ptr::null_mut();
            let err = crate::generated::svn_ra_create_callbacks(&mut callbacks, pool.as_mut_ptr());
            Error::from_raw(err)?;
            Ok::<_, crate::Error>(callbacks)
        })?))
    }

    fn as_mut_ptr(&mut self) -> *mut crate::generated::svn_ra_callbacks2_t {
        self.0.as_mut_ptr()
    }
}
