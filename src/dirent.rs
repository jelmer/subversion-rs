use crate::uri::Uri;
use crate::Canonical;
use crate::Error;
use apr::pool::PooledPtr;

pub struct Dirent<'a>(*const std::ffi::c_char, std::marker::PhantomData<&'a ()>);

impl std::fmt::Display for Dirent<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.as_str().fmt(f)
    }
}

impl std::fmt::Debug for Dirent<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Dirent").field(&self.to_string()).finish()
    }
}

impl std::cmp::PartialEq for Dirent<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl std::cmp::Eq for Dirent<'_> {}

impl<'a> Dirent<'a> {
    pub fn pooled(s: &str) -> PooledPtr<Self> {
        PooledPtr::initialize(|pool| {
            let ptr = apr::strings::pstrdup(s, pool).as_ptr() as *const i8;
            let dirent: *mut Self = pool.calloc();
            unsafe {
                *dirent = Self(ptr, std::marker::PhantomData);
            }
            Ok::<*mut Self, crate::Error>(dirent)
        })
        .unwrap()
    }

    pub fn from_raw(raw: *const i8) -> Self {
        Self(raw, std::marker::PhantomData)
    }

    pub fn from_cstr(cstr: &std::ffi::CStr) -> Self {
        Self(cstr.as_ptr(), std::marker::PhantomData)
    }

    pub fn as_str(&self) -> &str {
        unsafe { std::ffi::CStr::from_ptr(self.0) }
            .to_str()
            .expect("invalid utf8")
    }

    pub fn canonicalize(&'_ self) -> PooledPtr<Canonical<Self>> {
        PooledPtr::initialize(|pool| unsafe {
            let canonical =
                crate::generated::svn_dirent_canonicalize(self.as_ptr(), pool.as_mut_ptr());
            let dirent: *mut Canonical<Self> = pool.calloc();
            *dirent = Canonical(Self(canonical, std::marker::PhantomData));
            Ok::<*mut Canonical<Self>, crate::Error>(dirent)
        })
        .unwrap()
    }

    pub fn canonicalize_in<'b>(&'_ self, pool: &'b mut apr::pool::Pool) -> Canonical<Self>
    where
        'a: 'b,
    {
        unsafe {
            let canonical =
                crate::generated::svn_dirent_canonicalize(self.as_ptr(), pool.as_mut_ptr());
            Canonical(Self(canonical, std::marker::PhantomData))
        }
    }

    pub fn canonicalize_safe(
        &self,
    ) -> Result<(PooledPtr<Canonical<Self>>, PooledPtr<Self>), crate::Error> {
        let pool = apr::pool::Pool::new();
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
            let canonical_dirent: *mut Canonical<Self> =
                apr::pool::Pool::from_raw(std::rc::Rc::as_ptr(&pool) as *mut _).calloc();
            let non_canonical_dirent: *mut Self =
                apr::pool::Pool::from_raw(std::rc::Rc::as_ptr(&pool) as *mut _).calloc();
            *canonical_dirent = Canonical(Self(canonical, std::marker::PhantomData));
            *non_canonical_dirent = Self(non_canonical, std::marker::PhantomData);
            Ok((
                PooledPtr::in_pool(pool.clone(), canonical_dirent),
                PooledPtr::in_pool(pool, non_canonical_dirent),
            ))
        }
    }

    pub fn as_ptr(&self) -> *const i8 {
        self.0
    }

    pub fn is_canonical(&self) -> bool {
        unsafe {
            crate::generated::svn_dirent_is_canonical(
                self.as_ptr(),
                apr::pool::Pool::new().as_mut_ptr(),
            ) != 0
        }
    }

    pub fn check_canonical(self) -> Option<Canonical<Self>> {
        if self.is_canonical() {
            Some(Canonical(self))
        } else {
            None
        }
    }

    pub fn is_root(&self, length: usize) -> bool {
        unsafe { crate::generated::svn_dirent_is_root(self.as_ptr(), length) != 0 }
    }

    pub fn is_absolute(&self) -> bool {
        unsafe { crate::generated::svn_dirent_is_absolute(self.as_ptr()) != 0 }
    }

    pub fn dirname(&self) -> PooledPtr<Self> {
        PooledPtr::initialize(|pool| unsafe {
            let dirname =
                crate::generated::svn_dirent_dirname(self.as_ptr(), pool.as_mut_ptr()) as *const i8;
            let dirent: *mut Self = pool.calloc();
            *dirent = Self(dirname, std::marker::PhantomData);
            Ok::<*mut Self, crate::Error>(dirent)
        })
        .unwrap()
    }

    pub fn basename(&self) -> PooledPtr<Self> {
        PooledPtr::initialize(|pool| unsafe {
            let basename = crate::generated::svn_dirent_basename(self.as_ptr(), pool.as_mut_ptr());
            let dirent: *mut Self = pool.calloc();
            *dirent = Self(basename, std::marker::PhantomData);
            Ok::<*mut Self, crate::Error>(dirent)
        })
        .unwrap()
    }

    pub fn split(&self) -> (PooledPtr<Self>, PooledPtr<Self>) {
        let pool = apr::pool::Pool::new();
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
            let dirname_dirent: *mut Self =
                apr::pool::Pool::from_raw(std::rc::Rc::as_ptr(&pool) as *mut _).calloc();
            let basename_dirent: *mut Self =
                apr::pool::Pool::from_raw(std::rc::Rc::as_ptr(&pool) as *mut _).calloc();
            *dirname_dirent = Self(dirname, std::marker::PhantomData);
            *basename_dirent = Self(basename, std::marker::PhantomData);
            (
                PooledPtr::in_pool(pool.clone(), dirname_dirent),
                PooledPtr::in_pool(pool, basename_dirent),
            )
        }
    }

    pub fn is_ancestor(&self, other: &Self) -> bool {
        unsafe { crate::generated::svn_dirent_is_ancestor(self.as_ptr(), other.as_ptr()) != 0 }
    }

    pub fn skip_ancestor(&self, child: &PooledPtr<Self>) -> Option<PooledPtr<Self>> {
        unsafe {
            let result = crate::generated::svn_dirent_skip_ancestor(self.as_ptr(), child.0);
            if result.is_null() {
                let result_dirent: *mut Self =
                    apr::pool::Pool::from_raw(std::rc::Rc::as_ptr(&child.pool()) as *mut _)
                        .calloc();
                *result_dirent = Self(result, std::marker::PhantomData);
                Some(PooledPtr::in_pool(child.pool(), result_dirent))
            } else {
                None
            }
        }
    }

    pub fn is_under_root(
        &self,
        root: &'_ Canonical<Self>,
    ) -> Result<Option<PooledPtr<Canonical<Self>>>, crate::Error> {
        let mut under_root = 0;
        let mut result_path = std::ptr::null();

        unsafe {
            let result_path = PooledPtr::initialize(|pool| {
                let err = crate::generated::svn_dirent_is_under_root(
                    &mut under_root,
                    &mut result_path,
                    root.as_ptr(),
                    self.as_ptr(),
                    pool.as_mut_ptr(),
                );
                Error::from_raw(err)?;
                let canonical_dirent: *mut Canonical<Self> = pool.calloc();
                *canonical_dirent = Canonical(Self(result_path, std::marker::PhantomData));
                Ok::<*mut Canonical<Self>, Error>(canonical_dirent)
            })?;
            if under_root == 0 {
                return Ok(None);
            }
            assert!(!result_path.0 .0.is_null());
            Ok(Some(result_path))
        }
    }

    pub fn absolute(&self) -> Result<PooledPtr<Self>, crate::Error> {
        PooledPtr::initialize(|pool| unsafe {
            let mut result = std::ptr::null();
            let err = crate::generated::svn_dirent_get_absolute(
                &mut result,
                self.as_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            let dirent: *mut Self = pool.calloc();
            *dirent = Self(result, std::marker::PhantomData);
            Ok::<*mut Self, Error>(dirent)
        })
    }
}

impl<'a> Canonical<Dirent<'a>> {
    pub fn to_file_url<'b>(&self) -> Result<PooledPtr<Uri<'b>>, crate::Error> {
        assert!(self.is_canonical());
        PooledPtr::initialize(|pool| unsafe {
            let mut url = std::ptr::null();
            let err = crate::generated::svn_uri_get_file_url_from_dirent(
                &mut url,
                self.as_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            let uri: *mut Uri = pool.calloc();
            *uri = Uri::from_raw(url);
            Ok::<*mut Uri, Error>(uri)
        })
    }
}

impl<'a, 'b> TryFrom<Canonical<Dirent<'a>>> for PooledPtr<Uri<'b>> {
    type Error = crate::Error;

    fn try_from(dirent: Canonical<Dirent<'a>>) -> Result<Self, Self::Error> {
        dirent.to_file_url()
    }
}

impl From<Dirent<'_>> for String {
    fn from(dirent: Dirent) -> Self {
        let c = unsafe { std::ffi::CStr::from_ptr(dirent.as_ptr()) };
        c.to_string_lossy().into_owned()
    }
}

impl From<Dirent<'_>> for &str {
    fn from(dirent: Dirent) -> Self {
        let c = unsafe { std::ffi::CStr::from_ptr(dirent.as_ptr()) };
        c.to_str().unwrap()
    }
}

impl<'a> From<&'a std::ffi::CStr> for Dirent<'a> {
    fn from(s: &'a std::ffi::CStr) -> Self {
        Self(s.as_ptr() as *const i8, std::marker::PhantomData)
    }
}

impl<'a> From<Dirent<'a>> for std::path::PathBuf {
    fn from(dirent: Dirent<'a>) -> Self {
        let c = unsafe { std::ffi::CStr::from_ptr(dirent.as_ptr()) };
        std::path::PathBuf::from(c.to_string_lossy().into_owned())
    }
}

pub trait AsCanonicalDirent<'a> {
    fn as_canonical_dirent(self, scratch_pool: &mut apr::pool::Pool) -> Canonical<Dirent<'a>>;
}

impl<'a> AsCanonicalDirent<'a> for &str {
    fn as_canonical_dirent(self, scratch_pool: &mut apr::pool::Pool) -> Canonical<Dirent<'a>> {
        Dirent::pooled(self).canonicalize_in(scratch_pool)
    }
}

impl<'a> AsCanonicalDirent<'a> for &std::path::PathBuf {
    fn as_canonical_dirent(self, scratch_pool: &mut apr::pool::Pool) -> Canonical<Dirent<'a>> {
        Dirent::pooled(self.to_str().unwrap()).canonicalize_in(scratch_pool)
    }
}

impl<'a> AsCanonicalDirent<'a> for Dirent<'a> {
    fn as_canonical_dirent(self, scratch_pool: &mut apr::pool::Pool) -> Canonical<Dirent<'a>> {
        self.canonicalize_in(scratch_pool)
    }
}

impl<'a> AsCanonicalDirent<'a> for Canonical<Dirent<'a>> {
    fn as_canonical_dirent(self, _scratch_pool: &mut apr::pool::Pool) -> Canonical<Dirent<'a>> {
        self
    }
}

impl<'a> AsCanonicalDirent<'a> for &'a std::path::Path {
    fn as_canonical_dirent(self, scratch_pool: &mut apr::pool::Pool) -> Canonical<Dirent<'a>> {
        Dirent::pooled(self.to_str().unwrap()).canonicalize_in(scratch_pool)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_as_str() {
        let dirent = super::Dirent::pooled("/foo/bar");
        assert_eq!("/foo/bar", dirent.as_str());
    }

    #[test]
    fn test_canonicalize() {
        let dirent = super::Dirent::pooled("/foo/bar");
        assert!(dirent.is_canonical());
        assert_eq!(*super::Dirent::pooled("/foo/bar"), *dirent);
        assert_eq!(*super::Dirent::pooled("/foo/bar"), **dirent.canonicalize());
    }

    #[test]
    fn test_to_uri() {
        let dirent = super::Dirent::pooled("/foo/bar");
        assert_eq!(
            *super::Uri::pooled("file:///foo/bar"),
            *dirent.canonicalize().to_file_url().unwrap()
        );
    }

    #[test]
    fn test_is_under_root() {
        let root = super::Dirent::pooled("/foo").canonicalize();
        let child = super::Dirent::pooled("bar");
        let result_path = child.is_under_root(&root).unwrap();
        assert!(result_path.is_some());
        assert_eq!(*super::Dirent::pooled("/foo/bar"), **result_path.unwrap());
    }
}
