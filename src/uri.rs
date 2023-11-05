use crate::dirent::Dirent;
use crate::Canonical;
use crate::Error;
use apr::pool::Pooled;

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

    pub fn pooled(s: &str) -> Pooled<Self> {
        Pooled::initialize(|pool| {
            Ok::<_, crate::Error>(Self(
                apr::strings::pstrdup(s, pool),
                std::marker::PhantomData,
            ))
        })
        .unwrap()
    }

    pub fn is_root(&self, length: usize) -> bool {
        unsafe { crate::generated::svn_uri_is_root(self.as_ptr(), length) != 0 }
    }

    pub fn as_ptr(&self) -> *const i8 {
        self.0
    }

    pub fn dirname(&self) -> Pooled<Self> {
        Pooled::initialize(|pool| unsafe {
            Ok::<_, crate::Error>(Self(
                crate::generated::svn_uri_dirname(self.as_ptr(), pool.as_mut_ptr()) as *const i8,
                std::marker::PhantomData,
            ))
        })
        .unwrap()
    }

    pub fn basename(&self) -> Pooled<Self> {
        Pooled::initialize(|pool| unsafe {
            Ok::<_, crate::Error>(Self(
                crate::generated::svn_uri_basename(self.as_ptr(), pool.as_mut_ptr()),
                std::marker::PhantomData,
            ))
        })
        .unwrap()
    }

    pub fn split(&self) -> (Pooled<Self>, Pooled<Self>) {
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
                Pooled::in_pool(pool.clone(), Self(dirname, std::marker::PhantomData)),
                Pooled::in_pool(pool, Self(basename, std::marker::PhantomData)),
            )
        }
    }

    pub fn canonicalize(&self) -> Pooled<Canonical<Self>> {
        Pooled::initialize(|pool| unsafe {
            Ok::<_, crate::Error>(Canonical(Self(
                crate::generated::svn_uri_canonicalize(self.as_ptr(), pool.as_mut_ptr()),
                std::marker::PhantomData,
            )))
        })
        .unwrap()
    }

    pub fn canonicalize_in(
        &'_ self,
        mut pool: std::rc::Rc<apr::pool::Pool>,
    ) -> Pooled<Canonical<Self>> {
        unsafe {
            let canonical = crate::generated::svn_uri_canonicalize(
                self.as_ptr(),
                std::rc::Rc::get_mut(&mut pool).unwrap().as_mut_ptr(),
            );

            Pooled::in_pool(pool, Canonical(Self(canonical, std::marker::PhantomData)))
        }
    }

    pub fn canonicalize_safe(
        &self,
    ) -> Result<(Pooled<Canonical<Self>>, Pooled<Self>), crate::Error> {
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
                Pooled::in_pool(
                    pool.clone(),
                    Canonical(Self(canonical, std::marker::PhantomData)),
                ),
                Pooled::in_pool(pool, Self(non_canonical, std::marker::PhantomData)),
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

    fn to_dirent<'b>(&self) -> Result<Pooled<Dirent<'b>>, crate::Error> {
        Pooled::initialize(|pool| unsafe {
            let mut dirent = std::ptr::null();
            let err = crate::generated::svn_uri_get_dirent_from_file_url(
                &mut dirent,
                self.as_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok::<_, Error>(Dirent::from_raw(dirent))
        })
    }
}

impl<'a, 'b> TryFrom<Canonical<Uri<'a>>> for Pooled<Dirent<'b>> {
    type Error = crate::Error;

    fn try_from(uri: Canonical<Uri<'a>>) -> Result<Pooled<Dirent<'b>>, Self::Error> {
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
impl TryFrom<Uri> for url::Url {
    type Error = crate::Error;

    fn try_from(uri: Uri) -> Result<Self, Self::Error> {
        let uri = uri.to_str()?;
        Ok(url::Url::parse(uri)?)
    }
}

#[cfg(feature = "url")]
impl From<url::Url> for Uri<'_> {
    fn from(url: url::Url) -> Self {
        Self::new(url.as_str())
    }
}
