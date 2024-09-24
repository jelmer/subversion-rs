use crate::dirent::AsCanonicalDirent;
use crate::generated::svn_error_t;
use crate::generated::{
    svn_client_add5, svn_client_checkout3, svn_client_cleanup2, svn_client_commit6,
    svn_client_conflict_get, svn_client_create_context2, svn_client_ctx_t, svn_client_delete4,
    svn_client_export5, svn_client_import5, svn_client_log5, svn_client_mkdir4,
    svn_client_proplist4, svn_client_relocate2, svn_client_status6, svn_client_switch3,
    svn_client_update4, svn_client_vacuum, svn_client_version,
};
use crate::io::Dirent;
use crate::uri::AsCanonicalUri;
use crate::{Depth, Error, LogEntry, Revision, RevisionRange, Revnum, Version};
use apr::pool::PooledPtr;
use apr::Pool;
use std::collections::HashMap;

pub fn version() -> Version {
    unsafe { Version(svn_client_version()) }
}

pub(crate) extern "C" fn wrap_commit_callback2(
    commit_info: *const crate::generated::svn_commit_info_t,
    baton: *mut std::ffi::c_void,
    pool: *mut apr::apr_pool_t,
) -> *mut crate::generated::svn_error_t {
    unsafe {
        let callback = baton as *mut &mut dyn FnMut(&crate::CommitInfo) -> Result<(), Error>;
        let mut callback = Box::from_raw(callback);
        match callback(&crate::CommitInfo(PooledPtr::in_pool(
            std::rc::Rc::new(Pool::from_raw(pool)),
            commit_info as *mut crate::generated::svn_commit_info_t,
        ))) {
            Ok(()) => std::ptr::null_mut(),
            Err(mut err) => err.as_mut_ptr(),
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
            ret.unwrap_err().as_mut_ptr()
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
        if let Err(mut err) = ret {
            err.as_mut_ptr()
        } else {
            std::ptr::null_mut()
        }
    }
}

pub(crate) extern "C" fn wrap_log_entry_receiver(
    baton: *mut std::ffi::c_void,
    log_entry: *mut crate::generated::svn_log_entry_t,
    pool: *mut apr::apr_pool_t,
) -> *mut crate::generated::svn_error_t {
    unsafe {
        let callback = baton as *mut &mut dyn FnMut(&LogEntry) -> Result<(), Error>;
        let mut callback = Box::from_raw(callback);
        let pool = apr::pool::Pool::from_raw(pool);
        let ret = callback(&LogEntry(apr::pool::PooledPtr::in_pool(
            std::rc::Rc::new(pool),
            log_entry,
        )));
        if let Err(mut err) = ret {
            err.as_mut_ptr()
        } else {
            std::ptr::null_mut()
        }
    }
}

extern "C" fn wrap_proplist_receiver2(
    baton: *mut std::ffi::c_void,
    path: *const i8,
    props: *mut apr::hash::apr_hash_t,
    inherited_props: *mut apr::tables::apr_array_header_t,
    scratch_pool: *mut apr::apr_pool_t,
) -> *mut crate::generated::svn_error_t {
    unsafe {
        let scratch_pool = std::rc::Rc::new(apr::pool::Pool::from_raw(scratch_pool));
        let callback = baton
            as *mut &mut dyn FnMut(
                &str,
                &HashMap<String, Vec<u8>>,
                Option<&[crate::InheritedItem]>,
            ) -> Result<(), Error>;
        let mut callback = Box::from_raw(callback);
        let path: &str = std::ffi::CStr::from_ptr(path).to_str().unwrap();
        let mut props = apr::hash::Hash::<&str, *mut crate::generated::svn_string_t>::from_raw(
            PooledPtr::in_pool(scratch_pool.clone(), props),
        );
        let props = props
            .iter()
            .map(|(k, v)| {
                (
                    String::from_utf8_lossy(k).to_string(),
                    Vec::from_raw_parts((**v).data as *mut u8, (**v).len, (**v).len),
                )
            })
            .collect::<HashMap<_, _>>();
        let inherited_props = if inherited_props.is_null() {
            None
        } else {
            let inherited_props = apr::tables::ArrayHeader::<
                *mut crate::generated::svn_prop_inherited_item_t,
            >::from_raw_parts(&scratch_pool, inherited_props);
            Some(
                inherited_props
                    .iter()
                    .map(|x| {
                        crate::InheritedItem::from_raw(PooledPtr::in_pool(scratch_pool.clone(), x))
                    })
                    .collect::<Vec<_>>(),
            )
        };
        let ret = callback(path, &props, inherited_props.as_deref());
        if let Err(mut err) = ret {
            err.as_mut_ptr()
        } else {
            std::ptr::null_mut()
        }
    }
}

pub struct Info(*const crate::generated::svn_client_info2_t);

impl Info {
    pub fn url(&self) -> &str {
        unsafe {
            let url = (*self.0).URL;
            std::ffi::CStr::from_ptr(url).to_str().unwrap()
        }
    }

    pub fn revision(&self) -> Revnum {
        unsafe { Revnum::from_raw((*self.0).rev).unwrap() }
    }

    pub fn kind(&self) -> crate::generated::svn_node_kind_t {
        unsafe { (*self.0).kind }
    }

    pub fn repos_root_url(&self) -> &str {
        unsafe {
            let url = (*self.0).repos_root_URL;
            std::ffi::CStr::from_ptr(url).to_str().unwrap()
        }
    }

    pub fn repos_uuid(&self) -> &str {
        unsafe {
            let uuid = (*self.0).repos_UUID;
            std::ffi::CStr::from_ptr(uuid).to_str().unwrap()
        }
    }

    pub fn last_changed_rev(&self) -> Revnum {
        Revnum::from_raw(unsafe { (*self.0).last_changed_rev }).unwrap()
    }

    pub fn last_changed_date(&self) -> apr::time::Time {
        unsafe { (*self.0).last_changed_date.into() }
    }

    pub fn last_changed_author(&self) -> &str {
        unsafe {
            let author = (*self.0).last_changed_author;
            std::ffi::CStr::from_ptr(author).to_str().unwrap()
        }
    }
}

extern "C" fn wrap_info_receiver2(
    baton: *mut std::ffi::c_void,
    abspath_or_url: *const i8,
    info: *const crate::generated::svn_client_info2_t,
    _scatch_pool: *mut apr::apr_pool_t,
) -> *mut crate::generated::svn_error_t {
    unsafe {
        let callback = baton as *mut &mut dyn FnMut(&std::path::Path, &Info) -> Result<(), Error>;
        let mut callback = Box::from_raw(callback);
        let abspath_or_url: &std::path::Path = std::ffi::CStr::from_ptr(abspath_or_url)
            .to_str()
            .unwrap()
            .as_ref();
        let ret = callback(abspath_or_url, &Info(info));
        if let Err(mut err) = ret {
            err.as_mut_ptr()
        } else {
            std::ptr::null_mut()
        }
    }
}

/// Options for a checkout
#[derive(Debug, Clone, Copy, Default)]
pub struct CheckoutOptions {
    peg_revision: Revision,
    revision: Revision,
    depth: Depth,
    ignore_externals: bool,
    allow_unver_obstructions: bool,
}

/// Options for an update
#[derive(Debug, Clone, Copy, Default)]
pub struct UpdateOptions {
    depth: Depth,
    depth_is_sticky: bool,
    ignore_externals: bool,
    allow_unver_obstructions: bool,
    adds_as_modifications: bool,
    make_parents: bool,
}

/// Options for a switch
pub struct SwitchOptions {
    peg_revision: Revision,
    revision: Revision,
    depth: Depth,
    depth_is_sticky: bool,
    ignore_externals: bool,
    allow_unver_obstructions: bool,
    make_parents: bool,
}

/// Options for add
pub struct AddOptions {
    depth: Depth,
    force: bool,
    no_ignore: bool,
    no_autoprops: bool,
    add_parents: bool,
}

/// Options for delete
pub struct DeleteOptions<'a> {
    force: bool,
    keep_local: bool,
    commit_callback: &'a dyn FnMut(&crate::CommitInfo) -> Result<(), Error>,
}

/// Options for commit
pub struct CommitOptions<'a> {
    depth: Depth,
    keep_locks: bool,
    keep_changelists: bool,
    commit_as_operations: bool,
    include_file_externals: bool,
    include_dir_externals: bool,
    changelists: Option<&'a [&'a str]>,
}

/// Options for status
pub struct StatusOptions<'a> {
    revision: Revision,
    depth: Depth,
    get_all: bool,
    check_out_of_date: bool,
    check_working_copy: bool,
    no_ignore: bool,
    ignore_externals: bool,
    depth_as_sticky: bool,
    changelists: Option<&'a [&'a str]>,
}

/// A client context.
///
/// This is the main entry point for the client library. It holds client specific configuration and
/// callbacks
pub struct Context(apr::pool::PooledPtr<svn_client_ctx_t>);
unsafe impl Send for Context {}

impl Clone for Context {
    fn clone(&self) -> Self {
        let new = Context::new().unwrap();

        // TODO: Copy auth_baton
        // TODO: Copy notify func
        // TODO: copy log_msg_func
        // TODO: copy config
        // TODO: copy cancel_func
        // TODO: copy progress_func
        // TODO: copy wc_ctx
        // TODO: copy conflict_func
        // TODO: copy mimetypes map
        // TODO: copy check_tunnel_func
        // TODO: copy open_tunnel_func
        new
    }
}

impl Context {
    pub fn new() -> Result<Self, Error> {
        // call svn_client_create_context2
        Ok(Context(apr::pool::PooledPtr::initialize(|pool| {
            let mut ctx = std::ptr::null_mut();
            let ret = unsafe {
                svn_client_create_context2(&mut ctx, std::ptr::null_mut(), pool.as_mut_ptr())
            };
            Error::from_raw(ret)?;
            Ok::<_, Error>(ctx)
        })?))
    }

    pub(crate) unsafe fn as_mut_ptr(&mut self) -> *mut svn_client_ctx_t {
        self.0.as_mut_ptr()
    }

    pub fn set_auth<'a, 'b>(&'a mut self, auth_baton: &'b mut crate::auth::AuthBaton)
    where
        'b: 'a,
    {
        self.0.auth_baton = auth_baton.as_mut_ptr();
    }

    /// Checkout a working copy from url to path.
    pub fn checkout<'a>(
        &mut self,
        url: impl AsCanonicalUri<'a>,
        path: impl AsCanonicalDirent<'a>,
        options: &CheckoutOptions,
    ) -> Result<Revnum, Error> {
        let peg_revision = options.peg_revision.into();
        let revision = options.revision.into();
        let mut pool = Pool::default();
        unsafe {
            let mut revnum = 0;
            let url = url.as_canonical_uri(&mut pool);
            let path = path.as_canonical_dirent(&mut pool);
            let err = svn_client_checkout3(
                &mut revnum,
                url.as_ptr(),
                path.as_ptr(),
                &peg_revision,
                &revision,
                options.depth.into(),
                options.ignore_externals.into(),
                options.allow_unver_obstructions.into(),
                &mut *self.0,
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(Revnum::from_raw(revnum).unwrap())
        }
    }

    pub fn update(
        &mut self,
        paths: &[&str],
        revision: Revision,
        options: &UpdateOptions
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
                options.depth.into(),
                options.depth_is_sticky.into(),
                options.ignore_externals.into(),
                options.allow_unver_obstructions.into(),
                options.adds_as_modifications.into(),
                options.make_parents.into(),
                &mut *self.0,
                std::rc::Rc::get_mut(&mut pool).unwrap().as_mut_ptr(),
            );
            let result_revs: apr::tables::ArrayHeader<Revnum> =
                apr::tables::ArrayHeader::<Revnum>::from_raw_parts(&pool, result_revs);
            Error::from_raw(err)?;
            Ok(result_revs.iter().collect())
        }
    }

    pub fn switch<'a>(
        &mut self,
        path: impl AsCanonicalDirent<'a>,
        url: impl AsCanonicalUri<'a>,
        options: &SwitchOptions
    ) -> Result<Revnum, Error> {
        let mut pool = Pool::default();
        let mut result_rev = 0;
        let path = path.as_canonical_dirent(&mut pool);
        let url = url.as_canonical_uri(&mut pool);
        unsafe {
            let err = svn_client_switch3(
                &mut result_rev,
                path.as_ptr(),
                url.as_ptr(),
                &options.peg_revision.into(),
                &options.revision.into(),
                options.depth.into(),
                options.depth_is_sticky.into(),
                options.ignore_externals.into(),
                options.allow_unver_obstructions.into(),
                options.make_parents.into(),
                &mut *self.0,
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(Revnum::from_raw(result_rev).unwrap())
        }
    }

    pub fn add<'a>(
        &mut self,
        path: impl AsCanonicalDirent<'a>,
        options: &AddOptions
    ) -> Result<(), Error> {
        let mut pool = Pool::default();
        let path = path.as_canonical_dirent(&mut pool);
        unsafe {
            let err = svn_client_add5(
                path.as_ptr(),
                options.depth.into(),
                options.force.into(),
                options.no_ignore.into(),
                options.no_autoprops.into(),
                options.add_parents.into(),
                &mut *self.0,
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(())
        }
    }

    pub fn mkdir(
        &mut self,
        paths: &[&str],
        make_parents: bool,
        revprop_table: std::collections::HashMap<&str, &[u8]>,
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
                let path = std::ffi::CString::new(*path).unwrap();
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
            Error::from_raw(err)?;
            Ok(())
        }
    }

    pub fn delete(
        &mut self,
        paths: &[&str],
        revprop_table: std::collections::HashMap<& str, & str>,
        options: &DeleteOptions
    ) -> Result<(), Error> {
        let mut pool = std::rc::Rc::new(Pool::default());
        unsafe {
            let mut rps = apr::hash::Hash::in_pool(&pool);
            for (k, v) in revprop_table {
                rps.set(k, &v);
            }
            let mut ps = apr::tables::ArrayHeader::in_pool(&pool, paths.len());
            for path in paths {
                let path = std::ffi::CString::new(*path).unwrap();
                ps.push(path.as_ptr() as *mut std::ffi::c_void);
            }
            let commit_callback = Box::into_raw(Box::new(options.commit_callback));
            let err = svn_client_delete4(
                ps.as_ptr(),
                options.force.into(),
                options.keep_local.into(),
                rps.as_ptr(),
                Some(wrap_commit_callback2),
                commit_callback as *mut std::ffi::c_void,
                &mut *self.0,
                std::rc::Rc::get_mut(&mut pool).unwrap().as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(())
        }
    }

    pub fn proplist(
        &mut self,
        target: &str,
        peg_revision: Revision,
        revision: Revision,
        depth: Depth,
        changelists: Option<&[&str]>,
        get_target_inherited_props: bool,
        receiver: &mut dyn FnMut(
            &str,
            &std::collections::HashMap<String, Vec<u8>>,
            Option<&[crate::InheritedItem]>,
        ) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let mut pool = Pool::default();

        let changelists = changelists.map(|cl| {
            cl.iter()
                .map(|cl| std::ffi::CString::new(*cl).unwrap())
                .collect::<Vec<_>>()
        });
        let changelists = changelists.as_ref().map(|cl| {
            cl.iter()
                .map(|cl| cl.as_ptr() as *const i8)
                .collect::<apr::tables::ArrayHeader<_>>()
        });

        unsafe {
            let receiver = Box::into_raw(Box::new(receiver));
            let err = svn_client_proplist4(
                target.as_ptr() as *const i8,
                &peg_revision.into(),
                &revision.into(),
                depth.into(),
                changelists
                    .map(|cl| cl.as_ptr())
                    .unwrap_or(std::ptr::null()),
                get_target_inherited_props.into(),
                Some(wrap_proplist_receiver2),
                receiver as *mut std::ffi::c_void,
                &mut *self.0,
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(())
        }
    }

    pub fn import<'a>(
        &mut self,
        path: impl AsCanonicalDirent<'a>,
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

        let path = path.as_canonical_dirent(std::rc::Rc::get_mut(&mut pool).unwrap());
        unsafe {
            let filter_callback = Box::into_raw(Box::new(filter_callback));
            let commit_callback = Box::into_raw(Box::new(commit_callback));
            let err = svn_client_import5(
                path.as_ptr(),
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
            Error::from_raw(err)?;
            Ok(())
        }
    }

    pub fn export<'a>(
        &mut self,
        from_path_or_url: &str,
        to_path: impl AsCanonicalDirent<'a>,
        peg_revision: Revision,
        revision: Revision,
        overwrite: bool,
        ignore_externals: bool,
        ignore_keywords: bool,
        depth: Depth,
        native_eol: crate::NativeEOL,
    ) -> Result<Revnum, Error> {
        let mut pool = std::rc::Rc::new(Pool::default());
        let native_eol: Option<&str> = native_eol.into();
        let native_eol = native_eol.map(|s| std::ffi::CString::new(s).unwrap());
        let mut revnum = 0;
        let to_path = to_path.as_canonical_dirent(std::rc::Rc::get_mut(&mut pool).unwrap());
        unsafe {
            let err = svn_client_export5(
                &mut revnum,
                from_path_or_url.as_ptr() as *const i8,
                to_path.as_ptr(),
                &peg_revision.into(),
                &revision.into(),
                overwrite as i32,
                ignore_externals as i32,
                ignore_keywords as i32,
                depth as i32,
                native_eol.map(|s| s.as_ptr()).unwrap_or(std::ptr::null()),
                &mut *self.0,
                std::rc::Rc::get_mut(&mut pool).unwrap().as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(Revnum::from_raw(revnum).unwrap())
        }
    }

    pub fn commit(
        &mut self,
        targets: &[&str],
        options: &CommitOptions,
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
            if let Some(changelists) = options.changelists {
                for changelist in changelists {
                    let changelist = std::ffi::CString::new(*changelist).unwrap();
                    cl.push(changelist.as_ptr() as *mut std::ffi::c_void);
                }
            }
            let commit_callback = Box::into_raw(Box::new(commit_callback));
            let err = svn_client_commit6(
                ps.as_ptr(),
                options.depth.into(),
                options.keep_locks.into(),
                options.keep_changelists.into(),
                options.commit_as_operations.into(),
                options.include_file_externals.into(),
                options.include_dir_externals.into(),
                cl.as_ptr(),
                rps.as_ptr(),
                Some(wrap_commit_callback2),
                commit_callback as *mut std::ffi::c_void,
                &mut *self.0,
                std::rc::Rc::get_mut(&mut pool).unwrap().as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(())
        }
    }

    pub fn status(
        &mut self,
        path: &str,
        options: &StatusOptions,
        status_func: &dyn FnMut(&'_ str, &'_ Status) -> Result<(), Error>,
    ) -> Result<Revnum, Error> {
        let mut pool = std::rc::Rc::new(Pool::default());
        let mut cl = apr::tables::ArrayHeader::in_pool(&pool, 0);
        if let Some(changelists) = options.changelists {
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
                &options.revision.into(),
                options.depth.into(),
                options.get_all.into(),
                options.check_out_of_date.into(),
                options.check_working_copy.into(),
                options.no_ignore.into(),
                options.ignore_externals.into(),
                options.depth_as_sticky.into(),
                cl.as_ptr(),
                Some(wrap_status_func),
                status_func as *mut std::ffi::c_void,
                std::rc::Rc::get_mut(&mut pool).unwrap().as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(Revnum::from_raw(revnum).unwrap())
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
            Error::from_raw(err)?;
            Ok(())
        }
    }

    pub fn iter_logs(
        &mut self,
        targets: &[&str],
        peg_revision: Revision,
        revision_ranges: &[RevisionRange],
        limit: i32,
        discover_changed_paths: bool,
        strict_node_history: bool,
        include_merged_revisions: bool,
        revprops: &[&str],
    ) -> impl Iterator<Item = Result<LogEntry, Error>> {
        // Create a channel between the worker and this thread
        let (tx, rx) = std::sync::mpsc::channel();

        let targets = targets.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let revision_ranges = revision_ranges.to_vec();
        let revprops = revprops.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let mut client = self.clone();

        std::thread::spawn(move || {
            let r = client.log(
                targets
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .as_slice(),
                peg_revision,
                &revision_ranges,
                limit,
                discover_changed_paths,
                strict_node_history,
                include_merged_revisions,
                revprops
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .as_slice(),
                &mut |log_entry| {
                    tx.send(Ok(Some(log_entry.clone()))).unwrap();
                    Ok(())
                },
            );
            if let Err(e) = r {
                tx.send(Err(e)).unwrap();
            }
            tx.send(Ok(None)).unwrap();
        });

        // Return an iterator that reads from the channel
        rx.into_iter()
            .take_while(|x| x.is_ok() && x.as_ref().unwrap().is_some())
            .map(|x| x.transpose().unwrap())
    }

    pub fn args_to_target_array(
        &mut self,
        mut os: apr::getopt::Getopt,
        known_targets: &[&str],
        keep_last_origpath_on_truepath_collision: bool,
    ) -> Result<Vec<String>, crate::Error> {
        let mut pool = apr::pool::Pool::new();
        let known_targets = known_targets
            .iter()
            .map(|s| std::ffi::CString::new(*s).unwrap())
            .collect::<Vec<_>>();
        let mut targets = std::ptr::null_mut();
        let err = unsafe {
            crate::generated::svn_client_args_to_target_array2(
                &mut targets,
                os.as_mut_ptr(),
                known_targets
                    .into_iter()
                    .map(|s| s.as_ptr())
                    .collect::<apr::tables::ArrayHeader<*const i8>>()
                    .as_ptr(),
                &mut *self.0,
                keep_last_origpath_on_truepath_collision.into(),
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        let targets = unsafe {
            apr::tables::ArrayHeader::<*const i8>::from_raw_parts(&std::rc::Rc::new(pool), targets)
        };
        Ok(targets
            .iter()
            .map(|s| unsafe { std::ffi::CStr::from_ptr(*s as *const i8) })
            .map(|s| s.to_str().unwrap().to_owned())
            .collect::<Vec<_>>())
    }

    pub fn vacuum(
        &mut self,
        path: &str,
        remove_unversioned_items: bool,
        remove_ignored_items: bool,
        fix_recorded_timestamps: bool,
        vacuum_pristines: bool,
        include_externals: bool,
    ) -> Result<(), Error> {
        let mut pool = std::rc::Rc::new(Pool::default());
        let path = std::ffi::CString::new(path).unwrap();
        unsafe {
            let err = svn_client_vacuum(
                path.as_ptr(),
                remove_unversioned_items.into(),
                remove_ignored_items.into(),
                fix_recorded_timestamps.into(),
                vacuum_pristines.into(),
                include_externals.into(),
                &mut *self.0,
                std::rc::Rc::get_mut(&mut pool).unwrap().as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(())
        }
    }

    pub fn cleanup(
        &mut self,
        path: &str,
        break_locks: bool,
        fix_recorded_timestamps: bool,
        clear_dav_cache: bool,
        vacuum_pristines: bool,
        include_externals: bool,
    ) -> Result<(), Error> {
        let mut pool = std::rc::Rc::new(Pool::default());
        let path = std::ffi::CString::new(path).unwrap();
        unsafe {
            let err = svn_client_cleanup2(
                path.as_ptr(),
                break_locks.into(),
                fix_recorded_timestamps.into(),
                clear_dav_cache.into(),
                vacuum_pristines.into(),
                include_externals.into(),
                &mut *self.0,
                std::rc::Rc::get_mut(&mut pool).unwrap().as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(())
        }
    }

    pub fn relocate<'a>(
        &mut self,
        path: impl AsCanonicalDirent<'a>,
        from: &str,
        to: &str,
        ignore_externals: bool,
    ) -> Result<(), Error> {
        let mut pool = std::rc::Rc::new(Pool::default());
        let from = std::ffi::CString::new(from).unwrap();
        let to = std::ffi::CString::new(to).unwrap();
        let path = path.as_canonical_dirent(std::rc::Rc::get_mut(&mut pool).unwrap());
        unsafe {
            let err = svn_client_relocate2(
                path.as_ptr(),
                from.as_ptr(),
                to.as_ptr(),
                ignore_externals.into(),
                &mut *self.0,
                std::rc::Rc::get_mut(&mut pool).unwrap().as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(())
        }
    }

    pub fn conflict_get<'a>(
        &mut self,
        local_abspath: impl AsCanonicalDirent<'a>,
    ) -> Result<Conflict, Error> {
        Ok(Conflict(apr::pool::PooledPtr::initialize(|pool| {
            let local_abspath = local_abspath.as_canonical_dirent(pool);
            let mut conflict: *mut crate::generated::svn_client_conflict_t = std::ptr::null_mut();
            unsafe {
                let err = svn_client_conflict_get(
                    &mut conflict,
                    local_abspath.as_ptr(),
                    &mut *self.0,
                    pool.as_mut_ptr(),
                    Pool::new().as_mut_ptr(),
                );
                Error::from_raw(err)?;
                Ok::<_, Error>(conflict)
            }
        })?))
    }

    pub fn cat(
        &mut self,
        path_or_url: &str,
        stream: &mut dyn std::io::Write,
        peg_revision: Revision,
        revision: Revision,
        expand_keywords: bool,
    ) -> Result<HashMap<String, Vec<u8>>, Error> {
        let mut pool = std::rc::Rc::new(Pool::default());
        let path_or_url = std::ffi::CString::new(path_or_url).unwrap();
        let mut s = crate::io::wrap_write(stream)?;
        let mut props: *mut apr::hash::apr_hash_t = std::ptr::null_mut();
        unsafe {
            let err = crate::generated::svn_client_cat3(
                &mut props,
                s.as_mut_ptr(),
                path_or_url.as_ptr(),
                &peg_revision.into(),
                &revision.into(),
                expand_keywords.into(),
                self.as_mut_ptr(),
                std::rc::Rc::get_mut(&mut pool).unwrap().as_mut_ptr(),
                apr::pool::Pool::new().as_mut_ptr(),
            );
            Error::from_raw(err)?;
            let mut props =
                apr::hash::Hash::<&str, &[u8]>::from_raw(PooledPtr::in_pool(pool, props));
            Ok(props
                .iter()
                .map(|(k, v)| (String::from_utf8_lossy(k).to_string(), v.to_vec()))
                .collect())
        }
    }

    pub fn lock(&mut self, targets: &[&str], comment: &str, steal_lock: bool) -> Result<(), Error> {
        let mut pool = std::rc::Rc::new(Pool::default());
        let targets = targets
            .iter()
            .map(|s| std::ffi::CString::new(*s).unwrap())
            .collect::<Vec<_>>();
        let targets = targets
            .iter()
            .map(|s| s.as_ptr())
            .collect::<apr::tables::ArrayHeader<_>>();
        let comment = std::ffi::CString::new(comment).unwrap();
        unsafe {
            let err = crate::generated::svn_client_lock(
                targets.as_ptr(),
                comment.as_ptr(),
                steal_lock.into(),
                self.as_mut_ptr(),
                std::rc::Rc::get_mut(&mut pool).unwrap().as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(())
        }
    }

    pub fn unlock(&mut self, targets: &[&str], break_lock: bool) -> Result<(), Error> {
        let mut pool = std::rc::Rc::new(Pool::default());
        let targets = targets
            .iter()
            .map(|s| std::ffi::CString::new(*s).unwrap())
            .collect::<Vec<_>>();
        let targets = targets
            .iter()
            .map(|s| s.as_ptr())
            .collect::<apr::tables::ArrayHeader<_>>();
        unsafe {
            let err = crate::generated::svn_client_unlock(
                targets.as_ptr(),
                break_lock.into(),
                self.as_mut_ptr(),
                std::rc::Rc::get_mut(&mut pool).unwrap().as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(())
        }
    }

    pub fn get_wc_root<'a>(
        &mut self,
        path: impl AsCanonicalDirent<'a>,
    ) -> Result<std::path::PathBuf, Error> {
        let mut pool = std::rc::Rc::new(Pool::default());
        let path = path.as_canonical_dirent(std::rc::Rc::get_mut(&mut pool).unwrap());
        let mut wc_root: *const i8 = std::ptr::null();
        unsafe {
            let err = crate::generated::svn_client_get_wc_root(
                &mut wc_root,
                path.as_ptr(),
                self.as_mut_ptr(),
                std::rc::Rc::get_mut(&mut pool).unwrap().as_mut_ptr(),
                apr::pool::Pool::new().as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(std::ffi::CStr::from_ptr(wc_root).to_str().unwrap().into())
        }
    }

    pub fn min_max_revisions<'a>(
        &mut self,
        local_abspath: impl AsCanonicalDirent<'a>,
        committed: bool,
    ) -> Result<(Revnum, Revnum), Error> {
        let mut scratch_pool = apr::pool::Pool::new();
        let local_abspath = local_abspath.as_canonical_dirent(&mut scratch_pool);
        let mut min_revision: crate::generated::svn_revnum_t = 0;
        let mut max_revision: crate::generated::svn_revnum_t = 0;
        unsafe {
            let err = crate::generated::svn_client_min_max_revisions(
                &mut min_revision,
                &mut max_revision,
                local_abspath.as_ptr(),
                committed as i32,
                self.as_mut_ptr(),
                scratch_pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok((
                Revnum::from_raw(min_revision).unwrap(),
                Revnum::from_raw(max_revision).unwrap(),
            ))
        }
    }

    pub fn url_from_path<'a>(&mut self, path: impl AsCanonicalUri<'a>) -> Result<String, Error> {
        let mut pool = Pool::default();
        let path = path.as_canonical_uri(&mut pool);
        let mut url: *const i8 = std::ptr::null();
        unsafe {
            let err = crate::generated::svn_client_url_from_path2(
                &mut url,
                path.as_ptr(),
                self.as_mut_ptr(),
                pool.as_mut_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(std::ffi::CStr::from_ptr(url).to_str().unwrap().into())
        }
    }

    pub fn get_repos_root(&mut self, path_or_url: &str) -> Result<(String, String), Error> {
        let mut pool = Pool::default();
        let path_or_url = std::ffi::CString::new(path_or_url).unwrap();
        let mut repos_root: *const i8 = std::ptr::null();
        let mut repos_uuid: *const i8 = std::ptr::null();
        unsafe {
            let err = crate::generated::svn_client_get_repos_root(
                &mut repos_root,
                &mut repos_uuid,
                path_or_url.as_ptr(),
                self.as_mut_ptr(),
                pool.as_mut_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok((
                std::ffi::CStr::from_ptr(repos_root)
                    .to_str()
                    .unwrap()
                    .into(),
                std::ffi::CStr::from_ptr(repos_uuid)
                    .to_str()
                    .unwrap()
                    .into(),
            ))
        }
    }

    pub fn open_raw_session(
        &mut self,
        url: &str,
        wri_path: &std::path::Path,
    ) -> Result<crate::ra::Session, Error> {
        let url = std::ffi::CString::new(url).unwrap();
        let wri_path = std::ffi::CString::new(wri_path.to_str().unwrap()).unwrap();
        let session = PooledPtr::initialize(|pool| unsafe {
            let mut scratch_pool = Pool::default();
            let mut session: *mut crate::generated::svn_ra_session_t = std::ptr::null_mut();
            let err = crate::generated::svn_client_open_ra_session2(
                &mut session,
                url.as_ptr(),
                wri_path.as_ptr(),
                self.as_mut_ptr(),
                pool.as_mut_ptr(),
                scratch_pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok::<_, Error>(session)
        })?;
        Ok(crate::ra::Session::from_raw(session))
    }

    pub fn info(
        &mut self,
        abspath_or_url: &str,
        peg_revision: Revision,
        revision: Revision,
        depth: Depth,
        fetch_excluded: bool,
        fetch_actual_only: bool,
        include_externals: bool,
        changelists: Option<&[&str]>,
        receiver: &dyn FnMut(&Info) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let mut pool = Pool::default();
        let abspath_or_url = std::ffi::CString::new(abspath_or_url).unwrap();
        let changelists = changelists.map(|cl| {
            cl.iter()
                .map(|cl| std::ffi::CString::new(*cl).unwrap())
                .collect::<Vec<_>>()
        });
        let changelists = changelists.as_ref().map(|cl| {
            cl.iter()
                .map(|cl| cl.as_ptr())
                .collect::<apr::tables::ArrayHeader<_>>()
        });
        let mut receiver = receiver;
        let receiver = &mut receiver as *mut _ as *mut std::ffi::c_void;
        unsafe {
            let err = crate::generated::svn_client_info4(
                abspath_or_url.as_ptr(),
                &peg_revision.into(),
                &revision.into(),
                depth.into(),
                fetch_excluded as i32,
                fetch_actual_only as i32,
                include_externals as i32,
                changelists.map_or(std::ptr::null(), |cl| cl.as_ptr()),
                Some(wrap_info_receiver2),
                receiver,
                self.as_mut_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(())
        }
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

pub struct Status(pub(crate) *const crate::generated::svn_client_status_t);

impl Status {
    pub fn kind(&self) -> crate::NodeKind {
        unsafe { (*self.0).kind.into() }
    }

    pub fn local_abspath(&self) -> &str {
        unsafe {
            std::ffi::CStr::from_ptr((*self.0).local_abspath)
                .to_str()
                .unwrap()
        }
    }

    pub fn filesize(&self) -> i64 {
        unsafe { (*self.0).filesize }
    }

    pub fn versioned(&self) -> bool {
        unsafe { (*self.0).versioned != 0 }
    }

    pub fn conflicted(&self) -> bool {
        unsafe { (*self.0).conflicted != 0 }
    }

    pub fn node_status(&self) -> crate::StatusKind {
        unsafe { (*self.0).node_status.into() }
    }

    pub fn text_status(&self) -> crate::StatusKind {
        unsafe { (*self.0).text_status.into() }
    }

    pub fn prop_status(&self) -> crate::StatusKind {
        unsafe { (*self.0).prop_status.into() }
    }

    pub fn wc_is_locked(&self) -> bool {
        unsafe { (*self.0).wc_is_locked != 0 }
    }

    pub fn copied(&self) -> bool {
        unsafe { (*self.0).copied != 0 }
    }

    pub fn repos_root_url(&self) -> &str {
        unsafe {
            std::ffi::CStr::from_ptr((*self.0).repos_root_url)
                .to_str()
                .unwrap()
        }
    }

    pub fn repos_uuid(&self) -> &str {
        unsafe {
            std::ffi::CStr::from_ptr((*self.0).repos_uuid)
                .to_str()
                .unwrap()
        }
    }

    pub fn repos_relpath(&self) -> &str {
        unsafe {
            std::ffi::CStr::from_ptr((*self.0).repos_relpath)
                .to_str()
                .unwrap()
        }
    }

    pub fn revision(&self) -> Revnum {
        Revnum::from_raw(unsafe { (*self.0).revision }).unwrap()
    }

    pub fn changed_rev(&self) -> Revnum {
        Revnum::from_raw(unsafe { (*self.0).changed_rev }).unwrap()
    }

    pub fn changed_date(&self) -> apr::time::Time {
        unsafe { apr::time::Time::from((*self.0).changed_date) }
    }

    pub fn changed_author(&self) -> &str {
        unsafe {
            std::ffi::CStr::from_ptr((*self.0).changed_author)
                .to_str()
                .unwrap()
        }
    }

    pub fn switched(&self) -> bool {
        unsafe { (*self.0).switched != 0 }
    }

    pub fn file_external(&self) -> bool {
        unsafe { (*self.0).file_external != 0 }
    }

    pub fn lock(&self) -> Option<&crate::Lock> {
        todo!()
    }

    pub fn changelist(&self) -> Option<&str> {
        unsafe {
            if (*self.0).changelist.is_null() {
                None
            } else {
                Some(
                    std::ffi::CStr::from_ptr((*self.0).changelist)
                        .to_str()
                        .unwrap(),
                )
            }
        }
    }

    pub fn depth(&self) -> crate::Depth {
        unsafe { (*self.0).depth.into() }
    }

    pub fn ood_kind(&self) -> crate::NodeKind {
        unsafe { (*self.0).ood_kind.into() }
    }

    pub fn repos_node_status(&self) -> crate::StatusKind {
        unsafe { (*self.0).repos_node_status.into() }
    }

    pub fn repos_text_status(&self) -> crate::StatusKind {
        unsafe { (*self.0).repos_text_status.into() }
    }

    pub fn repos_prop_status(&self) -> crate::StatusKind {
        unsafe { (*self.0).repos_prop_status.into() }
    }

    pub fn repos_lock(&self) -> Option<crate::Lock> {
        todo!()
    }

    pub fn ood_changed_rev(&self) -> Option<Revnum> {
        Revnum::from_raw(unsafe { (*self.0).ood_changed_rev })
    }

    pub fn ood_changed_author(&self) -> Option<&str> {
        unsafe {
            if (*self.0).ood_changed_author.is_null() {
                None
            } else {
                Some(
                    std::ffi::CStr::from_ptr((*self.0).ood_changed_author)
                        .to_str()
                        .unwrap(),
                )
            }
        }
    }

    pub fn moved_from_abspath(&self) -> Option<&str> {
        unsafe {
            if (*self.0).moved_from_abspath.is_null() {
                None
            } else {
                Some(
                    std::ffi::CStr::from_ptr((*self.0).moved_from_abspath)
                        .to_str()
                        .unwrap(),
                )
            }
        }
    }

    pub fn moved_to_abspath(&self) -> Option<&str> {
        unsafe {
            if (*self.0).moved_to_abspath.is_null() {
                None
            } else {
                Some(
                    std::ffi::CStr::from_ptr((*self.0).moved_to_abspath)
                        .to_str()
                        .unwrap(),
                )
            }
        }
    }
}

pub struct Conflict(pub(crate) apr::pool::PooledPtr<crate::generated::svn_client_conflict_t>);

impl Conflict {
    pub fn prop_get_description(&mut self) -> Result<String, Error> {
        let mut pool = apr::pool::Pool::new();
        let mut description: *const i8 = std::ptr::null_mut();
        let err = unsafe {
            crate::generated::svn_client_conflict_prop_get_description(
                &mut description,
                self.0.as_mut_ptr(),
                pool.as_mut_ptr(),
                pool.as_mut_ptr(),
            )
        };

        Error::from_raw(err)?;
        Ok(unsafe { std::ffi::CStr::from_ptr(description) }
            .to_str()
            .unwrap()
            .to_owned())
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

    #[test]
    fn test_open() {
        let td = tempfile::tempdir().unwrap();
        let repo_path = td.path().join("repo");
        let mut repos = crate::repos::Repos::create(&repo_path).unwrap();
        assert_eq!(repos.path(), td.path().join("repo"));
        let mut ctx = Context::new().unwrap();
        let dirent = crate::dirent::Dirent::from(repo_path.to_str().unwrap());
        let url = dirent.canonicalize().to_file_url().unwrap();
        let revnum = ctx
            .checkout(
                url,
                &td.path().join("wc"),
                &CheckoutOptions {
                    peg_revision: Revision::Head,
                    revision: Revision::Head,
                    depth: Depth::Infinity,
                    ignore_externals: false,
                    allow_unver_obstructions: false,
                }
            )
            .unwrap();
        assert_eq!(revnum, Revnum(0));
    }
}
