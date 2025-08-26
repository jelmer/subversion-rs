use crate::dirent::Dirent;
use crate::Canonical;
use crate::Error;
use apr::pool::PooledPtr;

pub struct Uri<'a>(*const i8, std::marker::PhantomData<&'a ()>);

impl ToString for Uri<'_> {
    fn to_string(&self) -> String {
        let t = unsafe { std::ffi::CStr::from_ptr(self.as_ptr()) };
        t.to_str().unwrap().to_string()
    }
}

impl std::fmt::Debug for Uri<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Uri").field(&self.to_string()).finish()
    }
}

impl PartialEq for Uri<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.to_string() == other.to_string()
    }
}

impl Eq for Uri<'_> {}

impl<'a> Uri<'a> {
    pub fn from_raw(raw: *const i8) -> Self {
        Self(raw, std::marker::PhantomData)
    }

    pub fn from_cstr(cstr: &std::ffi::CStr) -> Self {
        Self(cstr.as_ptr(), std::marker::PhantomData)
    }

    pub fn pooled(s: &str) -> PooledPtr<Self> {
        PooledPtr::initialize(|pool| {
            let ptr = apr::strings::pstrdup(s, pool).as_ptr() as *const i8;
            let uri: *mut Self = pool.calloc();
            unsafe {
                *uri = Self(ptr, std::marker::PhantomData);
            }
            Ok::<*mut Self, crate::Error>(uri)
        })
        .unwrap()
    }

    pub fn is_root(&self, length: usize) -> bool {
        unsafe { crate::generated::svn_uri_is_root(self.as_ptr(), length) != 0 }
    }

    pub fn as_ptr(&self) -> *const i8 {
        self.0
    }

    pub fn dirname(&self) -> PooledPtr<Self> {
        PooledPtr::initialize(|pool| unsafe {
            let dirname =
                crate::generated::svn_uri_dirname(self.as_ptr(), pool.as_mut_ptr()) as *const i8;
            let uri: *mut Self = pool.calloc();
            *uri = Self(dirname, std::marker::PhantomData);
            Ok::<*mut Self, crate::Error>(uri)
        })
        .unwrap()
    }

    pub fn basename(&self) -> PooledPtr<Self> {
        PooledPtr::initialize(|pool| unsafe {
            let basename = crate::generated::svn_uri_basename(self.as_ptr(), pool.as_mut_ptr());
            let uri: *mut Self = pool.calloc();
            *uri = Self(basename, std::marker::PhantomData);
            Ok::<*mut Self, crate::Error>(uri)
        })
        .unwrap()
    }

    pub fn split(&self) -> (PooledPtr<Self>, PooledPtr<Self>) {
        let pool = apr::pool::Pool::new();
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
            let dirname_uri: *mut Self =
                apr::pool::Pool::from_raw(std::rc::Rc::as_ptr(&pool) as *mut _).calloc();
            let basename_uri: *mut Self =
                apr::pool::Pool::from_raw(std::rc::Rc::as_ptr(&pool) as *mut _).calloc();
            *dirname_uri = Self(dirname, std::marker::PhantomData);
            *basename_uri = Self(basename, std::marker::PhantomData);
            (
                PooledPtr::in_pool(pool.clone(), dirname_uri),
                PooledPtr::in_pool(pool, basename_uri),
            )
        }
    }

    pub fn canonicalize(&self) -> PooledPtr<Canonical<Self>> {
        PooledPtr::initialize(|pool| unsafe {
            let canonical =
                crate::generated::svn_uri_canonicalize(self.as_ptr(), pool.as_mut_ptr());
            let canonical_uri: *mut Canonical<Self> = pool.calloc();
            *canonical_uri = Canonical(Self(canonical, std::marker::PhantomData));
            Ok::<*mut Canonical<Self>, crate::Error>(canonical_uri)
        })
        .unwrap()
    }

    pub fn canonicalize_in<'b>(&'_ self, pool: &'b mut apr::pool::Pool) -> Canonical<Self>
    where
        'a: 'b,
    {
        Canonical(Self(
            unsafe { crate::generated::svn_uri_canonicalize(self.as_ptr(), pool.as_mut_ptr()) },
            std::marker::PhantomData,
        ))
    }

    pub fn canonicalize_safe(
        &self,
    ) -> Result<(PooledPtr<Canonical<Self>>, PooledPtr<Self>), crate::Error> {
        let pool = apr::pool::Pool::new();
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
            let canonical_uri: *mut Canonical<Self> =
                apr::pool::Pool::from_raw(std::rc::Rc::as_ptr(&pool) as *mut _).calloc();
            let non_canonical_uri: *mut Self =
                apr::pool::Pool::from_raw(std::rc::Rc::as_ptr(&pool) as *mut _).calloc();
            *canonical_uri = Canonical(Self(canonical, std::marker::PhantomData));
            *non_canonical_uri = Self(non_canonical, std::marker::PhantomData);
            Ok((
                PooledPtr::in_pool(pool.clone(), canonical_uri),
                PooledPtr::in_pool(pool, non_canonical_uri),
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

    pub fn check_canonical(self) -> Option<Canonical<Self>> {
        if self.is_canonical() {
            Some(Canonical(self))
        } else {
            None
        }
    }

    fn to_dirent<'b>(&self) -> Result<PooledPtr<Dirent<'b>>, crate::Error> {
        PooledPtr::initialize(|pool| unsafe {
            let mut dirent = std::ptr::null();
            let err = crate::generated::svn_uri_get_dirent_from_file_url(
                &mut dirent,
                self.as_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            let dirent_ptr: *mut Dirent = pool.calloc();
            *dirent_ptr = Dirent::from_raw(dirent);
            Ok::<*mut Dirent, Error>(dirent_ptr)
        })
    }
}

impl<'a, 'b> TryFrom<Canonical<Uri<'a>>> for PooledPtr<Dirent<'b>> {
    type Error = crate::Error;

    fn try_from(uri: Canonical<Uri<'a>>) -> Result<PooledPtr<Dirent<'b>>, Self::Error> {
        uri.to_dirent()
    }
}

impl<'a> From<&'a str> for Uri<'a> {
    fn from(s: &'a str) -> Self {
        Self(
            std::ffi::CString::new(s).unwrap().into_raw(),
            std::marker::PhantomData,
        )
    }
}

impl<'a> From<&Uri<'a>> for &'a str {
    fn from(uri: &Uri<'a>) -> Self {
        let t = unsafe { std::ffi::CStr::from_ptr(uri.as_ptr()) };
        t.to_str().unwrap()
    }
}

#[cfg(feature = "url")]
impl TryFrom<Uri<'_>> for url::Url {
    type Error = url::ParseError;

    fn try_from(uri: Uri) -> Result<Self, Self::Error> {
        let uri = uri.to_string();
        Ok(url::Url::parse(&uri)?)
    }
}

pub trait AsCanonicalUri<'a> {
    fn as_canonical_uri(self, scratch_pool: &mut apr::pool::Pool) -> Canonical<Uri<'a>>;
}

impl<'a> AsCanonicalUri<'a> for Uri<'a> {
    fn as_canonical_uri(self, scratch_pool: &mut apr::pool::Pool) -> Canonical<Uri<'a>> {
        self.canonicalize_in(scratch_pool)
    }
}

impl<'a> AsCanonicalUri<'a> for Canonical<Uri<'a>> {
    fn as_canonical_uri(self, _scratch_pool: &mut apr::pool::Pool) -> Canonical<Uri<'a>> {
        self
    }
}

#[cfg(feature = "url")]
impl<'a> AsCanonicalUri<'a> for url::Url {
    fn as_canonical_uri(self, scratch_pool: &mut apr::pool::Pool) -> Canonical<Uri<'a>> {
        Uri::pooled(self.as_str()).canonicalize_in(scratch_pool)
    }
}

#[cfg(feature = "url")]
impl<'a> AsCanonicalUri<'a> for &'a url::Url {
    fn as_canonical_uri(self, scratch_pool: &mut apr::pool::Pool) -> Canonical<Uri<'a>> {
        Uri::pooled(self.as_str()).canonicalize_in(scratch_pool)
    }
}

impl<'a> AsCanonicalUri<'a> for &'a str {
    fn as_canonical_uri(self, scratch_pool: &mut apr::pool::Pool) -> Canonical<Uri<'a>> {
        Uri::pooled(self).canonicalize_in(scratch_pool)
    }
}

impl<'a> AsCanonicalUri<'a> for PooledPtr<Uri<'a>> {
    fn as_canonical_uri(self, scratch_pool: &mut apr::pool::Pool) -> Canonical<Uri<'a>> {
        self.canonicalize_in(scratch_pool)
    }
}

impl<'a> AsCanonicalUri<'a> for PooledPtr<Canonical<Uri<'a>>> {
    fn as_canonical_uri(self, _scratch_pool: &mut apr::pool::Pool) -> Canonical<Uri<'a>> {
        Canonical(Uri(self.0 .0, std::marker::PhantomData))
    }
}
