use crate::apr::Pool;
use crate::generated::{
    svn_client_checkout3, svn_client_create_context2, svn_client_ctx_t, svn_client_version,
};
use crate::{Depth, Error, Revision, Revnum, Version};

pub fn version() -> Version {
    unsafe {
        let version = svn_client_version();
        Version(version)
    }
}

pub struct Context(*mut svn_client_ctx_t, Pool);

impl Context {
    pub fn new(pool: &mut Pool) -> Self {
        // call svn_client_create_context2
        let mut ctx = std::ptr::null_mut();
        let mut pool = pool.subpool();
        unsafe {
            svn_client_create_context2(&mut ctx, std::ptr::null_mut(), (&mut pool).into());
        }
        Context(ctx, pool)
    }

    /// Checkout a working copy from url to path.
    pub fn checkout(
        &self,
        url: &str,
        path: &std::path::Path,
        peg_revision: Revision,
        revision: Revision,
        depth: Depth,
        ignore_externals: bool,
        allow_unver_obstructions: bool,
    ) -> Result<Revnum, Error> {
        // call svn_client_checkout2
        let peg_revision = peg_revision.into();
        let revision = revision.into();
        let mut pool = Pool::default();
        unsafe {
            let mut revnum = 0;
            let url = std::ffi::CString::new(url).unwrap();
            let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
            let err = svn_client_checkout3(
                &mut revnum,
                url.as_ptr(),
                path.as_ptr(),
                &peg_revision,
                &revision,
                depth.into(),
                ignore_externals.into(),
                allow_unver_obstructions.into(),
                self.0,
                (&mut pool).into(),
            );
            if err.is_null() {
                Ok(revnum)
            } else {
                Err(Error(err))
            }
        }
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new(&mut Pool::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        let version = version();
        assert_eq!(version.major(), 1);
    }
}
