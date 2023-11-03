use crate::generated::svn_error_t;
use crate::generated::{
    svn_client_add5, svn_client_checkout3, svn_client_commit6, svn_client_create_context2,
    svn_client_ctx_t, svn_client_delete4, svn_client_import5, svn_client_log5, svn_client_mkdir4,
    svn_client_status6, svn_client_switch3, svn_client_update4, svn_client_version,
};
use crate::io::Dirent;
use crate::{Depth, Error, LogEntry, Revision, RevisionRange, Revnum, Version};
use apr::Pool;

pub fn version() -> Version {
    unsafe {
        let version = svn_client_version();
        Version(version)
    }
}

extern "C" fn wrap_commit_callback2(
    commit_info: *const crate::generated::svn_commit_info_t,
    baton: *mut std::ffi::c_void,
    _pool: *mut apr::apr_pool_t,
) -> *mut crate::generated::svn_error_t {
    unsafe {
        let callback = baton as *mut &mut dyn FnMut(&crate::CommitInfo) -> Result<(), Error>;
        let mut callback = Box::from_raw(callback);
        match callback(&crate::CommitInfo(commit_info)) {
            Ok(()) => std::ptr::null_mut(),
            Err(err) => err.0,
        }
    }
}

extern "C" fn wrap_filter_callback(
    baton: *mut std::ffi::c_void,
    filtered: *mut crate::generated::svn_boolean_t,
    local_abspath: *const i8,
    dirent: *const crate::generated::svn_io_dirent2_t,
    _pool: *mut apr::apr_pool_t,
) -> *mut svn_error_t {
    unsafe {
        let callback =
            baton as *mut &mut dyn FnMut(&std::path::Path, &Dirent) -> Result<bool, Error>;
        let mut callback = Box::from_raw(callback);
        let local_abspath: &std::path::Path = std::ffi::CStr::from_ptr(local_abspath)
            .to_str()
            .unwrap()
            .as_ref();
        let ret = callback(local_abspath, &Dirent::from(dirent));
        if let Ok(ret) = ret {
            *filtered = ret as crate::generated::svn_boolean_t;
            std::ptr::null_mut()
        } else {
            ret.unwrap_err().0
        }
    }
}

extern "C" fn wrap_status_func(
    baton: *mut std::ffi::c_void,
    path: *const i8,
    status: *const crate::generated::svn_client_status_t,
    _pool: *mut apr::apr_pool_t,
) -> *mut crate::generated::svn_error_t {
    unsafe {
        let callback = baton as *mut &mut dyn FnMut(&std::path::Path, &Status) -> Result<(), Error>;
        let mut callback = Box::from_raw(callback);
        let path: &std::path::Path = std::ffi::CStr::from_ptr(path).to_str().unwrap().as_ref();
        let ret = callback(path, &Status(status));
        if let Err(err) = ret {
            err.0
        } else {
            std::ptr::null_mut()
        }
    }
}

extern "C" fn wrap_log_entry_receiver(
    baton: *mut std::ffi::c_void,
    log_entry: *mut crate::generated::svn_log_entry_t,
    _pool: *mut apr::apr_pool_t,
) -> *mut crate::generated::svn_error_t {
    unsafe {
        let callback = baton as *mut &mut dyn FnMut(&LogEntry) -> Result<(), Error>;
        let mut callback = Box::from_raw(callback);
        let ret = callback(&LogEntry(log_entry));
        if let Err(err) = ret {
            err.0
        } else {
            std::ptr::null_mut()
        }
    }
}

/// A client context.
///
/// This is the main entry point for the client library. It holds client specific configuration and
/// callbacks
pub struct Context<'pool>(apr::pool::PooledPtr<'pool, svn_client_ctx_t>);

impl<'pool> Context<'pool> {
    pub fn new() -> Result<Self, Error> {
        // call svn_client_create_context2
        Ok(Context(apr::pool::PooledPtr::initialize(|pool| {
            let mut ctx = std::ptr::null_mut();
            let ret = unsafe {
                svn_client_create_context2(&mut ctx, std::ptr::null_mut(), pool.as_mut_ptr())
            };
            if ret.is_null() {
                Ok(ctx)
            } else {
                Err(Error(ret))
            }
        })?))
    }

    pub fn as_mut_ptr(&mut self) -> *mut svn_client_ctx_t {
        self.0.as_mut_ptr()
    }

    pub fn as_ptr(&self) -> *const svn_client_ctx_t {
        self.0.as_ptr()
    }

    /// Checkout a working copy from url to path.
    pub fn checkout(
        &mut self,
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
                &mut *self.0,
                pool.as_mut_ptr(),
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
        let mut pool = std::rc::Rc::new(Pool::default());
        let mut result_revs = std::ptr::null_mut();
        unsafe {
            let mut ps = apr::tables::ArrayHeader::in_pool(&pool, paths.len());
            for path in paths {
                let path = std::ffi::CString::new(*path).unwrap();
                ps.push(path.as_ptr() as *mut std::ffi::c_void);
            }

            let err = svn_client_update4(
                &mut result_revs,
                ps.as_ptr(),
                &revision.into(),
                depth.into(),
                depth_is_sticky.into(),
                ignore_externals.into(),
                allow_unver_obstructions.into(),
                adds_as_modifications.into(),
                make_parents.into(),
                &mut *self.0,
                std::rc::Rc::get_mut(&mut pool).unwrap().as_mut_ptr(),
            );
            let result_revs: apr::tables::ArrayHeader<Revnum> =
                apr::tables::ArrayHeader::<Revnum>::from_raw_parts(&pool, result_revs);
            if err.is_null() {
                Ok(result_revs.iter().collect())
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
                &mut *self.0,
                pool.as_mut_ptr(),
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
                &mut *self.0,
                pool.as_mut_ptr(),
            );
            if err.is_null() {
                Ok(())
            } else {
                Err(Error(err))
            }
        }
    }

    pub fn mkdir(
        &mut self,
        paths: &[&std::path::Path],
        make_parents: bool,
        revprop_table: std::collections::HashMap<&str, &str>,
        commit_callback: &dyn FnMut(&crate::CommitInfo) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let mut pool = std::rc::Rc::new(Pool::default());
        unsafe {
            let mut rps = apr::hash::Hash::in_pool(&pool);
            for (k, v) in revprop_table {
                rps.set(k, &v);
            }
            let mut ps = apr::tables::ArrayHeader::in_pool(&pool, paths.len());
            for path in paths {
                let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
                ps.push(path.as_ptr() as *mut std::ffi::c_void);
            }
            let commit_callback = Box::into_raw(Box::new(commit_callback));
            let err = svn_client_mkdir4(
                ps.as_ptr(),
                make_parents.into(),
                rps.as_ptr(),
                Some(wrap_commit_callback2),
                commit_callback as *mut std::ffi::c_void,
                &mut *self.0,
                std::rc::Rc::get_mut(&mut pool).unwrap().as_mut_ptr(),
            );
            if err.is_null() {
                Ok(())
            } else {
                Err(Error(err))
            }
        }
    }

    pub fn delete(
        &mut self,
        paths: &[&std::path::Path],
        force: bool,
        keep_local: bool,
        revprop_table: std::collections::HashMap<&str, &str>,
        commit_callback: &dyn FnMut(&crate::CommitInfo) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let mut pool = std::rc::Rc::new(Pool::default());
        unsafe {
            let mut rps = apr::hash::Hash::in_pool(&pool);
            for (k, v) in revprop_table {
                rps.set(k, &v);
            }
            let mut ps = apr::tables::ArrayHeader::in_pool(&pool, paths.len());
            for path in paths {
                let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
                ps.push(path.as_ptr() as *mut std::ffi::c_void);
            }
            let commit_callback = Box::into_raw(Box::new(commit_callback));
            let err = svn_client_delete4(
                ps.as_ptr(),
                force.into(),
                keep_local.into(),
                rps.as_ptr(),
                Some(wrap_commit_callback2),
                commit_callback as *mut std::ffi::c_void,
                &mut *self.0,
                std::rc::Rc::get_mut(&mut pool).unwrap().as_mut_ptr(),
            );
            if err.is_null() {
                Ok(())
            } else {
                Err(Error(err))
            }
        }
    }

    pub fn import(
        &mut self,
        path: &std::path::Path,
        url: &str,
        depth: Depth,
        no_ignore: bool,
        no_autoprops: bool,
        ignore_unknown_node_types: bool,
        revprop_table: std::collections::HashMap<&str, &str>,
        filter_callback: &dyn FnMut(&mut bool, &std::path::Path, &Dirent) -> Result<(), Error>,
        commit_callback: &dyn FnMut(&crate::CommitInfo) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let mut pool = std::rc::Rc::new(Pool::default());
        let mut rps = apr::hash::Hash::in_pool(&pool);
        for (k, v) in revprop_table {
            rps.set(k, &v);
        }

        unsafe {
            let filter_callback = Box::into_raw(Box::new(filter_callback));
            let commit_callback = Box::into_raw(Box::new(commit_callback));
            let err = svn_client_import5(
                path.to_str().unwrap().as_ptr() as *const i8,
                url.as_ptr() as *const i8,
                depth.into(),
                no_ignore.into(),
                no_autoprops.into(),
                ignore_unknown_node_types.into(),
                rps.as_ptr(),
                Some(wrap_filter_callback),
                filter_callback as *mut std::ffi::c_void,
                Some(wrap_commit_callback2),
                commit_callback as *mut std::ffi::c_void,
                &mut *self.0,
                std::rc::Rc::get_mut(&mut pool).unwrap().as_mut_ptr(),
            );
            if err.is_null() {
                Ok(())
            } else {
                Err(Error(err))
            }
        }
    }

    pub fn commit(
        &mut self,
        targets: &[&str],
        depth: Depth,
        keep_locks: bool,
        keep_changelists: bool,
        commit_as_operations: bool,
        include_file_externals: bool,
        include_dir_externals: bool,
        changelists: Option<&[&str]>,
        revprop_table: std::collections::HashMap<&str, &str>,
        commit_callback: &dyn FnMut(&crate::CommitInfo) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let mut pool = std::rc::Rc::new(Pool::default());
        let mut rps = apr::hash::Hash::in_pool(&pool);
        for (k, v) in revprop_table {
            rps.set(k, &v);
        }

        unsafe {
            let mut ps = apr::tables::ArrayHeader::in_pool(&pool, targets.len());
            for target in targets {
                let target = std::ffi::CString::new(*target).unwrap();
                ps.push(target.as_ptr() as *mut std::ffi::c_void);
            }
            let mut cl = apr::tables::ArrayHeader::in_pool(&pool, 0);
            if let Some(changelists) = changelists {
                for changelist in changelists {
                    let changelist = std::ffi::CString::new(*changelist).unwrap();
                    cl.push(changelist.as_ptr() as *mut std::ffi::c_void);
                }
            }
            let commit_callback = Box::into_raw(Box::new(commit_callback));
            let err = svn_client_commit6(
                ps.as_ptr(),
                depth.into(),
                keep_locks.into(),
                keep_changelists.into(),
                commit_as_operations.into(),
                include_file_externals.into(),
                include_dir_externals.into(),
                cl.as_ptr(),
                rps.as_ptr(),
                Some(wrap_commit_callback2),
                commit_callback as *mut std::ffi::c_void,
                &mut *self.0,
                std::rc::Rc::get_mut(&mut pool).unwrap().as_mut_ptr(),
            );
            if err.is_null() {
                Ok(())
            } else {
                Err(Error(err))
            }
        }
    }

    pub fn status(
        &mut self,
        path: &str,
        revision: Revision,
        depth: Depth,
        get_all: bool,
        check_out_of_date: bool,
        check_working_copy: bool,
        no_ignore: bool,
        ignore_externals: bool,
        depth_as_sticky: bool,
        changelists: Option<&[&str]>,
        status_func: &dyn FnMut(&str, &Status) -> Result<(), Error>,
    ) -> Result<Revnum, Error> {
        let mut pool = std::rc::Rc::new(Pool::default());
        let mut cl = apr::tables::ArrayHeader::in_pool(&pool, 0);
        if let Some(changelists) = changelists {
            for changelist in changelists {
                let changelist = std::ffi::CString::new(*changelist).unwrap();
                cl.push(changelist.as_ptr() as *mut std::ffi::c_void);
            }
        }

        unsafe {
            let status_func = Box::into_raw(Box::new(status_func));
            let mut revnum = 0;
            let err = svn_client_status6(
                &mut revnum,
                &mut *self.0,
                path.as_ptr() as *const i8,
                &revision.into(),
                depth.into(),
                get_all.into(),
                check_out_of_date.into(),
                check_working_copy.into(),
                no_ignore.into(),
                ignore_externals.into(),
                depth_as_sticky.into(),
                cl.as_ptr(),
                Some(wrap_status_func),
                status_func as *mut std::ffi::c_void,
                std::rc::Rc::get_mut(&mut pool).unwrap().as_mut_ptr(),
            );
            if err.is_null() {
                Ok(revnum as Revnum)
            } else {
                Err(Error(err))
            }
        }
    }

    pub fn log(
        &mut self,
        targets: &[&str],
        peg_revision: Revision,
        revision_ranges: &[RevisionRange],
        limit: i32,
        discover_changed_paths: bool,
        strict_node_history: bool,
        include_merged_revisions: bool,
        revprops: &[&str],
        log_entry_receiver: &dyn FnMut(&LogEntry) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let mut pool = std::rc::Rc::new(Pool::default());

        unsafe {
            let mut ps = apr::tables::ArrayHeader::in_pool(&pool, targets.len());
            for target in targets {
                let target = std::ffi::CString::new(*target).unwrap();
                ps.push(target.as_ptr() as *mut std::ffi::c_void);
            }
            let mut rrs = apr::tables::ArrayHeader::<*mut crate::generated::svn_opt_revision_range_t>::in_pool(&pool, revision_ranges.len());
            for revision_range in revision_ranges {
                rrs.push(revision_range.to_c(std::rc::Rc::get_mut(&mut pool).unwrap()));
            }
            let mut rps = apr::tables::ArrayHeader::in_pool(&pool, revprops.len());
            for revprop in revprops {
                let revprop = std::ffi::CString::new(*revprop).unwrap();
                rps.push(revprop.as_ptr() as *mut std::ffi::c_void);
            }
            let log_entry_receiver = Box::into_raw(Box::new(log_entry_receiver));
            let err = svn_client_log5(
                ps.as_ptr(),
                &peg_revision.into(),
                rrs.as_ptr(),
                limit,
                discover_changed_paths.into(),
                strict_node_history.into(),
                include_merged_revisions.into(),
                rps.as_ptr(),
                Some(wrap_log_entry_receiver),
                log_entry_receiver as *mut std::ffi::c_void,
                &mut *self.0,
                std::rc::Rc::get_mut(&mut pool).unwrap().as_mut_ptr(),
            );
            if err.is_null() {
                Ok(())
            } else {
                Err(Error(err))
            }
        }
    }
}

impl<'pool> Default for Context<'pool> {
    fn default() -> Self {
        Self::new().unwrap()
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

pub struct Status(pub(crate) *const crate::generated::svn_client_status_t);
