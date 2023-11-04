use crate::dirent::Dirent;
use crate::Error;
use apr::pool::Pooled;

pub struct Uri(pub(crate) Pooled<*const i8>);

impl ToString for Uri {
    fn to_string(&self) -> String {
        let t = unsafe { std::ffi::CStr::from_ptr(self.as_ptr()) };
        t.to_str().unwrap().to_string()
    }
}

impl Uri {
    pub fn is_root(&self, length: usize) -> bool {
        unsafe { crate::generated::svn_uri_is_root(self.as_ptr(), length) != 0 }
    }

    pub fn as_ptr(&self) -> *const i8 {
        self.0.data
    }

    pub fn dirname(&self) -> Self {
        Self(
            Pooled::initialize(|pool| unsafe {
                Ok::<_, crate::Error>(crate::generated::svn_uri_dirname(
                    self.as_ptr(),
                    pool.as_mut_ptr(),
                ) as *const i8)
            })
            .unwrap(),
        )
    }

    pub fn basename(&self) -> Self {
        Self(
            Pooled::initialize(|pool| unsafe {
                Ok::<_, crate::Error>(crate::generated::svn_uri_basename(
                    self.as_ptr(),
                    pool.as_mut_ptr(),
                ))
            })
            .unwrap(),
        )
    }

    pub fn split(&self) -> (Self, Self) {
        let mut pool = apr::pool::Pool::new();
        unsafe {
            let mut dirname = std::ptr::null();
            let mut basename = std::ptr::null();
            crate::generated::svn_uri_split(
                &mut dirname,
                &mut basename,
                self.as_ptr(),
                pool.as_mut_ptr(),
            );
            let pool = std::rc::Rc::new(pool);
            (
                Self(Pooled::in_pool(pool.clone(), dirname)),
                Self(Pooled::in_pool(pool, basename)),
            )
        }
    }

    pub fn canonicalize(&self) -> Self {
        Self(
            Pooled::initialize(|pool| unsafe {
                Ok::<_, crate::Error>(crate::generated::svn_uri_canonicalize(
                    self.as_ptr(),
                    pool.as_mut_ptr(),
                ))
            })
            .unwrap(),
        )
    }

    pub fn canonicalize_safe(&self) -> Result<(Self, Self), crate::Error> {
        let mut pool = apr::pool::Pool::new();
        unsafe {
            let mut canonical = std::ptr::null();
            let mut non_canonical = std::ptr::null();
            let err = crate::generated::svn_uri_canonicalize_safe(
                &mut canonical,
                &mut non_canonical,
                self.as_ptr(),
                pool.as_mut_ptr(),
                apr::pool::Pool::new().as_mut_ptr(),
            );
            Error::from_raw(err)?;
            let pool = std::rc::Rc::new(pool);
            Ok((
                Self(Pooled::in_pool(pool.clone(), canonical)),
                Self(Pooled::in_pool(pool, non_canonical)),
            ))
        }
    }

    pub fn is_canonical(&self) -> bool {
        unsafe {
            crate::generated::svn_uri_is_canonical(
                self.as_ptr(),
                apr::pool::Pool::new().as_mut_ptr(),
            ) != 0
        }
    }

    fn to_dirent(&self) -> Result<Dirent, crate::Error> {
        Ok(Dirent(Pooled::initialize(|pool| unsafe {
            let mut dirent = std::ptr::null();
            let err = crate::generated::svn_uri_get_dirent_from_file_url(
                &mut dirent,
                self.as_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok::<_, Error>(dirent)
        })?))
    }
}

impl TryFrom<Uri> for Dirent {
    type Error = crate::Error;

    fn try_from(uri: Uri) -> Result<Self, Self::Error> {
        uri.to_dirent()
    }
}

impl From<&str> for Uri {
    fn from(s: &str) -> Self {
        Self(
            Pooled::initialize(|_pool| {
                Ok::<_, crate::Error>(std::ffi::CString::new(s).unwrap().into_raw() as *const i8)
            })
            .unwrap(),
        )
    }
}

impl From<&Uri> for &str {
    fn from(uri: &Uri) -> Self {
        let t = unsafe { std::ffi::CStr::from_ptr(uri.as_ptr()) };
        t.to_str().unwrap()
    }
}

#[cfg(feature = "url")]
impl TryFrom<Uri> for url::Url {
    type Error = crate::Error;

    fn try_from(uri: Uri) -> Result<Self, Self::Error> {
        let uri = uri.to_str()?;
        Ok(url::Url::parse(uri)?)
    }
}
