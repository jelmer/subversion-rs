use crate::generated::{
    svn_client_add5, svn_client_checkout3, svn_client_create_context2, svn_client_ctx_t,
    svn_client_mkdir4, svn_client_switch3, svn_client_update4, svn_client_version,
};
use crate::{Depth, Error, Revision, Revnum, Version};
use apr::Pool;

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

    pub fn update(
        &mut self,
        paths: &[&str],
        revision: Revision,
        depth: Depth,
        depth_is_sticky: bool,
        ignore_externals: bool,
        allow_unver_obstructions: bool,
        adds_as_modifications: bool,
        make_parents: bool,
    ) -> Result<Vec<Revnum>, Error> {
        let mut pool = Pool::default();
        let mut result_revs = std::ptr::null_mut();
        unsafe {
            let mut ps = apr::tables::ArrayHeader::new::<*const i8>(&mut pool, paths.len());
            for path in paths {
                let path = std::ffi::CString::new(*path).unwrap();
                ps.push(path.as_ptr() as *mut std::ffi::c_void);
            }

            let err = svn_client_update4(
                &mut result_revs,
                ps.into(),
                &revision.into(),
                depth.into(),
                depth_is_sticky.into(),
                ignore_externals.into(),
                allow_unver_obstructions.into(),
                adds_as_modifications.into(),
                make_parents.into(),
                self.0,
                (&mut pool).into(),
            );
            let result_revs: apr::tables::ArrayHeader = result_revs.into();
            if err.is_null() {
                Ok(result_revs.iter().map(|r| r as Revnum).collect())
            } else {
                Err(Error(err))
            }
        }
    }

    pub fn switch(
        &mut self,
        path: &std::path::Path,
        url: &str,
        peg_revision: Revision,
        revision: Revision,
        depth: Depth,
        depth_is_sticky: bool,
        ignore_externals: bool,
        allow_unver_obstructions: bool,
        make_parents: bool,
    ) -> Result<Revnum, Error> {
        let mut pool = Pool::default();
        let mut result_rev = 0;
        unsafe {
            let err = svn_client_switch3(
                &mut result_rev,
                path.to_str().unwrap().as_ptr() as *const i8,
                url.as_ptr() as *const i8,
                &peg_revision.into(),
                &revision.into(),
                depth.into(),
                depth_is_sticky.into(),
                ignore_externals.into(),
                allow_unver_obstructions.into(),
                make_parents.into(),
                self.0,
                (&mut pool).into(),
            );
            if err.is_null() {
                Ok(result_rev)
            } else {
                Err(Error(err))
            }
        }
    }

    pub fn add(
        &mut self,
        path: &std::path::Path,
        depth: Depth,
        force: bool,
        no_ignore: bool,
        no_autoprops: bool,
        add_parents: bool,
    ) -> Result<(), Error> {
        let mut pool = Pool::default();
        unsafe {
            let err = svn_client_add5(
                path.to_str().unwrap().as_ptr() as *const i8,
                depth.into(),
                force.into(),
                no_ignore.into(),
                no_autoprops.into(),
                add_parents.into(),
                self.0,
                (&mut pool).into(),
            );
            if err.is_null() {
                Ok(())
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
