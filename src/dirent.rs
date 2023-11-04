use crate::uri::Uri;
use crate::Error;
use apr::pool::Pooled;

pub struct Dirent<'pool>(pub(crate) Pooled<'pool, *const i8>);

impl ToString for Dirent<'_> {
    fn to_string(&self) -> String {
        let c = unsafe { std::ffi::CStr::from_ptr(self.as_ptr()) };
        c.to_string_lossy().into_owned()
    }
}

impl<'pool> Dirent<'pool> {
    pub fn canonicalize(&self) -> Self {
        Self(
            Pooled::initialize(|pool| unsafe {
                Ok::<_, crate::Error>(crate::generated::svn_dirent_canonicalize(
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
            let err = crate::generated::svn_dirent_canonicalize_safe(
                &mut canonical,
                &mut non_canonical,
                self.as_ptr(),
                pool.as_mut_ptr(),
                apr::pool::Pool::new().as_mut_ptr(),
            );
            let pool = std::rc::Rc::new(pool);
            Error::from_raw(err)?;
            Ok((
                Self(Pooled::in_pool(pool.clone(), canonical)),
                Self(Pooled::in_pool(pool, non_canonical)),
            ))
        }
    }

    pub fn as_ptr(&self) -> *const i8 {
        self.0.data
    }

    pub fn is_canonical(&self) -> bool {
        unsafe {
            crate::generated::svn_dirent_is_canonical(
                self.as_ptr(),
                apr::pool::Pool::new().as_mut_ptr(),
            ) != 0
        }
    }

    pub fn is_root(&self, length: usize) -> bool {
        unsafe { crate::generated::svn_dirent_is_root(self.as_ptr(), length) != 0 }
    }

    pub fn is_absolute(&self) -> bool {
        unsafe { crate::generated::svn_dirent_is_absolute(self.as_ptr()) != 0 }
    }

    pub fn dirname(&self) -> Self {
        Self(
            Pooled::initialize(|pool| unsafe {
                Ok::<_, crate::Error>(crate::generated::svn_dirent_dirname(
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
                Ok::<_, crate::Error>(crate::generated::svn_dirent_basename(
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
            crate::generated::svn_dirent_split(
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

    pub fn is_ancestor(&self, other: &Self) -> bool {
        unsafe { crate::generated::svn_dirent_is_ancestor(self.as_ptr(), other.as_ptr()) != 0 }
    }

    pub fn skip_ancestor(&self, child: &Self) -> Option<Self> {
        unsafe {
            let result = crate::generated::svn_dirent_skip_ancestor(self.as_ptr(), child.as_ptr());
            if result.is_null() {
                Some(Self(Pooled::in_pool(child.0.pool(), result)))
            } else {
                None
            }
        }
    }

    fn to_file_url<'a>(&self) -> Result<Uri<'a>, crate::Error> {
        Ok(Uri(Pooled::initialize(|pool| unsafe {
            let mut url = std::ptr::null();
            let err = crate::generated::svn_uri_get_file_url_from_dirent(
                &mut url,
                self.as_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(url)
        })?))
    }

    pub fn is_under_root(&self, root: &'_ Self) -> Result<(bool, Self), crate::Error> {
        let mut under_root = 0;
        let mut result_path = std::ptr::null();

        unsafe {
            let result_path = Pooled::initialize(|pool| {
                let err = crate::generated::svn_dirent_is_under_root(
                    &mut under_root,
                    &mut result_path,
                    self.as_ptr(),
                    root.as_ptr(),
                    pool.as_mut_ptr(),
                );
                Error::from_raw(err)?;
                Ok(result_path)
            })?;
            Ok((under_root != 0, Self(result_path)))
        }
    }
}

impl<'a, 'b> TryFrom<Dirent<'a>> for Uri<'b> {
    type Error = crate::Error;

    fn try_from(dirent: Dirent<'a>) -> Result<Self, Self::Error> {
        dirent.to_file_url()
    }
}

impl<'pool> From<Dirent<'pool>> for String {
    fn from(dirent: Dirent<'pool>) -> Self {
        let c = unsafe { std::ffi::CStr::from_ptr(dirent.as_ptr()) };
        c.to_string_lossy().into_owned()
    }
}

impl<'pool> From<&'pool Dirent<'pool>> for &'pool str {
    fn from(dirent: &'pool Dirent<'pool>) -> Self {
        let c = unsafe { std::ffi::CStr::from_ptr(dirent.as_ptr()) };
        c.to_str().unwrap()
    }
}

impl<'pool> From<&'pool str> for Dirent<'pool> {
    fn from(s: &'pool str) -> Self {
        Self(
            Pooled::initialize(|_pool| {
                Ok::<_, crate::Error>(std::ffi::CString::new(s).unwrap().into_raw() as *const i8)
            })
            .unwrap(),
        )
    }
}

impl<'pool> From<&'pool std::path::Path> for Dirent<'pool> {
    fn from(path: &'pool std::path::Path) -> Self {
        Self(
            Pooled::initialize(|_pool| {
                Ok::<_, crate::Error>(
                    std::ffi::CString::new(path.to_str().unwrap())
                        .unwrap()
                        .into_raw() as *const i8,
                )
            })
            .unwrap(),
        )
    }
}
