use crate::dirent::AsCanonicalDirent;
use crate::io::Dirent;
use crate::uri::AsCanonicalUri;
use crate::{svn_result, with_tmp_pool, Depth, Error, LogEntry, Revision, RevisionRange, Revnum, Version};
use apr::Pool;

use std::collections::HashMap;
use subversion_sys::svn_error_t;
use subversion_sys::{
    svn_client_add5, svn_client_checkout3, svn_client_cleanup2, svn_client_commit6,
    svn_client_conflict_get, svn_client_create_context2, svn_client_ctx_t, svn_client_delete4,
    svn_client_export5, svn_client_import5, svn_client_log5, svn_client_mkdir4,
    svn_client_proplist4, svn_client_relocate2, svn_client_status6, svn_client_switch3,
    svn_client_update4, svn_client_vacuum, svn_client_version,
};

pub fn version() -> Version {
    unsafe { Version(svn_client_version()) }
}

extern "C" fn wrap_filter_callback(
    baton: *mut std::ffi::c_void,
    filtered: *mut subversion_sys::svn_boolean_t,
    local_abspath: *const i8,
    dirent: *const subversion_sys::svn_io_dirent2_t,
    _pool: *mut apr_sys::apr_pool_t,
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
            *filtered = ret as subversion_sys::svn_boolean_t;
            std::ptr::null_mut()
        } else {
            ret.unwrap_err().as_mut_ptr()
        }
    }
}

extern "C" fn wrap_status_func(
    baton: *mut std::ffi::c_void,
    path: *const i8,
    status: *const subversion_sys::svn_client_status_t,
    _pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
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

extern "C" fn wrap_proplist_receiver2(
    baton: *mut std::ffi::c_void,
    path: *const i8,
    props: *mut apr::hash::apr_hash_t,
    inherited_props: *mut apr::tables::apr_array_header_t,
    scratch_pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
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
        let mut props = apr::hash::Hash::<&str, *mut subversion_sys::svn_string_t>::from_ptr(props);
        let props = props
            .iter(&*scratch_pool)
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
                *mut subversion_sys::svn_prop_inherited_item_t,
            >::from_ptr(inherited_props);
            Some(
                inherited_props
                    .iter()
                    .map(|x| crate::InheritedItem::from_raw(x))
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

pub struct Info(*const subversion_sys::svn_client_info2_t);

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

    pub fn kind(&self) -> subversion_sys::svn_node_kind_t {
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
    info: *const subversion_sys::svn_client_info2_t,
    _scatch_pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
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

/// Options for cat
#[derive(Debug, Clone, Copy, Default)]
pub struct CatOptions {
    revision: Revision,
    peg_revision: Revision,
    expand_keywords: bool,
}

/// Options for cleanup
#[derive(Debug, Clone, Copy, Default)]
pub struct CleanupOptions {
    break_locks: bool,
    fix_recorded_timestamps: bool,
    clear_dav_cache: bool,
    vacuum_pristines: bool,
    include_externals: bool,
}

/// Options for proplist
#[derive(Debug, Clone, Copy, Default)]
pub struct ProplistOptions<'a> {
    peg_revision: Revision,
    revision: Revision,
    depth: Depth,
    changelists: Option<&'a [&'a str]>,
    get_target_inherited_props: bool,
}

/// Options for export
#[derive(Debug, Clone, Copy, Default)]
pub struct ExportOptions {
    peg_revision: Revision,
    revision: Revision,
    overwrite: bool,
    ignore_externals: bool,
    ignore_keywords: bool,
    depth: Depth,
    native_eol: crate::NativeEOL,
}

/// Options for vacuum
#[derive(Debug, Clone, Copy, Default)]
pub struct VacuumOptions {
    remove_unversioned_items: bool,
    remove_ignored_items: bool,
    fix_recorded_timestamps: bool,
    vacuum_pristines: bool,
    include_externals: bool,
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
pub struct Context {
    ptr: *mut svn_client_ctx_t,
    pool: apr::Pool,
    _phantom: std::marker::PhantomData<*mut ()>,
}
unsafe impl Send for Context {}

impl Drop for Context {
    fn drop(&mut self) {
        // Pool drop will clean up context
    }
}

impl Context {
    pub fn new() -> Result<Self, Error> {
        let pool = apr::Pool::new();
        let mut ctx = std::ptr::null_mut();
        let ret = unsafe {
            svn_client_create_context2(&mut ctx, std::ptr::null_mut(), pool.as_mut_ptr())
        };
        Error::from_raw(ret)?;
        Ok(Context {
            ptr: ctx,
            pool,
            _phantom: std::marker::PhantomData,
        })
    }

    pub(crate) unsafe fn as_mut_ptr(&mut self) -> *mut svn_client_ctx_t {
        self.ptr
    }

    pub fn set_auth<'a, 'b>(&'a mut self, auth_baton: &'b mut crate::auth::AuthBaton)
    where
        'b: 'a,
    {
        unsafe {
            (*self.ptr).auth_baton = auth_baton.as_mut_ptr();
        }
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
                self.ptr,
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
        options: &UpdateOptions,
    ) -> Result<Vec<Revnum>, Error> {
        let mut pool = std::rc::Rc::new(Pool::default());
        let mut result_revs = std::ptr::null_mut();
        unsafe {
            let mut ps = apr::tables::ArrayHeader::new_with_capacity(&pool, paths.len());
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
                self.ptr,
                std::rc::Rc::get_mut(&mut pool).unwrap().as_mut_ptr(),
            );
            let result_revs: apr::tables::ArrayHeader<Revnum> =
                apr::tables::ArrayHeader::<Revnum>::from_ptr(result_revs);
            Error::from_raw(err)?;
            Ok(result_revs.iter().collect())
        }
    }

    pub fn switch<'a>(
        &mut self,
        path: impl AsCanonicalDirent<'a>,
        url: impl AsCanonicalUri<'a>,
        options: &SwitchOptions,
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
                self.ptr,
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(Revnum::from_raw(result_rev).unwrap())
        }
    }

    pub fn add<'a>(
        &mut self,
        path: impl AsCanonicalDirent<'a>,
        options: &AddOptions,
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
                self.ptr,
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
            let mut rps = apr::hash::Hash::new(&pool);
            for (k, v) in revprop_table {
                rps.set(k, &v);
            }
            let mut ps = apr::tables::ArrayHeader::new_with_capacity(&pool, paths.len());
            for path in paths {
                let path = std::ffi::CString::new(*path).unwrap();
                ps.push(path.as_ptr() as *mut std::ffi::c_void);
            }
            let commit_callback = Box::into_raw(Box::new(commit_callback));
            let err = svn_client_mkdir4(
                ps.as_ptr(),
                make_parents.into(),
                rps.as_ptr(),
                Some(crate::wrap_commit_callback2),
                commit_callback as *mut std::ffi::c_void,
                self.ptr,
                std::rc::Rc::get_mut(&mut pool).unwrap().as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(())
        }
    }

    pub fn delete(
        &mut self,
        paths: &[&str],
        revprop_table: std::collections::HashMap<&str, &str>,
        options: &DeleteOptions,
    ) -> Result<(), Error> {
        let mut pool = std::rc::Rc::new(Pool::default());
        unsafe {
            let mut rps = apr::hash::Hash::new(&pool);
            for (k, v) in revprop_table {
                rps.set(k, &v);
            }
            let mut ps = apr::tables::ArrayHeader::new_with_capacity(&pool, paths.len());
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
                Some(crate::wrap_commit_callback2),
                commit_callback as *mut std::ffi::c_void,
                self.ptr,
                std::rc::Rc::get_mut(&mut pool).unwrap().as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(())
        }
    }

    pub fn proplist(
        &mut self,
        target: &str,
        options: &ProplistOptions,
        receiver: &mut dyn FnMut(
            &str,
            &std::collections::HashMap<String, Vec<u8>>,
            Option<&[crate::InheritedItem]>,
        ) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let pool = Pool::default();

        let changelists = options.changelists.map(|cl| {
            cl.iter()
                .map(|cl| std::ffi::CString::new(*cl).unwrap())
                .collect::<Vec<_>>()
        });
        let changelists = changelists.as_ref().map(|cl| {
            let mut array = apr::tables::ArrayHeader::<*const i8>::new(&pool);
            for item in cl.iter() {
                array.push(item.as_ptr() as *const i8);
            }
            array
        });

        unsafe {
            let receiver = Box::into_raw(Box::new(receiver));
            let err = svn_client_proplist4(
                target.as_ptr() as *const i8,
                &options.peg_revision.into(),
                &options.revision.into(),
                options.depth.into(),
                changelists
                    .map(|cl| cl.as_ptr())
                    .unwrap_or(std::ptr::null()),
                options.get_target_inherited_props.into(),
                Some(wrap_proplist_receiver2),
                receiver as *mut std::ffi::c_void,
                self.ptr,
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
        let path = path.as_canonical_dirent(std::rc::Rc::get_mut(&mut pool).unwrap());
        let mut rps = apr::hash::Hash::new(std::rc::Rc::get_mut(&mut pool).unwrap());
        for (k, v) in revprop_table {
            rps.set(k, &v);
        }
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
                Some(crate::wrap_commit_callback2),
                commit_callback as *mut std::ffi::c_void,
                self.ptr,
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
        options: &ExportOptions,
    ) -> Result<Revnum, Error> {
        let mut pool = std::rc::Rc::new(Pool::default());
        let native_eol: Option<&str> = options.native_eol.into();
        let native_eol = native_eol.map(|s| std::ffi::CString::new(s).unwrap());
        let mut revnum = 0;
        let to_path = to_path.as_canonical_dirent(std::rc::Rc::get_mut(&mut pool).unwrap());
        unsafe {
            let err = svn_client_export5(
                &mut revnum,
                from_path_or_url.as_ptr() as *const i8,
                to_path.as_ptr(),
                &options.peg_revision.into(),
                &options.revision.into(),
                options.overwrite as i32,
                options.ignore_externals as i32,
                options.ignore_keywords as i32,
                options.depth as i32,
                native_eol.map(|s| s.as_ptr()).unwrap_or(std::ptr::null()),
                self.ptr,
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
        let mut rps = apr::hash::Hash::new(&pool);
        for (k, v) in revprop_table {
            rps.set(k, &v);
        }

        unsafe {
            let mut ps = apr::tables::ArrayHeader::new_with_capacity(&pool, targets.len());
            for target in targets {
                let target = std::ffi::CString::new(*target).unwrap();
                ps.push(target.as_ptr() as *mut std::ffi::c_void);
            }
            let mut cl = apr::tables::ArrayHeader::new_with_capacity(&pool, 0);
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
                Some(crate::wrap_commit_callback2),
                commit_callback as *mut std::ffi::c_void,
                self.ptr,
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
        let mut cl = apr::tables::ArrayHeader::new_with_capacity(&pool, 0);
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
                self.ptr,
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
            let pool_mut = std::rc::Rc::get_mut(&mut pool).unwrap();
            let mut ps = apr::tables::ArrayHeader::new_with_capacity(pool_mut, targets.len());
            for target in targets {
                let target = std::ffi::CString::new(*target).unwrap();
                ps.push(target.as_ptr() as *mut std::ffi::c_void);
            }
            let mut rrs =
                apr::tables::ArrayHeader::<*mut subversion_sys::svn_opt_revision_range_t>::new_with_capacity(
                    pool_mut,
                    revision_ranges.len(),
                );
            for revision_range in revision_ranges {
                rrs.push(revision_range.to_c(pool_mut));
            }
            let mut rps = apr::tables::ArrayHeader::new_with_capacity(pool_mut, revprops.len());
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
                Some(crate::wrap_log_entry_receiver),
                log_entry_receiver as *mut std::ffi::c_void,
                self.ptr,
                std::rc::Rc::get_mut(&mut pool).unwrap().as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(())
        }
    }

    // TODO: This method needs to be reworked to handle lifetime-bound Context
    // Cannot clone Context anymore due to lifetime bounds
    /*pub fn iter_logs(
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
    }*/

    pub fn args_to_target_array(
        &mut self,
        mut os: apr::getopt::Getopt,
        known_targets: &[&str],
        keep_last_origpath_on_truepath_collision: bool,
    ) -> Result<Vec<String>, crate::Error> {
        let pool = apr::pool::Pool::new();
        let known_targets = known_targets
            .iter()
            .map(|s| std::ffi::CString::new(*s).unwrap())
            .collect::<Vec<_>>();
        let mut targets = std::ptr::null_mut();
        let err = unsafe {
            subversion_sys::svn_client_args_to_target_array2(
                &mut targets,
                os.as_mut_ptr(),
                {
                    let mut array = apr::tables::ArrayHeader::<*const i8>::new(&pool);
                    for s in known_targets {
                        array.push(s.as_ptr());
                    }
                    array.as_ptr()
                },
                self.ptr,
                keep_last_origpath_on_truepath_collision.into(),
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        let targets = apr::tables::ArrayHeader::<*const i8>::from_ptr(targets);
        Ok(targets
            .iter()
            .map(|s| unsafe { std::ffi::CStr::from_ptr(*s as *const i8) })
            .map(|s| s.to_str().unwrap().to_owned())
            .collect::<Vec<_>>())
    }

    pub fn vacuum(&mut self, path: &str, options: &VacuumOptions) -> Result<(), Error> {
        let mut pool = std::rc::Rc::new(Pool::default());
        let path = std::ffi::CString::new(path).unwrap();
        unsafe {
            let err = svn_client_vacuum(
                path.as_ptr(),
                options.remove_unversioned_items.into(),
                options.remove_ignored_items.into(),
                options.fix_recorded_timestamps.into(),
                options.vacuum_pristines.into(),
                options.include_externals.into(),
                self.ptr,
                std::rc::Rc::get_mut(&mut pool).unwrap().as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(())
        }
    }

    pub fn cleanup(&mut self, path: &str, options: &CleanupOptions) -> Result<(), Error> {
        let mut pool = std::rc::Rc::new(Pool::default());
        let path = std::ffi::CString::new(path).unwrap();
        unsafe {
            let err = svn_client_cleanup2(
                path.as_ptr(),
                options.break_locks.into(),
                options.fix_recorded_timestamps.into(),
                options.clear_dav_cache.into(),
                options.vacuum_pristines.into(),
                options.include_externals.into(),
                self.ptr,
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
                self.ptr,
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
        let pool = apr::Pool::new();
        let mut scratch_pool = apr::Pool::new();
        let local_abspath = local_abspath.as_canonical_dirent(&mut scratch_pool);
        let mut conflict: *mut subversion_sys::svn_client_conflict_t = std::ptr::null_mut();
        unsafe {
            let err = svn_client_conflict_get(
                &mut conflict,
                local_abspath.as_ptr(),
                self.ptr,
                pool.as_mut_ptr(),
                Pool::new().as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(Conflict::from_ptr_and_pool(conflict, pool))
        }
    }

    pub fn cat(
        &mut self,
        path_or_url: &str,
        stream: &mut dyn std::io::Write,
        options: &CatOptions,
    ) -> Result<HashMap<String, Vec<u8>>, Error> {
        let mut pool = std::rc::Rc::new(Pool::default());
        let path_or_url = std::ffi::CString::new(path_or_url).unwrap();
        let mut s = crate::io::wrap_write(stream)?;
        let mut props: *mut apr::hash::apr_hash_t = std::ptr::null_mut();
        unsafe {
            let err = subversion_sys::svn_client_cat3(
                &mut props,
                s.as_mut_ptr(),
                path_or_url.as_ptr(),
                &options.peg_revision.into(),
                &options.revision.into(),
                options.expand_keywords.into(),
                self.as_mut_ptr(),
                std::rc::Rc::get_mut(&mut pool).unwrap().as_mut_ptr(),
                apr::pool::Pool::new().as_mut_ptr(),
            );
            Error::from_raw(err)?;
            let mut props = apr::hash::Hash::<&str, &[u8]>::from_ptr(props);
            Ok(props
                .iter(&*pool)
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
        let mut targets_array = apr::tables::ArrayHeader::<*const i8>::new(&*pool);
        for target in targets.iter() {
            targets_array.push(target.as_ptr());
        }
        let comment = std::ffi::CString::new(comment).unwrap();
        unsafe {
            let err = subversion_sys::svn_client_lock(
                targets_array.as_ptr(),
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
        let mut targets_array = apr::tables::ArrayHeader::<*const i8>::new(&*pool);
        for target in targets.iter() {
            targets_array.push(target.as_ptr());
        }
        unsafe {
            let err = subversion_sys::svn_client_unlock(
                targets_array.as_ptr(),
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
            let err = subversion_sys::svn_client_get_wc_root(
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
        let mut min_revision: subversion_sys::svn_revnum_t = 0;
        let mut max_revision: subversion_sys::svn_revnum_t = 0;
        unsafe {
            let err = subversion_sys::svn_client_min_max_revisions(
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
            let err = subversion_sys::svn_client_url_from_path2(
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
        let pool = Pool::default();
        let path_or_url = std::ffi::CString::new(path_or_url).unwrap();
        let mut repos_root: *const i8 = std::ptr::null();
        let mut repos_uuid: *const i8 = std::ptr::null();
        unsafe {
            let err = subversion_sys::svn_client_get_repos_root(
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

    #[cfg(feature = "ra")]
    pub fn open_raw_session(
        &mut self,
        url: &str,
        wri_path: &std::path::Path,
    ) -> Result<crate::ra::Session, Error> {
        let url = std::ffi::CString::new(url).unwrap();
        let wri_path = std::ffi::CString::new(wri_path.to_str().unwrap()).unwrap();
        let pool = apr::Pool::new();
        unsafe {
            let scratch_pool = Pool::default();
            let mut session: *mut subversion_sys::svn_ra_session_t = std::ptr::null_mut();
            let err = subversion_sys::svn_client_open_ra_session2(
                &mut session,
                url.as_ptr(),
                wri_path.as_ptr(),
                self.as_mut_ptr(),
                pool.as_mut_ptr(),
                scratch_pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(crate::ra::Session::from_ptr_and_pool(session, pool))
        }
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
        let pool = Pool::default();
        let abspath_or_url = std::ffi::CString::new(abspath_or_url).unwrap();
        let changelists = changelists.map(|cl| {
            cl.iter()
                .map(|cl| std::ffi::CString::new(*cl).unwrap())
                .collect::<Vec<_>>()
        });
        let changelists = changelists.as_ref().map(|cl| {
            let mut array = apr::tables::ArrayHeader::<*const i8>::new(&pool);
            for item in cl.iter() {
                array.push(item.as_ptr());
            }
            array
        });
        let mut receiver = receiver;
        let receiver = &mut receiver as *mut _ as *mut std::ffi::c_void;
        unsafe {
            let err = subversion_sys::svn_client_info4(
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

    /// Copy or move a file or directory
    pub fn copy(
        &mut self,
        sources: &[(&str, Option<Revision>)],
        dst_path: &str,
        copy_as_child: bool,
        make_parents: bool,
        ignore_externals: bool,
        metadata_only: bool,
        pin_externals: bool,
        // TODO: Add proper externals_to_pin support
    ) -> Result<(), Error> {
        with_tmp_pool(|pool| {
            let sources_array = sources
                .iter()
                .map(|(path, rev)| {
                    let path_c = std::ffi::CString::new(*path).unwrap();
                    let mut src: *mut subversion_sys::svn_client_copy_source_t = pool.calloc();
                    unsafe {
                        (*src).path = path_c.as_ptr();
                        (*src).revision = Box::into_raw(Box::new(rev.as_ref().map(|r| (*r).into()).unwrap_or(Revision::Head.into())));
                        (*src).peg_revision = Box::into_raw(Box::new(Revision::Head.into()));
                    }
                    src
                })
                .collect::<Vec<_>>();

            let mut sources_apr_array = apr::tables::ArrayHeader::<*const subversion_sys::svn_client_copy_source_t>::new(pool);
            for src in sources_array.iter() {
                sources_apr_array.push(*src as *const _);
            }

            let dst_c = std::ffi::CString::new(dst_path).unwrap();
            
            let err = unsafe {
                subversion_sys::svn_client_copy7(
                    sources_apr_array.as_ptr(),
                    dst_c.as_ptr(),
                    copy_as_child as i32,
                    make_parents as i32,
                    ignore_externals as i32,
                    metadata_only as i32,
                    pin_externals as i32,
                    std::ptr::null_mut(), // externals_to_pin - TODO
                    std::ptr::null_mut(), // revprop_table
                    None, // commit_callback
                    std::ptr::null_mut(), // commit_baton
                    self.as_mut_ptr(),
                    pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })
    }

    /// Create directories (supports multiple paths)
    pub fn mkdir_multiple(&mut self, paths: &[&str], make_parents: bool) -> Result<(), Error> {
        with_tmp_pool(|pool| {
            let paths_c: Vec<_> = paths.iter().map(|p| std::ffi::CString::new(*p).unwrap()).collect();
            let mut paths_array = apr::tables::ArrayHeader::<*const i8>::new(pool);
            for path in paths_c.iter() {
                paths_array.push(path.as_ptr());
            }

            let err = unsafe {
                subversion_sys::svn_client_mkdir4(
                    paths_array.as_ptr(),
                    make_parents as i32,
                    std::ptr::null_mut(), // revprop_table
                    None, // commit_callback  
                    std::ptr::null_mut(), // commit_baton
                    self.as_mut_ptr(),
                    pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })
    }

    /// Get property value
    pub fn propget(
        &mut self,
        propname: &str,
        target: &str,
        peg_revision: &Revision,
        revision: &Revision,
        actual_revnum: Option<&mut Revnum>,
        depth: Depth,
        changelists: Option<&[&str]>,
    ) -> Result<std::collections::HashMap<String, Vec<u8>>, Error> {
        with_tmp_pool(|pool| {
            let propname_c = std::ffi::CString::new(propname).unwrap();
            let target_c = std::ffi::CString::new(target).unwrap();

            let changelists = changelists.map(|cl| {
                let mut array = apr::tables::ArrayHeader::<*const i8>::new(pool);
                for item in cl.iter() {
                    let item_c = std::ffi::CString::new(*item).unwrap();
                    array.push(item_c.as_ptr());
                }
                array
            });

            let mut props = std::ptr::null_mut();
            let mut actual_rev = actual_revnum.as_ref().map_or(0, |r| r.0);

            let err = unsafe {
                subversion_sys::svn_client_propget5(
                    &mut props,
                    std::ptr::null_mut(), // inherited_props
                    propname_c.as_ptr(),
                    target_c.as_ptr(),
                    &(*peg_revision).into(),
                    &(*revision).into(),
                    if actual_revnum.is_some() { &mut actual_rev } else { std::ptr::null_mut() },
                    depth.into(),
                    changelists.map_or(std::ptr::null(), |cl| cl.as_ptr()),
                    self.as_mut_ptr(),
                    pool.as_mut_ptr(),
                    pool.as_mut_ptr(),
                )
            };
            svn_result(err)?;

            if let Some(revnum) = actual_revnum {
                *revnum = Revnum(actual_rev);
            }

            // Convert apr hash to Rust HashMap
            let hash = unsafe { apr::hash::Hash::<&[u8], *const subversion_sys::svn_string_t>::from_ptr(props) };
            let mut result = std::collections::HashMap::new();
            for (k, v) in hash.iter(pool) {
                let key = String::from_utf8_lossy(k).into_owned();
                let value = unsafe {
                    std::slice::from_raw_parts((**v).data as *const u8, (**v).len).to_vec()
                };
                result.insert(key, value);
            }
            Ok(result)
        })
    }

    /// Set property value
    pub fn propset(
        &mut self,
        propname: &str,
        propval: Option<&[u8]>,
        target: &str,
        depth: Depth,
        skip_checks: bool,
        base_revision_for_url: Revnum,
        changelists: Option<&[&str]>,
    ) -> Result<(), Error> {
        with_tmp_pool(|pool| {
            let propname_c = std::ffi::CString::new(propname).unwrap();
            let target_c = std::ffi::CString::new(target).unwrap();
            
            // propset_local expects an array of targets
            let mut targets_array = apr::tables::ArrayHeader::<*const i8>::new(pool);
            targets_array.push(target_c.as_ptr());

            let propval_svn = propval.map(|val| subversion_sys::svn_string_t {
                data: val.as_ptr() as *mut i8,
                len: val.len(),
            });

            let changelists = changelists.map(|cl| {
                let mut array = apr::tables::ArrayHeader::<*const i8>::new(pool);
                for item in cl.iter() {
                    let item_c = std::ffi::CString::new(*item).unwrap();
                    array.push(item_c.as_ptr());
                }
                array
            });

            let err = unsafe {
                subversion_sys::svn_client_propset_local(
                    propname_c.as_ptr(),
                    propval_svn.as_ref().map_or(std::ptr::null(), |v| v as *const _),
                    targets_array.as_ptr(),
                    depth.into(),
                    skip_checks as i32,
                    changelists.map_or(std::ptr::null(), |cl| cl.as_ptr()),
                    self.as_mut_ptr(),
                    pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })
    }

    /// List properties with callback
    pub fn proplist_all(
        &mut self,
        target: &str,
        peg_revision: &Revision,
        revision: &Revision,
        depth: Depth,
        changelists: Option<&[&str]>,
        get_target_inherited_props: bool,
        receiver: &mut dyn FnMut(&str, std::collections::HashMap<String, Vec<u8>>) -> Result<(), Error>,
    ) -> Result<(), Error> {
        with_tmp_pool(|pool| {
            let target_c = std::ffi::CString::new(target).unwrap();

            let changelists = changelists.map(|cl| {
                let mut array = apr::tables::ArrayHeader::<*const i8>::new(pool);
                for item in cl.iter() {
                    let item_c = std::ffi::CString::new(*item).unwrap();
                    array.push(item_c.as_ptr());
                }
                array
            });

            extern "C" fn proplist_receiver(
                baton: *mut std::ffi::c_void,
                path: *const i8,
                props: *mut apr_sys::apr_hash_t,
                _inherited_props: *mut apr_sys::apr_array_header_t,
                _scratch_pool: *mut apr_sys::apr_pool_t,
            ) -> *mut subversion_sys::svn_error_t {
                let receiver = unsafe { &mut *(baton as *mut &mut dyn FnMut(&str, std::collections::HashMap<String, Vec<u8>>) -> Result<(), Error>) };
                let path_str = unsafe { std::ffi::CStr::from_ptr(path).to_str().unwrap() };
                
                // Convert props hash
                let hash = unsafe { apr::hash::Hash::<&[u8], *const subversion_sys::svn_string_t>::from_ptr(props) };
                let mut prop_hash = std::collections::HashMap::new();
                let pool = apr::Pool::new();
                for (k, v) in hash.iter(&pool) {
                    let key = String::from_utf8_lossy(k).into_owned();
                    let value = unsafe {
                        std::slice::from_raw_parts((**v).data as *const u8, (**v).len).to_vec()
                    };
                    prop_hash.insert(key, value);
                }
                
                match receiver(path_str, prop_hash) {
                    Ok(()) => std::ptr::null_mut(),
                    Err(mut e) => unsafe { e.detach() },
                }
            }

            let receiver_ptr = receiver as *mut _ as *mut std::ffi::c_void;

            let err = unsafe {
                subversion_sys::svn_client_proplist4(
                    target_c.as_ptr(),
                    &(*peg_revision).into(),
                    &(*revision).into(),
                    depth.into(),
                    changelists.map_or(std::ptr::null(), |cl| cl.as_ptr()),
                    get_target_inherited_props as i32,
                    Some(proplist_receiver),
                    receiver_ptr,
                    self.as_mut_ptr(),
                    pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })
    }

    /// Get differences between two paths/revisions
    pub fn diff(
        &mut self,
        diff_options: &[&str],
        path_or_url1: &str,
        revision1: &Revision,
        path_or_url2: &str, 
        revision2: &Revision,
        relative_to_dir: Option<&str>,
        depth: Depth,
        ignore_ancestry: bool,
        no_diff_added: bool,
        no_diff_deleted: bool,
        show_copies_as_adds: bool,
        ignore_content_type: bool,
        ignore_properties: bool,
        properties_only: bool,
        use_git_diff_format: bool,
        header_encoding: &str,
        outstream: &mut crate::io::Stream,
        errstream: &mut crate::io::Stream,
        changelists: Option<&[&str]>,
    ) -> Result<(), Error> {
        with_tmp_pool(|pool| {
            let path1_c = std::ffi::CString::new(path_or_url1).unwrap();
            let path2_c = std::ffi::CString::new(path_or_url2).unwrap();
            let header_encoding_c = std::ffi::CString::new(header_encoding).unwrap();
            
            let diff_options_c: Vec<_> = diff_options.iter().map(|o| std::ffi::CString::new(*o).unwrap()).collect();
            let mut diff_options_array = apr::tables::ArrayHeader::<*const i8>::new(pool);
            for opt in diff_options_c.iter() {
                diff_options_array.push(opt.as_ptr());
            }

            let relative_to_dir_c = relative_to_dir.map(|d| std::ffi::CString::new(d).unwrap());

            let changelists = changelists.map(|cl| {
                let mut array = apr::tables::ArrayHeader::<*const i8>::new(pool);
                for item in cl.iter() {
                    let item_c = std::ffi::CString::new(*item).unwrap();
                    array.push(item_c.as_ptr());
                }
                array
            });

            let err = unsafe {
                subversion_sys::svn_client_diff6(
                    diff_options_array.as_ptr(),
                    path1_c.as_ptr(),
                    &revision1.into(),
                    path2_c.as_ptr(),
                    &revision2.into(),
                    relative_to_dir_c.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()),
                    depth.into(),
                    ignore_ancestry as i32,
                    no_diff_added as i32,
                    no_diff_deleted as i32,
                    show_copies_as_adds as i32,
                    ignore_content_type as i32,
                    ignore_properties as i32,
                    properties_only as i32,
                    use_git_diff_format as i32,
                    header_encoding_c.as_ptr(),
                    outstream.as_mut_ptr(),
                    errstream.as_mut_ptr(),
                    changelists.map_or(std::ptr::null(), |cl| cl.as_ptr()),
                    self.as_mut_ptr(),
                    pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })
    }

    /// List directory contents
    pub fn list(
        &mut self,
        path_or_url: &str,
        peg_revision: &Revision,
        revision: &Revision,
        patterns: Option<&[&str]>,
        depth: Depth,
        dirent_fields: u32,
        fetch_locks: bool,
        include_externals: bool,
        list_func: &mut dyn FnMut(&str, &crate::ra::Dirent, Option<&crate::Lock>) -> Result<(), Error>,
    ) -> Result<(), Error> {
        with_tmp_pool(|pool| {
            let path_or_url_c = std::ffi::CString::new(path_or_url).unwrap();

            let patterns = patterns.map(|p| {
                let mut array = apr::tables::ArrayHeader::<*const i8>::new(pool);
                for pattern in p.iter() {
                    let pattern_c = std::ffi::CString::new(*pattern).unwrap();
                    array.push(pattern_c.as_ptr());
                }
                array
            });

            extern "C" fn list_receiver(
                baton: *mut std::ffi::c_void,
                path: *const i8,
                dirent: *const subversion_sys::svn_dirent_t,
                lock: *const subversion_sys::svn_lock_t,
                _abs_path: *const i8,
                _external_parent_url: *const i8,
                _external_target: *const i8,
                _scratch_pool: *mut apr_sys::apr_pool_t,
            ) -> *mut subversion_sys::svn_error_t {
                let list_func = unsafe { &mut *(baton as *mut &mut dyn FnMut(&str, &crate::ra::Dirent, Option<&crate::Lock>) -> Result<(), Error>) };
                let path_str = unsafe { std::ffi::CStr::from_ptr(path).to_str().unwrap() };
                let dirent = unsafe { crate::ra::Dirent::from_raw(dirent as *mut _) };
                let lock = if lock.is_null() {
                    None
                } else {
                    Some(unsafe { crate::Lock::from_raw(lock as *mut _) })
                };
                
                match list_func(path_str, &dirent, lock.as_ref()) {
                    Ok(()) => std::ptr::null_mut(),
                    Err(mut e) => unsafe { e.detach() },
                }
            }

            let list_func_ptr = list_func as *mut _ as *mut std::ffi::c_void;

            let err = unsafe {
                subversion_sys::svn_client_list4(
                    path_or_url_c.as_ptr(),
                    &(*peg_revision).into(),
                    &(*revision).into(),
                    patterns.map_or(std::ptr::null(), |p| p.as_ptr()),
                    depth.into(),
                    dirent_fields,
                    fetch_locks as i32,
                    include_externals as i32,
                    Some(list_receiver),
                    list_func_ptr,
                    self.as_mut_ptr(),
                    pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })
    }

    /// Resolve conflicts
    pub fn resolve(&mut self, path: &str, depth: Depth, conflict_choice: crate::ConflictChoice) -> Result<(), Error> {
        with_tmp_pool(|pool| {
            let path_c = std::ffi::CString::new(path).unwrap();
            
            let err = unsafe {
                subversion_sys::svn_client_resolve(
                    path_c.as_ptr(),
                    depth.into(),
                    conflict_choice.into(),
                    self.as_mut_ptr(),
                    pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })
    }
}

// Note: Context now requires a pool parameter, so no Default impl

pub struct Status(pub(crate) *const subversion_sys::svn_client_status_t);

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

/// Conflict handle with RAII cleanup
pub struct Conflict {
    ptr: *mut subversion_sys::svn_client_conflict_t,
    pool: apr::Pool,
    _phantom: std::marker::PhantomData<*mut ()>, // !Send + !Sync
}

impl Drop for Conflict {
    fn drop(&mut self) {
        // Pool drop will clean up conflict
    }
}

impl Conflict {
    pub(crate) unsafe fn from_ptr_and_pool(
        ptr: *mut subversion_sys::svn_client_conflict_t,
        pool: apr::Pool,
    ) -> Self {
        Self {
            ptr,
            pool,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn prop_get_description(&mut self) -> Result<String, Error> {
        let pool = apr::pool::Pool::new();
        let mut description: *const i8 = std::ptr::null_mut();
        let err = unsafe {
            subversion_sys::svn_client_conflict_prop_get_description(
                &mut description,
                self.ptr,
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
        // TODO: Fix when to_file_url is reimplemented
        // let repo_path = std::ffi::CString::new(repo_path.to_str().unwrap()).unwrap();
        // let dirent = crate::dirent::Dirent::from(repo_path.as_c_str());
        // let url = dirent.canonicalize().to_file_url().unwrap();
        let url_str = format!("file://{}", repo_path.to_str().unwrap());
        let pool = apr::Pool::new();
        let url = crate::uri::Uri::from_str(&url_str, &pool);
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
                },
            )
            .unwrap();
        assert_eq!(revnum, Revnum(0));
    }
}
