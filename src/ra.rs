use crate::generated::svn_ra_session_t;
use crate::{Error, Revnum};
use apr::pool::{Pool, PooledPtr};

pub struct Session<'pool>(PooledPtr<'pool, svn_ra_session_t>);

impl<'pool> Session<'pool> {
    pub fn reparent(&mut self, url: &str) -> Result<(), Error> {
        let url = std::ffi::CString::new(url).unwrap();
        let mut pool = Pool::new();
        let err = unsafe {
            crate::generated::svn_ra_reparent(self.0.as_mut_ptr(), url.as_ptr(), pool.as_mut_ptr())
        };
        Error::from_raw(err)?;
        Ok(())
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
                rev.into(),
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
                rev.into(),
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
                rev.into(),
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
