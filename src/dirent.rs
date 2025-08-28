use crate::Canonical;

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
    pub fn from_str(s: &str, pool: &'a apr::Pool) -> Self {
        Self(
            apr::strings::pstrdup_raw(s, pool).unwrap() as *const i8,
            std::marker::PhantomData,
        )
    }

    /// Create a Dirent from a path, converting to internal style for cross-platform compatibility
    pub fn from_path(path: impl AsRef<std::path::Path>, pool: &'a apr::Pool) -> Self {
        let path_str = path.as_ref().to_string_lossy();
        let path_cstr = std::ffi::CString::new(path_str.as_ref()).unwrap();

        unsafe {
            let internal =
                subversion_sys::svn_dirent_internal_style(path_cstr.as_ptr(), pool.as_mut_ptr());
            Self::from_raw(internal)
        }
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

    /// Convert to local path style for the current platform
    pub fn to_local_style(&self, pool: &apr::Pool) -> std::path::PathBuf {
        unsafe {
            let local = subversion_sys::svn_dirent_local_style(self.0, pool.as_mut_ptr());
            let path_str = std::ffi::CStr::from_ptr(local).to_string_lossy();
            std::path::PathBuf::from(path_str.as_ref())
        }
    }

    pub fn canonicalize<'b>(&'_ self, pool: &'b apr::Pool) -> Canonical<Dirent<'b>>
    where
        'a: 'b,
    {
        unsafe {
            Canonical(Dirent(
                subversion_sys::svn_dirent_canonicalize(self.as_ptr(), pool.as_mut_ptr()),
                std::marker::PhantomData,
            ))
        }
    }

    pub fn canonicalize_in<'b>(&'_ self, pool: &'b mut apr::pool::Pool) -> Canonical<Self>
    where
        'a: 'b,
    {
        unsafe {
            let canonical =
                subversion_sys::svn_dirent_canonicalize(self.as_ptr(), pool.as_mut_ptr());
            Canonical(Self(canonical, std::marker::PhantomData))
        }
    }

    // TODO: This method needs to be reworked to handle lifetime-bound types
    // Cannot return Pooled types with lifetime parameters
    /*pub fn canonicalize_safe(
        &self,
    ) -> Result<(Pooled<Canonical<Self>>, Pooled<Self>), crate::Error> {
        let pool = apr::pool::Pool::new();
        unsafe {
            let mut canonical = std::ptr::null();
            let mut non_canonical = std::ptr::null();
            let err = subversion_sys::svn_dirent_canonicalize_safe(
                &mut canonical,
                &mut non_canonical,
                self.as_ptr(),
                pool.as_mut_ptr(),
                apr::pool::Pool::new().as_mut_ptr(),
            );
            let pool = std::rc::Rc::new(pool);
            Error::from_raw(err)?;
            Ok((
                Pooled::in_pool(
                    pool.clone(),
                    Canonical(Self(canonical, std::marker::PhantomData)),
                ),
                Pooled::in_pool(pool, Self(non_canonical, std::marker::PhantomData)),
            ))
        }
    }*/

    pub fn as_ptr(&self) -> *const i8 {
        self.0
    }

    pub fn is_canonical(&self) -> bool {
        unsafe {
            subversion_sys::svn_dirent_is_canonical(
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
        unsafe { subversion_sys::svn_dirent_is_root(self.as_ptr(), length) != 0 }
    }

    pub fn is_absolute(&self) -> bool {
        unsafe { subversion_sys::svn_dirent_is_absolute(self.as_ptr()) != 0 }
    }

    pub fn dirname<'b>(&self, pool: &'b apr::Pool) -> Dirent<'b> {
        unsafe {
            let dirname_ptr =
                subversion_sys::svn_dirent_dirname(self.as_ptr(), pool.as_mut_ptr()) as *const i8;
            Dirent(dirname_ptr, std::marker::PhantomData)
        }
    }

    pub fn basename<'b>(&self, pool: &'b apr::Pool) -> Dirent<'b> {
        unsafe {
            let basename_ptr =
                subversion_sys::svn_dirent_basename(self.as_ptr(), pool.as_mut_ptr());
            Dirent(basename_ptr, std::marker::PhantomData)
        }
    }

    pub fn split<'b>(&self, pool: &'b apr::Pool) -> (Dirent<'b>, Dirent<'b>) {
        unsafe {
            let mut dirname = std::ptr::null();
            let mut basename = std::ptr::null();
            subversion_sys::svn_dirent_split(
                &mut dirname,
                &mut basename,
                self.as_ptr(),
                pool.as_mut_ptr(),
            );
            (
                Dirent(dirname, std::marker::PhantomData),
                Dirent(basename, std::marker::PhantomData),
            )
        }
    }

    pub fn is_ancestor(&self, other: &Self) -> bool {
        unsafe { subversion_sys::svn_dirent_is_ancestor(self.as_ptr(), other.as_ptr()) != 0 }
    }

    /*pub fn skip_ancestor(&self, child: &Pooled<Self>) -> Option<Pooled<Self>> {
        unsafe {
            let result = subversion_sys::svn_dirent_skip_ancestor(self.as_ptr(), child.as_ptr());
            if result.is_null() {
                Some(Pooled::in_pool(
                    child.pool(),
                    Self(result, std::marker::PhantomData),
                ))
            } else {
                None
            }
        }
    }*/

    /*pub fn is_under_root(
        &self,
        root: &'_ Canonical<Self>,
    ) -> Result<Option<Pooled<Canonical<Self>>>, crate::Error> {
        let mut under_root = 0;
        let mut result_path = std::ptr::null();

        unsafe {
            let result_path = Pooled::initialize(|pool| {
                let err = subversion_sys::svn_dirent_is_under_root(
                    &mut under_root,
                    &mut result_path,
                    root.as_ptr(),
                    self.as_ptr(),
                    pool.as_mut_ptr(),
                );
                Error::from_raw(err)?;
                Ok::<_, Error>(Canonical(Self(result_path, std::marker::PhantomData)))
            })?;
            if under_root == 0 {
                return Ok(None);
            }
            assert!(!result_path.0 .0.is_null());
            Ok(Some(result_path))
        }
    }*/

    /*pub fn absolute(&self) -> Result<Pooled<Self>, crate::Error> {
        Pooled::initialize(|pool| unsafe {
            let mut result = std::ptr::null();
            let err = subversion_sys::svn_dirent_get_absolute(
                &mut result,
                self.as_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok::<_, Error>(Self(result, std::marker::PhantomData))
        })
    }*/
}

impl<'a> Canonical<Dirent<'a>> {
    /*pub fn to_file_url<'b>(&self) -> Result<Pooled<Uri<'b>>, crate::Error> {
        assert!(self.is_canonical());
        Pooled::initialize(|pool| unsafe {
            let mut url = std::ptr::null();
            let err = subversion_sys::svn_uri_get_file_url_from_dirent(
                &mut url,
                self.as_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok::<_, Error>(Uri::from_raw(url))
        })
    }*/
}

/*impl<'a, 'b> TryFrom<Canonical<Dirent<'a>>> for Pooled<Uri<'b>> {
    type Error = crate::Error;

    fn try_from(dirent: Canonical<Dirent<'a>>) -> Result<Self, Self::Error> {
        dirent.to_file_url()
    }
}*/

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
        // SAFETY: We need to cast the lifetime, but scratch_pool outlives the return value
        let ptr = apr::strings::pstrdup_raw(self, scratch_pool).unwrap() as *const i8;
        let dirent = Dirent(ptr, std::marker::PhantomData);
        dirent.canonicalize_in(scratch_pool)
    }
}

impl<'a> AsCanonicalDirent<'a> for &std::path::PathBuf {
    fn as_canonical_dirent(self, scratch_pool: &mut apr::pool::Pool) -> Canonical<Dirent<'a>> {
        // SAFETY: We need to cast the lifetime, but scratch_pool outlives the return value
        let ptr =
            apr::strings::pstrdup_raw(self.to_str().unwrap(), scratch_pool).unwrap() as *const i8;
        let dirent = Dirent(ptr, std::marker::PhantomData);
        dirent.canonicalize_in(scratch_pool)
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
        // SAFETY: We need to cast the lifetime, but scratch_pool outlives the return value
        let ptr =
            apr::strings::pstrdup_raw(self.to_str().unwrap(), scratch_pool).unwrap() as *const i8;
        let dirent = Dirent(ptr, std::marker::PhantomData);
        dirent.canonicalize_in(scratch_pool)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_as_str() {
        let pool = apr::pool::Pool::new();
        let dirent = Dirent::from_str("/foo/bar", &pool);
        assert_eq!("/foo/bar", dirent.as_str());
    }

    #[test]
    fn test_canonicalize() {
        let pool = apr::pool::Pool::new();
        let dirent = Dirent::from_str("/foo/bar", &pool);
        assert!(dirent.is_canonical());

        let mut canon_pool = apr::pool::Pool::new();
        let canonical = dirent.canonicalize_in(&mut canon_pool);
        assert_eq!("/foo/bar", canonical.as_str());
    }

    #[test]
    fn test_dirname() {
        let pool = apr::pool::Pool::new();
        let dirent = Dirent::from_str("/foo/bar", &pool);
        let dirname = dirent.dirname(&pool);
        assert_eq!("/foo", dirname.as_str());
    }

    #[test]
    fn test_basename() {
        let pool = apr::pool::Pool::new();
        let dirent = Dirent::from_str("/foo/bar", &pool);
        let basename = dirent.basename(&pool);
        assert_eq!("bar", basename.as_str());
    }

    #[test]
    fn test_split() {
        let pool = apr::pool::Pool::new();
        let dirent = Dirent::from_str("/foo/bar", &pool);
        let (dirname, basename) = dirent.split(&pool);
        assert_eq!("/foo", dirname.as_str());
        assert_eq!("bar", basename.as_str());
    }

    #[test]
    fn test_from_path() {
        let pool = apr::pool::Pool::new();
        let path = std::path::Path::new("/foo/bar");
        let dirent = Dirent::from_path(path, &pool);
        // The internal style should handle the path correctly
        assert!(dirent.as_str().contains("foo"));
        assert!(dirent.as_str().contains("bar"));
    }

    #[test]
    fn test_to_local_style() {
        let pool = apr::pool::Pool::new();
        let dirent = Dirent::from_str("/foo/bar", &pool);
        let local_path = dirent.to_local_style(&pool);
        // Should return a valid PathBuf
        assert!(local_path.to_str().is_some());
    }

    // TODO: Uncomment when to_file_url is reimplemented
    /*#[test]
    fn test_to_uri() {
        let pool = apr::pool::Pool::new();
        let dirent = Dirent::from_str("/foo/bar", &pool);
        let canonical = dirent.canonicalize_in(&mut canon_pool);
        assert_eq!(
            "file:///foo/bar",
            canonical.to_file_url().unwrap().to_string()
        );
    }*/

    // TODO: Uncomment when is_under_root is reimplemented
    /*#[test]
    fn test_is_under_root() {
        let pool = apr::pool::Pool::new();
        let root = Dirent::from_str("/foo", &pool);
        let child = Dirent::from_str("bar", &pool);
        let canonical_root = root.canonicalize_in(&mut canon_pool);
        let result_path = child.is_under_root(&canonical_root).unwrap();
        assert!(result_path.is_some());
    }*/
}
