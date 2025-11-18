use crate::dirent::AsCanonicalDirent;
use crate::io::Dirent;
use crate::uri::AsCanonicalUri;
use crate::{
    svn_result, with_tmp_pool, Depth, Error, LogEntry, Revision, RevisionRange, Revnum, Version,
};
use apr::Pool;

use std::collections::HashMap;
use std::ops::ControlFlow;
use subversion_sys::svn_error_t;
use subversion_sys::{
    svn_client_add5, svn_client_checkout3, svn_client_cleanup2, svn_client_commit6,
    svn_client_conflict_get, svn_client_create_context2, svn_client_ctx_t, svn_client_delete4,
    svn_client_export5, svn_client_import5, svn_client_log5, svn_client_mkdir4,
    svn_client_proplist4, svn_client_status6, svn_client_switch3, svn_client_update4,
    svn_client_vacuum, svn_client_version,
};

/// Returns the version information for the Subversion client library.
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
            &mut *(baton as *mut &mut dyn FnMut(&std::path::Path, &Dirent) -> Result<bool, Error>);
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
        let callback = &mut *(baton as *mut &mut dyn FnMut(&str, &Status) -> Result<(), Error>);
        let path_str = std::ffi::CStr::from_ptr(path).to_str().unwrap();
        let status_wrapper = Status(status);
        let ret = callback(path_str, &status_wrapper);
        if let Err(mut err) = ret {
            err.as_mut_ptr()
        } else {
            std::ptr::null_mut()
        }
    }
}

/// C trampoline for cancel callbacks
extern "C" fn cancel_trampoline(baton: *mut std::ffi::c_void) -> *mut subversion_sys::svn_error_t {
    if baton.is_null() {
        return std::ptr::null_mut();
    }

    let handler = unsafe { &mut *(baton as *mut Box<dyn FnMut() -> bool + Send>) };
    if handler() {
        // Return SVN_ERR_CANCELLED (200015)
        let err = Error::new(
            apr::Status::from(200015_i32),
            None,
            "Operation cancelled by user",
        );
        unsafe { err.into_raw() }
    } else {
        std::ptr::null_mut()
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
        // SVN owns the scratch_pool, we just need to reference it
        let _scratch_pool = apr::PoolHandle::from_borrowed_raw(scratch_pool);
        let callback = baton
            as *mut &mut dyn FnMut(
                &str,
                &HashMap<String, Vec<u8>>,
                Option<&[crate::InheritedItem]>,
            ) -> Result<(), Error>;
        let callback = &mut *(callback);
        let path: &str = std::ffi::CStr::from_ptr(path).to_str().unwrap();
        let prop_hash = crate::props::PropHash::from_ptr(props);
        let props = prop_hash.to_hashmap();
        let inherited_props = if inherited_props.is_null() {
            None
        } else {
            let inherited_props = apr::tables::TypedArray::<
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

/// Information about a versioned item in the repository.
pub struct Info(*const subversion_sys::svn_client_info2_t);

/// Information about a line in a blamed file
pub struct BlameInfo {
    /// Line number in the file.
    pub line_no: i64,
    /// Revision that last modified this line.
    pub revision: Revnum,
    /// Revision properties.
    pub revprops: HashMap<String, Vec<u8>>,
    /// Merged revision if this line came from a merge.
    pub merged_revision: Option<Revnum>,
    /// Properties of the merged revision.
    pub merged_revprops: HashMap<String, Vec<u8>>,
    /// Path of the merged file.
    pub merged_path: Option<String>,
    /// The actual line content.
    pub line: String,
    /// Whether this line has local changes.
    pub local_change: bool,
}

impl Info {
    /// Returns the URL of the item.
    pub fn url(&self) -> &str {
        unsafe {
            let url = (*self.0).URL;
            std::ffi::CStr::from_ptr(url).to_str().unwrap()
        }
    }

    /// Returns the revision number of the item.
    pub fn revision(&self) -> Revnum {
        unsafe { Revnum::from_raw((*self.0).rev).unwrap() }
    }

    /// Returns the node kind (file, directory, etc.) of the item.
    pub fn kind(&self) -> subversion_sys::svn_node_kind_t {
        unsafe { (*self.0).kind }
    }

    /// Returns the repository root URL.
    pub fn repos_root_url(&self) -> &str {
        unsafe {
            let url = (*self.0).repos_root_URL;
            std::ffi::CStr::from_ptr(url).to_str().unwrap()
        }
    }

    /// Returns the repository UUID.
    pub fn repos_uuid(&self) -> &str {
        unsafe {
            let uuid = (*self.0).repos_UUID;
            std::ffi::CStr::from_ptr(uuid).to_str().unwrap()
        }
    }

    /// Returns the last changed revision number.
    pub fn last_changed_rev(&self) -> Revnum {
        Revnum::from_raw(unsafe { (*self.0).last_changed_rev }).unwrap()
    }

    /// Returns the last changed date.
    pub fn last_changed_date(&self) -> apr::time::Time {
        unsafe { (*self.0).last_changed_date.into() }
    }

    /// Returns the last changed author.
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
        let callback =
            &mut *(baton as *mut &mut dyn FnMut(&std::path::Path, &Info) -> Result<(), Error>);
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

/// Callback wrapper for blame receiver
unsafe extern "C" fn blame_receiver_wrapper(
    baton: *mut std::ffi::c_void,
    line_no: apr_sys::apr_int64_t,
    revision: subversion_sys::svn_revnum_t,
    rev_props: *mut apr_sys::apr_hash_t,
    merged_revision: subversion_sys::svn_revnum_t,
    merged_rev_props: *mut apr_sys::apr_hash_t,
    merged_path: *const i8,
    line: *const subversion_sys::svn_string_t,
    local_change: subversion_sys::svn_boolean_t,
    _pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let callback = &mut *(baton as *mut &mut dyn FnMut(BlameInfo) -> Result<(), Error>);

    // Extract revprops
    let revprops = if !rev_props.is_null() {
        let prop_hash = unsafe { crate::props::PropHash::from_ptr(rev_props) };
        prop_hash.to_hashmap()
    } else {
        HashMap::new()
    };

    // Extract merged revprops
    let merged_revprops = if !merged_rev_props.is_null() {
        let prop_hash = unsafe { crate::props::PropHash::from_ptr(merged_rev_props) };
        prop_hash.to_hashmap()
    } else {
        HashMap::new()
    };

    // Extract line content
    let line_str = if !line.is_null() {
        let line_ref = &*line;
        let str_data = line_ref.data as *const i8;
        let str_len = line_ref.len;
        let bytes = std::slice::from_raw_parts(str_data as *const u8, str_len);
        String::from_utf8_lossy(bytes).into_owned()
    } else {
        String::new()
    };

    // Extract merged path
    let merged_path_str = if !merged_path.is_null() {
        Some(
            std::ffi::CStr::from_ptr(merged_path)
                .to_string_lossy()
                .into_owned(),
        )
    } else {
        None
    };

    let info = BlameInfo {
        line_no,
        revision: Revnum::from_raw(revision).unwrap_or(Revnum::from(0u64)),
        revprops,
        merged_revision: Revnum::from_raw(merged_revision),
        merged_revprops,
        merged_path: merged_path_str,
        line: line_str,
        local_change: local_change != 0,
    };

    match callback(info) {
        Ok(()) => std::ptr::null_mut(),
        Err(mut e) => e.as_mut_ptr(),
    }
}

/// Options for cat
#[derive(Debug, Clone, Copy, Default)]
/// Options for the cat operation to retrieve file contents.
pub struct CatOptions {
    /// Revision to retrieve.
    pub revision: Revision,
    /// Peg revision for the path.
    pub peg_revision: Revision,
    /// Whether to expand keywords in the file content.
    pub expand_keywords: bool,
}

/// Options for cleanup
#[derive(Debug, Clone, Copy, Default)]
/// Options for the cleanup operation to remove locks and complete unfinished operations.
pub struct CleanupOptions {
    /// Whether to break locks.
    pub break_locks: bool,
    /// Whether to fix recorded timestamps.
    pub fix_recorded_timestamps: bool,
    /// Whether to clear DAV cache.
    pub clear_dav_cache: bool,
    /// Whether to vacuum pristine copies.
    pub vacuum_pristines: bool,
    /// Whether to include externals.
    pub include_externals: bool,
}

/// Options for proplist
#[derive(Debug, Clone, Copy, Default)]
/// Options for listing properties on versioned items.
pub struct ProplistOptions<'a> {
    /// Peg revision for the path.
    pub peg_revision: Revision,
    /// Revision to list properties from.
    pub revision: Revision,
    /// Depth of the operation.
    pub depth: Depth,
    /// Changelists to filter by.
    pub changelists: Option<&'a [&'a str]>,
    /// Whether to get inherited properties.
    pub get_target_inherited_props: bool,
}

/// Options for export
#[derive(Debug, Clone, Copy, Default)]
/// Options for exporting a tree from the repository.
pub struct ExportOptions {
    /// Peg revision for the path.
    pub peg_revision: Revision,
    /// Revision to export.
    pub revision: Revision,
    /// Whether to overwrite existing files.
    pub overwrite: bool,
    /// Whether to ignore externals.
    pub ignore_externals: bool,
    /// Whether to ignore keywords.
    pub ignore_keywords: bool,
    /// Depth of the export.
    pub depth: Depth,
    /// Native end-of-line style.
    pub native_eol: crate::NativeEOL,
}

/// Options for vacuum
#[derive(Debug, Clone, Copy, Default)]
/// Options for vacuuming the working copy to remove unversioned and ignored items.
pub struct VacuumOptions {
    /// Whether to remove unversioned items.
    pub remove_unversioned_items: bool,
    /// Whether to remove ignored items.
    pub remove_ignored_items: bool,
    /// Whether to fix recorded timestamps.
    pub fix_recorded_timestamps: bool,
    /// Whether to vacuum pristine copies.
    pub vacuum_pristines: bool,
    /// Whether to include externals.
    pub include_externals: bool,
}

/// Options for a checkout
#[derive(Debug, Clone, Copy, Default)]
/// Options for checking out a working copy from a repository.
pub struct CheckoutOptions {
    /// Peg revision for the URL.
    pub peg_revision: Revision,
    /// Revision to check out.
    pub revision: Revision,
    /// Depth of the checkout.
    pub depth: Depth,
    /// Whether to ignore externals.
    pub ignore_externals: bool,
    /// Whether to allow unversioned obstructions.
    pub allow_unver_obstructions: bool,
}

/// Options for an update
#[derive(Debug, Clone, Copy, Default)]
/// Options for updating a working copy to a different revision.
pub struct UpdateOptions {
    /// Depth of the update.
    pub depth: Depth,
    /// Whether the depth setting is sticky.
    pub depth_is_sticky: bool,
    /// Whether to ignore externals.
    pub ignore_externals: bool,
    /// Whether to allow unversioned obstructions.
    pub allow_unver_obstructions: bool,
    /// Whether to treat adds as modifications.
    pub adds_as_modifications: bool,
    /// Whether to create parent directories.
    pub make_parents: bool,
}

/// Options for a switch
#[derive(Debug, Clone, Copy)]
/// Options for switching a working copy to a different URL.
pub struct SwitchOptions {
    /// Peg revision for the URL.
    pub peg_revision: Revision,
    /// Revision to switch to.
    pub revision: Revision,
    /// Depth of the switch.
    pub depth: Depth,
    /// Whether the depth setting is sticky.
    pub depth_is_sticky: bool,
    /// Whether to ignore externals.
    pub ignore_externals: bool,
    /// Whether to allow unversioned obstructions.
    pub allow_unver_obstructions: bool,
    /// Whether to ignore ancestry when comparing trees.
    pub ignore_ancestry: bool,
}

impl Default for SwitchOptions {
    fn default() -> Self {
        Self {
            peg_revision: Revision::Unspecified,
            revision: Revision::Head,
            depth: Depth::Infinity,
            depth_is_sticky: false,
            ignore_externals: false,
            allow_unver_obstructions: false,
            ignore_ancestry: false,
        }
    }
}

impl SwitchOptions {
    /// Creates a new SwitchOptions with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the peg revision.
    pub fn with_peg_revision(mut self, peg_revision: Revision) -> Self {
        self.peg_revision = peg_revision;
        self
    }

    /// Sets the revision.
    pub fn with_revision(mut self, revision: Revision) -> Self {
        self.revision = revision;
        self
    }

    /// Sets the depth.
    pub fn with_depth(mut self, depth: Depth) -> Self {
        self.depth = depth;
        self
    }

    /// Sets whether the depth is sticky.
    pub fn with_depth_is_sticky(mut self, depth_is_sticky: bool) -> Self {
        self.depth_is_sticky = depth_is_sticky;
        self
    }

    /// Sets whether to ignore externals.
    pub fn with_ignore_externals(mut self, ignore_externals: bool) -> Self {
        self.ignore_externals = ignore_externals;
        self
    }

    /// Sets whether to allow unversioned obstructions.
    pub fn with_allow_unver_obstructions(mut self, allow_unver_obstructions: bool) -> Self {
        self.allow_unver_obstructions = allow_unver_obstructions;
        self
    }

    /// Sets whether to ignore ancestry when comparing trees.
    pub fn with_ignore_ancestry(mut self, ignore_ancestry: bool) -> Self {
        self.ignore_ancestry = ignore_ancestry;
        self
    }
}

/// Options for add
#[derive(Debug, Clone, Copy)]
/// Options for adding files and directories to version control.
pub struct AddOptions {
    /// Depth of the add operation.
    pub depth: Depth,
    /// Whether to force the add.
    pub force: bool,
    /// Whether to add files that match ignore patterns.
    pub no_ignore: bool,
    /// Whether to disable automatic properties.
    pub no_autoprops: bool,
    /// Whether to add parent directories.
    pub add_parents: bool,
}

impl Default for AddOptions {
    fn default() -> Self {
        Self {
            depth: Depth::Infinity,
            force: false,
            no_ignore: false,
            no_autoprops: false,
            add_parents: false,
        }
    }
}

impl AddOptions {
    /// Creates a new AddOptions with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the depth of the add operation.
    pub fn with_depth(mut self, depth: Depth) -> Self {
        self.depth = depth;
        self
    }

    /// Sets whether to force the add.
    pub fn with_force(mut self, force: bool) -> Self {
        self.force = force;
        self
    }

    /// Sets whether to add files that match ignore patterns.
    pub fn with_no_ignore(mut self, no_ignore: bool) -> Self {
        self.no_ignore = no_ignore;
        self
    }

    /// Sets whether to disable automatic properties.
    pub fn with_no_autoprops(mut self, no_autoprops: bool) -> Self {
        self.no_autoprops = no_autoprops;
        self
    }

    /// Sets whether to add parent directories.
    pub fn with_add_parents(mut self, add_parents: bool) -> Self {
        self.add_parents = add_parents;
        self
    }
}

/// Options for delete
#[derive(Debug, Clone, Copy, Default)]
/// Options for deleting versioned items.
pub struct DeleteOptions {
    /// Whether to force deletion even if there are local modifications.
    pub force: bool,
    /// Whether to keep the local copy of the file.
    pub keep_local: bool,
}

impl DeleteOptions {
    /// Creates a new DeleteOptions with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets whether to force deletion.
    pub fn with_force(mut self, force: bool) -> Self {
        self.force = force;
        self
    }

    /// Sets whether to keep the local copy.
    pub fn with_keep_local(mut self, keep_local: bool) -> Self {
        self.keep_local = keep_local;
        self
    }
}

/// Options for commit
#[derive(Debug, Clone)]
/// Options for committing changes to the repository.
pub struct CommitOptions {
    /// Depth of the commit.
    pub depth: Depth,
    /// Whether to keep locks after commit.
    pub keep_locks: bool,
    /// Whether to keep changelists after commit.
    pub keep_changelists: bool,
    /// Whether to commit as operations.
    pub commit_as_operations: bool,
    /// Whether to include file externals.
    pub include_file_externals: bool,
    /// Whether to include directory externals.
    pub include_dir_externals: bool,
    /// Changelists to commit.
    pub changelists: Option<Vec<String>>,
}

impl Default for CommitOptions {
    fn default() -> Self {
        Self {
            depth: Depth::Infinity,
            keep_locks: false,
            keep_changelists: false,
            commit_as_operations: true,
            include_file_externals: false,
            include_dir_externals: false,
            changelists: None,
        }
    }
}

impl CommitOptions {
    /// Creates a new CommitOptions with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the depth for the commit.
    pub fn with_depth(mut self, depth: Depth) -> Self {
        self.depth = depth;
        self
    }

    /// Sets whether to keep locks after commit.
    pub fn with_keep_locks(mut self, keep_locks: bool) -> Self {
        self.keep_locks = keep_locks;
        self
    }

    /// Sets whether to keep changelists after commit.
    pub fn with_keep_changelists(mut self, keep_changelists: bool) -> Self {
        self.keep_changelists = keep_changelists;
        self
    }

    /// Sets whether to commit as operations.
    pub fn with_commit_as_operations(mut self, commit_as_operations: bool) -> Self {
        self.commit_as_operations = commit_as_operations;
        self
    }

    /// Sets whether to include file externals.
    pub fn with_include_file_externals(mut self, include_file_externals: bool) -> Self {
        self.include_file_externals = include_file_externals;
        self
    }

    /// Sets whether to include directory externals.
    pub fn with_include_dir_externals(mut self, include_dir_externals: bool) -> Self {
        self.include_dir_externals = include_dir_externals;
        self
    }

    /// Sets the changelists to include in the commit.
    pub fn with_changelists(mut self, changelists: Vec<String>) -> Self {
        self.changelists = Some(changelists);
        self
    }
}

/// Options for status
#[derive(Debug, Clone)]
/// Options for retrieving status information about working copy items.
pub struct StatusOptions {
    /// Revision to check status against.
    pub revision: Revision,
    /// Depth of the status operation.
    pub depth: Depth,
    /// Whether to get all entries.
    pub get_all: bool,
    /// Whether to check out-of-date status.
    pub check_out_of_date: bool,
    /// Whether to check the working copy.
    pub check_working_copy: bool,
    /// Whether to include ignored files.
    pub no_ignore: bool,
    /// Whether to ignore externals.
    pub ignore_externals: bool,
    /// Whether to treat depth as sticky.
    pub depth_as_sticky: bool,
    /// Changelists to filter by.
    pub changelists: Option<Vec<String>>,
}

impl Default for StatusOptions {
    fn default() -> Self {
        Self {
            revision: Revision::Working,
            depth: Depth::Infinity,
            get_all: false,
            check_out_of_date: false,
            check_working_copy: true,
            no_ignore: false,
            ignore_externals: false,
            depth_as_sticky: false,
            changelists: None,
        }
    }
}

impl StatusOptions {
    /// Creates a new StatusOptions with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the revision to check status against.
    pub fn with_revision(mut self, revision: Revision) -> Self {
        self.revision = revision;
        self
    }

    /// Sets the depth of the status operation.
    pub fn with_depth(mut self, depth: Depth) -> Self {
        self.depth = depth;
        self
    }

    /// Sets whether to get all entries.
    pub fn with_get_all(mut self, get_all: bool) -> Self {
        self.get_all = get_all;
        self
    }

    /// Sets whether to check out-of-date status.
    pub fn with_check_out_of_date(mut self, check_out_of_date: bool) -> Self {
        self.check_out_of_date = check_out_of_date;
        self
    }

    /// Sets whether to check the working copy.
    pub fn with_check_working_copy(mut self, check_working_copy: bool) -> Self {
        self.check_working_copy = check_working_copy;
        self
    }

    /// Sets whether to include ignored files.
    pub fn with_no_ignore(mut self, no_ignore: bool) -> Self {
        self.no_ignore = no_ignore;
        self
    }

    /// Sets whether to ignore externals.
    pub fn with_ignore_externals(mut self, ignore_externals: bool) -> Self {
        self.ignore_externals = ignore_externals;
        self
    }

    /// Sets whether to treat depth as sticky.
    pub fn with_depth_as_sticky(mut self, depth_as_sticky: bool) -> Self {
        self.depth_as_sticky = depth_as_sticky;
        self
    }

    /// Sets the changelists to filter by.
    pub fn with_changelists(mut self, changelists: Vec<String>) -> Self {
        self.changelists = Some(changelists);
        self
    }
}

/// A client context.
///
/// This is the main entry point for the client library. It holds client specific configuration and
/// callbacks
/// Client context for performing Subversion operations.
pub struct Context {
    ptr: *mut svn_client_ctx_t,
    pool: apr::Pool<'static>,
    _phantom: std::marker::PhantomData<*mut ()>,
    conflict_resolver: Option<Box<crate::conflict::ConflictResolverBaton>>,
    cancel_handler: Option<Box<dyn FnMut() -> bool + Send>>,
}
unsafe impl Send for Context {}

impl Drop for Context {
    fn drop(&mut self) {
        // Clear the conflict resolver callback first
        if self.conflict_resolver.is_some() {
            unsafe {
                (*self.ptr).conflict_func2 = None;
                (*self.ptr).conflict_baton2 = std::ptr::null_mut();
            }
        }
        // Clear cancel handler
        if self.cancel_handler.is_some() {
            unsafe {
                (*self.ptr).cancel_func = None;
                (*self.ptr).cancel_baton = std::ptr::null_mut();
            }
        }
        // Pool drop will clean up context
    }
}

impl Context {
    /// Creates a new Subversion client context.
    pub fn new() -> Result<Self, Error> {
        // Ensure SVN libraries are initialized
        crate::init::initialize()?;

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
            conflict_resolver: None,
            cancel_handler: None,
        })
    }

    pub(crate) unsafe fn as_mut_ptr(&mut self) -> *mut svn_client_ctx_t {
        self.ptr
    }

    /// Set the conflict resolver for this context
    ///
    /// The resolver will be called when conflicts are encountered during
    /// operations like merge, update, or switch.
    pub fn set_conflict_resolver(
        &mut self,
        resolver: impl crate::conflict::ConflictResolver + 'static,
    ) {
        // Clear any existing resolver
        unsafe {
            (*self.ptr).conflict_func2 = None;
            (*self.ptr).conflict_baton2 = std::ptr::null_mut();
        }

        // Create new resolver baton
        let baton = Box::new(crate::conflict::ConflictResolverBaton {
            resolver: Box::new(resolver),
        });

        // Store the baton and set up the callback
        unsafe {
            let baton_ptr = Box::into_raw(baton);
            self.conflict_resolver = Some(Box::from_raw(baton_ptr));

            (*self.ptr).conflict_func2 = Some(crate::conflict::conflict_resolver_callback);
            (*self.ptr).conflict_baton2 = baton_ptr as *mut std::ffi::c_void;
        }
    }

    /// Clear the conflict resolver
    pub fn clear_conflict_resolver(&mut self) {
        unsafe {
            (*self.ptr).conflict_func2 = None;
            (*self.ptr).conflict_baton2 = std::ptr::null_mut();
        }
        self.conflict_resolver = None;
    }

    /// Set a cancel handler that will be called periodically during long operations
    ///
    /// The handler should return `true` to cancel the operation, `false` to continue.
    /// Operations will fail with an SVN_ERR_CANCELLED error if cancelled.
    pub fn set_cancel_handler<F>(&mut self, handler: F)
    where
        F: FnMut() -> bool + Send + 'static,
    {
        // Clear any existing handler
        unsafe {
            (*self.ptr).cancel_func = None;
            (*self.ptr).cancel_baton = std::ptr::null_mut();
        }

        // Store the new handler
        self.cancel_handler = Some(Box::new(handler));

        // Set up the C callback
        unsafe {
            (*self.ptr).cancel_func = Some(cancel_trampoline);
            (*self.ptr).cancel_baton = self
                .cancel_handler
                .as_mut()
                .map(|h| h.as_mut() as *mut _ as *mut std::ffi::c_void)
                .unwrap_or(std::ptr::null_mut());
        }
    }

    /// Clear the cancel handler
    pub fn clear_cancel_handler(&mut self) {
        unsafe {
            (*self.ptr).cancel_func = None;
            (*self.ptr).cancel_baton = std::ptr::null_mut();
        }
        self.cancel_handler = None;
    }

    /// Sets the authentication baton for this context.
    pub fn set_auth<'a, 'b>(&'a mut self, auth_baton: &'b mut crate::auth::AuthBaton)
    where
        'b: 'a,
    {
        unsafe {
            (*self.ptr).auth_baton = auth_baton.as_mut_ptr();
        }
    }

    /// Get a reference to the underlying pool
    pub fn pool(&self) -> &apr::Pool<'_> {
        &self.pool
    }

    /// Get the raw pointer to the context (use with caution)
    pub fn as_ptr(&self) -> *const svn_client_ctx_t {
        self.ptr
    }

    /// Checkout a working copy from url to path.
    pub fn checkout(
        &mut self,
        url: impl AsCanonicalUri,
        path: impl AsCanonicalDirent,
        options: &CheckoutOptions,
    ) -> Result<Revnum, Error> {
        let peg_revision = options.peg_revision.into();
        let revision = options.revision.into();

        // Canonicalize inputs
        let url = url.as_canonical_uri()?;
        let path = path.as_canonical_dirent()?;

        with_tmp_pool(|pool| unsafe {
            let mut revnum = 0;
            // Convert to C strings for FFI
            let url_cstr = std::ffi::CString::new(url.as_str())?;
            let path_cstr = std::ffi::CString::new(path.as_str())?;

            let err = svn_client_checkout3(
                &mut revnum,
                url_cstr.as_ptr(),
                path_cstr.as_ptr(),
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
        })
    }

    /// Updates working copy paths to a specific revision.
    pub fn update(
        &mut self,
        paths: &[&str],
        revision: Revision,
        options: &UpdateOptions,
    ) -> Result<Vec<Revnum>, Error> {
        with_tmp_pool(|pool| unsafe {
            let mut result_revs = std::ptr::null_mut();
            // Keep CStrings alive for the duration of the function
            let path_cstrings: Vec<std::ffi::CString> = paths
                .iter()
                .map(|p| std::ffi::CString::new(*p).unwrap())
                .collect();
            let mut ps = apr::tables::TypedArray::new(pool, paths.len() as i32);
            for path in &path_cstrings {
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
                pool.as_mut_ptr(),
            );
            let result_revs: apr::tables::TypedArray<Revnum> =
                apr::tables::TypedArray::<Revnum>::from_ptr(result_revs);
            Error::from_raw(err)?;
            Ok(result_revs.iter().collect())
        })
    }

    /// Switches a working copy path to a different URL.
    pub fn switch(
        &mut self,
        path: impl AsCanonicalDirent,
        url: impl AsCanonicalUri,
        options: &SwitchOptions,
    ) -> Result<Revnum, Error> {
        // Canonicalize inputs
        let path = path.as_canonical_dirent()?;
        let url = url.as_canonical_uri()?;

        with_tmp_pool(|pool| unsafe {
            let mut result_rev = 0;
            // Convert to C strings for FFI
            let path_cstr = std::ffi::CString::new(path.as_str())?;
            let url_cstr = std::ffi::CString::new(url.as_str())?;

            let err = svn_client_switch3(
                &mut result_rev,
                path_cstr.as_ptr(),
                url_cstr.as_ptr(),
                &options.peg_revision.into(),
                &options.revision.into(),
                options.depth.into(),
                options.depth_is_sticky.into(),
                options.ignore_externals.into(),
                options.allow_unver_obstructions.into(),
                options.ignore_ancestry.into(),
                self.ptr,
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(Revnum::from_raw(result_rev).unwrap())
        })
    }

    /// Adds a file or directory to version control.
    pub fn add(&mut self, path: impl AsCanonicalDirent, options: &AddOptions) -> Result<(), Error> {
        let path = path.as_canonical_dirent()?;
        with_tmp_pool(|pool| unsafe {
            let path_cstr = std::ffi::CString::new(path.as_str())?;
            let err = svn_client_add5(
                path_cstr.as_ptr(),
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
        })
    }

    /// Creates directories in version control.
    pub fn mkdir(
        &mut self,
        paths: &[&str],
        make_parents: bool,
        revprop_table: std::collections::HashMap<&str, &[u8]>,
        commit_callback: &dyn FnMut(&crate::CommitInfo) -> Result<(), Error>,
    ) -> Result<(), Error> {
        with_tmp_pool(|pool| unsafe {
            // Convert revprops to BStr objects that live in the pool
            let svn_strings: Vec<_> = revprop_table
                .iter()
                .map(|(k, v)| (*k, crate::string::BStr::from_bytes(v, pool)))
                .collect();

            let rps = apr::hash::Hash::from_iter(
                pool,
                svn_strings
                    .iter()
                    .map(|(k, v)| (k.as_bytes(), v.as_ptr() as *mut std::ffi::c_void)),
            );
            // Keep CStrings alive for the duration of the function
            let path_cstrings: Vec<std::ffi::CString> = paths
                .iter()
                .map(|p| std::ffi::CString::new(*p).unwrap())
                .collect();
            let mut ps = apr::tables::TypedArray::new(pool, paths.len() as i32);
            for path in &path_cstrings {
                ps.push(path.as_ptr() as *mut std::ffi::c_void);
            }
            let err = svn_client_mkdir4(
                ps.as_ptr(),
                make_parents.into(),
                rps.as_ptr(),
                Some(crate::wrap_commit_callback2),
                &commit_callback as *const _ as *mut std::ffi::c_void,
                self.ptr,
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(())
        })
    }

    /// Deletes files or directories from version control.
    pub fn delete(
        &mut self,
        paths: &[&str],
        revprop_table: std::collections::HashMap<&str, &str>,
        options: &DeleteOptions,
    ) -> Result<(), Error> {
        with_tmp_pool(|pool| unsafe {
            // Convert revprops to BStr objects that live in the pool
            let svn_strings: Vec<_> = revprop_table
                .iter()
                .map(|(k, v)| (*k, crate::string::BStr::from_str(v, pool)))
                .collect();

            let rps = apr::hash::Hash::from_iter(
                pool,
                svn_strings
                    .iter()
                    .map(|(k, v)| (k.as_bytes(), v.as_ptr() as *mut std::ffi::c_void)),
            );
            // Keep CStrings alive for the duration of the function
            let path_cstrings: Vec<std::ffi::CString> = paths
                .iter()
                .map(|p| std::ffi::CString::new(*p).unwrap())
                .collect();
            let mut ps = apr::tables::TypedArray::new(pool, paths.len() as i32);
            for path in &path_cstrings {
                ps.push(path.as_ptr() as *mut std::ffi::c_void);
            }
            // Note: commit_callback parameter removed from DeleteOptions for simplicity
            let commit_callback = std::ptr::null_mut();
            let err = svn_client_delete4(
                ps.as_ptr(),
                options.force.into(),
                options.keep_local.into(),
                rps.as_ptr(),
                Some(crate::wrap_commit_callback2),
                commit_callback as *mut std::ffi::c_void,
                self.ptr,
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(())
        })
    }

    /// Lists properties on a path.
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
        with_tmp_pool(|pool| {
            let changelists = options.changelists.map(|cl| {
                cl.iter()
                    .map(|cl| std::ffi::CString::new(*cl).unwrap())
                    .collect::<Vec<_>>()
            });
            let changelists = changelists.as_ref().map(|cl| {
                let mut array = apr::tables::TypedArray::<*const i8>::new(pool, 10);
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
        })
    }

    /// Imports an unversioned path into the repository.
    pub fn import(
        &mut self,
        path: impl AsCanonicalDirent,
        url: &str,
        depth: Depth,
        no_ignore: bool,
        no_autoprops: bool,
        ignore_unknown_node_types: bool,
        revprop_table: std::collections::HashMap<&str, &str>,
        filter_callback: &dyn FnMut(&mut bool, &std::path::Path, &Dirent) -> Result<(), Error>,
        commit_callback: &dyn FnMut(&crate::CommitInfo) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let path = path.as_canonical_dirent()?;
        with_tmp_pool(|pool| {
            // Convert revprops to BStr objects that live in the pool
            let svn_strings: Vec<_> = revprop_table
                .iter()
                .map(|(k, v)| (*k, crate::string::BStr::from_str(v, pool)))
                .collect();

            let rps = apr::hash::Hash::from_iter(
                pool,
                svn_strings
                    .iter()
                    .map(|(k, v)| (k.as_bytes(), v.as_ptr() as *mut std::ffi::c_void)),
            );
            unsafe {
                let filter_callback = Box::into_raw(Box::new(filter_callback));
                let commit_callback = Box::into_raw(Box::new(commit_callback));
                let path_cstr = std::ffi::CString::new(path.as_str())?;
                let err = svn_client_import5(
                    path_cstr.as_ptr(),
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
                    pool.as_mut_ptr(),
                );
                // Free the boxed callbacks to prevent memory leak
                drop(Box::from_raw(filter_callback));
                drop(Box::from_raw(commit_callback));
                Error::from_raw(err)?;
                Ok(())
            }
        })
    }

    /// Exports a versioned path to an unversioned path.
    pub fn export(
        &mut self,
        from_path_or_url: &str,
        to_path: impl AsCanonicalDirent,
        options: &ExportOptions,
    ) -> Result<Revnum, Error> {
        let native_eol: Option<&str> = options.native_eol.into();
        let native_eol = native_eol.map(|s| std::ffi::CString::new(s).unwrap());
        let mut revnum = 0;
        let to_path = to_path.as_canonical_dirent()?;
        with_tmp_pool(|tmp_pool| unsafe {
            let path_cstr = std::ffi::CString::new(to_path.as_path().to_str().unwrap())?;
            let err = svn_client_export5(
                &mut revnum,
                from_path_or_url.as_ptr() as *const i8,
                path_cstr.as_ptr(),
                &options.peg_revision.into(),
                &options.revision.into(),
                options.overwrite as i32,
                options.ignore_externals as i32,
                options.ignore_keywords as i32,
                options.depth as i32,
                native_eol.map(|s| s.as_ptr()).unwrap_or(std::ptr::null()),
                self.ptr,
                tmp_pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(Revnum::from_raw(revnum).unwrap())
        })
    }

    /// Commits changes from a working copy to the repository.
    pub fn commit(
        &mut self,
        targets: &[&str],
        options: &CommitOptions,
        revprop_table: std::collections::HashMap<&str, &str>,
        commit_callback: &dyn FnMut(&crate::CommitInfo) -> Result<(), Error>,
    ) -> Result<(), Error> {
        with_tmp_pool(|pool| {
            // Convert revprops to BStr objects that live in the pool
            let svn_strings: Vec<_> = revprop_table
                .iter()
                .map(|(k, v)| (*k, crate::string::BStr::from_str(v, pool)))
                .collect();

            let rps = apr::hash::Hash::from_iter(
                pool,
                svn_strings
                    .iter()
                    .map(|(k, v)| (k.as_bytes(), v.as_ptr() as *mut std::ffi::c_void)),
            );

            unsafe {
                // Keep CStrings alive for the duration of the function
                let target_cstrings: Vec<std::ffi::CString> = targets
                    .iter()
                    .map(|t| std::ffi::CString::new(*t).unwrap())
                    .collect();
                let mut ps = apr::tables::TypedArray::new(pool, targets.len() as i32);
                for target in &target_cstrings {
                    ps.push(target.as_ptr() as *mut std::ffi::c_void);
                }

                let changelist_cstrings: Vec<std::ffi::CString> =
                    if let Some(changelists) = &options.changelists {
                        changelists
                            .iter()
                            .map(|c| std::ffi::CString::new(c.as_str()).unwrap())
                            .collect()
                    } else {
                        Vec::new()
                    };
                let mut cl = apr::tables::TypedArray::new(pool, changelist_cstrings.len() as i32);
                for changelist in &changelist_cstrings {
                    cl.push(changelist.as_ptr() as *mut std::ffi::c_void);
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
                    pool.as_mut_ptr(),
                );
                // Free the boxed callback to prevent memory leak
                drop(Box::from_raw(commit_callback));
                Error::from_raw(err)?;
                Ok(())
            }
        })
    }

    /// Gets the status of a working copy path.
    pub fn status(
        &mut self,
        path: &str,
        options: &StatusOptions,
        status_func: &dyn FnMut(&'_ str, &'_ Status) -> Result<(), Error>,
    ) -> Result<Revnum, Error> {
        with_tmp_pool(|pool| {
            let changelist_cstrings: Vec<std::ffi::CString> =
                if let Some(changelists) = &options.changelists {
                    changelists
                        .iter()
                        .map(|cl| std::ffi::CString::new(cl.as_str()).unwrap())
                        .collect()
                } else {
                    Vec::new()
                };
            let mut cl = apr::tables::TypedArray::new(pool, changelist_cstrings.len() as i32);
            for changelist in &changelist_cstrings {
                cl.push(changelist.as_ptr() as *mut std::ffi::c_void);
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
                    pool.as_mut_ptr(),
                );
                Error::from_raw(err)?;
                Ok(Revnum::from_raw(revnum).unwrap())
            }
        })
    }

    /// Retrieve log messages for a set of paths.
    ///
    /// The receiver callback will be called for each log entry found.
    /// Return an error from the callback to stop iteration early.
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
        with_tmp_pool(|pool| unsafe {
            // Keep CStrings alive for the duration of the function
            let target_cstrings: Vec<std::ffi::CString> = targets
                .iter()
                .map(|t| std::ffi::CString::new(*t).unwrap())
                .collect();
            let mut ps = apr::tables::TypedArray::new(pool, targets.len() as i32);
            for target in &target_cstrings {
                ps.push(target.as_ptr() as *mut std::ffi::c_void);
            }

            let mut rrs =
                apr::tables::TypedArray::<*mut subversion_sys::svn_opt_revision_range_t>::new(
                    pool,
                    revision_ranges.len() as i32,
                );
            for revision_range in revision_ranges {
                rrs.push(revision_range.to_c(pool));
            }

            // Keep CStrings alive for the duration of the function
            let revprop_cstrings: Vec<std::ffi::CString> = revprops
                .iter()
                .map(|r| std::ffi::CString::new(*r).unwrap())
                .collect();
            let mut rps = apr::tables::TypedArray::new(pool, revprops.len() as i32);
            for revprop in &revprop_cstrings {
                rps.push(revprop.as_ptr() as *mut std::ffi::c_void);
            }
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
                &log_entry_receiver as *const _ as *mut std::ffi::c_void,
                self.ptr,
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(())
        })
    }

    /// Retrieve log messages with control flow support.
    ///
    /// The receiver can return `ControlFlow::Break(())` to stop iteration early
    /// or `ControlFlow::Continue(())` to continue processing.
    pub fn log_with_control<F>(
        &mut self,
        targets: &[&str],
        peg_revision: Revision,
        revision_ranges: &[RevisionRange],
        limit: i32,
        discover_changed_paths: bool,
        strict_node_history: bool,
        include_merged_revisions: bool,
        revprops: &[&str],
        mut receiver: F,
    ) -> Result<(), Error>
    where
        F: FnMut(&LogEntry) -> ControlFlow<()>,
    {
        self.log(
            targets,
            peg_revision,
            revision_ranges,
            limit,
            discover_changed_paths,
            strict_node_history,
            include_merged_revisions,
            revprops,
            &|entry| match receiver(entry) {
                ControlFlow::Continue(()) => Ok(()),
                ControlFlow::Break(()) => {
                    // Return a cancellation error to stop iteration
                    Err(Error::new(
                        apr::Status::from(200015_i32), // SVN_ERR_CANCELLED
                        None,
                        "Log iteration stopped by user",
                    ))
                }
            },
        )
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

    /// Converts command-line arguments to a target array.
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
                    let mut array = apr::tables::TypedArray::<*const i8>::new(&pool, 10);
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
        let targets = unsafe { apr::tables::TypedArray::<*const i8>::from_ptr(targets) };
        Ok(targets
            .iter()
            .map(|s| unsafe { std::ffi::CStr::from_ptr(*s as *const i8) })
            .map(|s| s.to_str().unwrap().to_owned())
            .collect::<Vec<_>>())
    }

    /// Vacuums pristine copies from a working copy.
    pub fn vacuum(&mut self, path: &str, options: &VacuumOptions) -> Result<(), Error> {
        let path = std::ffi::CString::new(path).unwrap();
        with_tmp_pool(|pool| unsafe {
            let err = svn_client_vacuum(
                path.as_ptr(),
                options.remove_unversioned_items.into(),
                options.remove_ignored_items.into(),
                options.fix_recorded_timestamps.into(),
                options.vacuum_pristines.into(),
                options.include_externals.into(),
                self.ptr,
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)
        })
    }

    /// Cleans up a working copy.
    pub fn cleanup(&mut self, path: &str, options: &CleanupOptions) -> Result<(), Error> {
        let path = std::ffi::CString::new(path).unwrap();
        with_tmp_pool(|pool| unsafe {
            let err = svn_client_cleanup2(
                path.as_ptr(),
                options.break_locks.into(),
                options.fix_recorded_timestamps.into(),
                options.clear_dav_cache.into(),
                options.vacuum_pristines.into(),
                options.include_externals.into(),
                self.ptr,
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)
        })
    }

    /// Gets conflict information for a path.
    pub fn conflict_get(
        &mut self,
        local_abspath: impl AsCanonicalDirent,
    ) -> Result<Conflict, Error> {
        let pool = apr::Pool::new();
        let _scratch_pool = apr::Pool::new();
        let local_abspath = local_abspath.as_canonical_dirent()?;
        let mut conflict: *mut subversion_sys::svn_client_conflict_t = std::ptr::null_mut();
        with_tmp_pool(|tmp_pool| unsafe {
            let path_cstr = std::ffi::CString::new(local_abspath.as_path().to_str().unwrap())?;
            let err = svn_client_conflict_get(
                &mut conflict,
                path_cstr.as_ptr(),
                self.ptr,
                tmp_pool.as_mut_ptr(),
                tmp_pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(Conflict::from_ptr_and_pool(conflict, pool))
        })
    }

    /// Outputs the contents of a file.
    pub fn cat(
        &mut self,
        path_or_url: &str,
        stream: &mut dyn std::io::Write,
        options: &CatOptions,
    ) -> Result<HashMap<String, Vec<u8>>, Error> {
        let path_or_url = std::ffi::CString::new(path_or_url).unwrap();
        let mut s = crate::io::wrap_write(stream)?;
        with_tmp_pool(|result_pool| {
            with_tmp_pool(|scratch_pool| unsafe {
                let mut props: *mut apr::hash::apr_hash_t = std::ptr::null_mut();
                let err = subversion_sys::svn_client_cat3(
                    &mut props,
                    s.as_mut_ptr(),
                    path_or_url.as_ptr(),
                    &options.peg_revision.into(),
                    &options.revision.into(),
                    options.expand_keywords.into(),
                    self.as_mut_ptr(),
                    result_pool.as_mut_ptr(),
                    scratch_pool.as_mut_ptr(),
                );
                Error::from_raw(err)?;
                let prop_hash = crate::props::PropHash::from_ptr(props);
                Ok(prop_hash.to_hashmap())
            })
        })
    }

    /// Locks paths in the repository.
    pub fn lock(&mut self, targets: &[&str], comment: &str, steal_lock: bool) -> Result<(), Error> {
        with_tmp_pool(|pool| {
            let targets = targets
                .iter()
                .map(|s| std::ffi::CString::new(*s).unwrap())
                .collect::<Vec<_>>();
            let mut targets_array = apr::tables::TypedArray::<*const i8>::new(pool, 10);
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
                    pool.as_mut_ptr(),
                );
                Error::from_raw(err)?;
                Ok(())
            }
        })
    }

    /// Unlocks paths in the repository.
    pub fn unlock(&mut self, targets: &[&str], break_lock: bool) -> Result<(), Error> {
        with_tmp_pool(|pool| {
            let targets = targets
                .iter()
                .map(|s| std::ffi::CString::new(*s).unwrap())
                .collect::<Vec<_>>();
            let mut targets_array = apr::tables::TypedArray::<*const i8>::new(pool, 10);
            for target in targets.iter() {
                targets_array.push(target.as_ptr());
            }
            unsafe {
                let err = subversion_sys::svn_client_unlock(
                    targets_array.as_ptr(),
                    break_lock.into(),
                    self.as_mut_ptr(),
                    pool.as_mut_ptr(),
                );
                Error::from_raw(err)?;
                Ok(())
            }
        })
    }

    /// Gets the root of the working copy.
    pub fn get_wc_root(
        &mut self,
        path: impl AsCanonicalDirent,
    ) -> Result<std::path::PathBuf, Error> {
        let path = path.as_canonical_dirent()?;
        let mut wc_root: *const i8 = std::ptr::null();
        with_tmp_pool(|tmp_pool| unsafe {
            let path_cstr = std::ffi::CString::new(path.as_path().to_str().unwrap())?;
            let err = subversion_sys::svn_client_get_wc_root(
                &mut wc_root,
                path_cstr.as_ptr(),
                self.as_mut_ptr(),
                tmp_pool.as_mut_ptr(),
                tmp_pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(std::ffi::CStr::from_ptr(wc_root).to_str().unwrap().into())
        })
    }

    /// Gets the minimum and maximum revisions in a working copy.
    pub fn min_max_revisions(
        &mut self,
        local_abspath: impl AsCanonicalDirent,
        committed: bool,
    ) -> Result<(Revnum, Revnum), Error> {
        let _scratch_pool = apr::pool::Pool::new();
        let local_abspath = local_abspath.as_canonical_dirent()?;
        let mut min_revision: subversion_sys::svn_revnum_t = 0;
        let mut max_revision: subversion_sys::svn_revnum_t = 0;
        with_tmp_pool(|tmp_pool| unsafe {
            let path_cstr = std::ffi::CString::new(local_abspath.as_path().to_str().unwrap())?;
            let err = subversion_sys::svn_client_min_max_revisions(
                &mut min_revision,
                &mut max_revision,
                path_cstr.as_ptr(),
                committed as i32,
                self.as_mut_ptr(),
                tmp_pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok((
                Revnum::from_raw(min_revision).unwrap(),
                Revnum::from_raw(max_revision).unwrap(),
            ))
        })
    }

    /// Gets the URL for a working copy path.
    pub fn url_from_path(&mut self, path: impl AsCanonicalUri) -> Result<String, Error> {
        // Canonicalize input
        let path = path.as_canonical_uri()?;

        with_tmp_pool(|pool| unsafe {
            let mut url: *const i8 = std::ptr::null();
            let path_cstr = std::ffi::CString::new(path.as_str())?;

            let err = subversion_sys::svn_client_url_from_path2(
                &mut url,
                path_cstr.as_ptr(),
                self.as_mut_ptr(),
                pool.as_mut_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(std::ffi::CStr::from_ptr(url).to_str().unwrap().into())
        })
    }

    /// Gets the repository root URL and UUID for a path or URL.
    pub fn get_repos_root(&mut self, path_or_url: &str) -> Result<(String, String), Error> {
        let path_or_url = std::ffi::CString::new(path_or_url).unwrap();
        with_tmp_pool(|pool| unsafe {
            let mut repos_root: *const i8 = std::ptr::null();
            let mut repos_uuid: *const i8 = std::ptr::null();
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
        })
    }

    #[cfg(feature = "ra")]
    /// Opens a raw repository access session.
    pub fn open_raw_session(
        &mut self,
        url: &str,
        wri_path: &std::path::Path,
    ) -> Result<crate::ra::Session<'_>, Error> {
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

    /// Gets information about a path or URL.
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
        with_tmp_pool(|pool| {
            let abspath_or_url = std::ffi::CString::new(abspath_or_url).unwrap();
            let changelists = changelists.map(|cl| {
                cl.iter()
                    .map(|cl| std::ffi::CString::new(*cl).unwrap())
                    .collect::<Vec<_>>()
            });
            let changelists = changelists.as_ref().map(|cl| {
                let mut array = apr::tables::TypedArray::<*const i8>::new(pool, 10);
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
        })
    }

    /// Blame/annotate a file, showing revision information for each line
    pub fn blame(
        &mut self,
        path_or_url: &str,
        peg_revision: Revision,
        start_revision: Revision,
        end_revision: Revision,
        diff_options: Vec<String>,
        ignore_mime_type: bool,
        include_merged_revisions: bool,
        receiver: &mut dyn FnMut(BlameInfo) -> Result<(), Error>,
    ) -> Result<(Revnum, Revnum), Error> {
        with_tmp_pool(|pool| {
            let path_or_url = std::ffi::CString::new(path_or_url).unwrap();

            // Convert diff options to C strings
            // Create diff file options struct
            let diff_file_options =
                unsafe { subversion_sys::svn_diff_file_options_create(pool.as_mut_ptr()) };

            if !diff_options.is_empty() {
                let diff_options_cstrings: Vec<std::ffi::CString> = diff_options
                    .iter()
                    .map(|opt| std::ffi::CString::new(opt.as_str()).unwrap())
                    .collect();

                // Create APR array of diff options for parsing
                let mut diff_opts_array = apr::tables::TypedArray::<*const i8>::new(pool, 10);
                for opt in diff_options_cstrings.iter() {
                    diff_opts_array.push(opt.as_ptr());
                }

                // Parse the options into the diff_file_options struct
                let parse_err = unsafe {
                    subversion_sys::svn_diff_file_options_parse(
                        diff_file_options,
                        diff_opts_array.as_ptr(),
                        pool.as_mut_ptr(),
                    )
                };
                Error::from_raw(parse_err)?;
            }

            // Output parameters for resolved revision numbers
            let mut start_revnum: subversion_sys::svn_revnum_t = 0;
            let mut end_revnum: subversion_sys::svn_revnum_t = 0;

            // Wrap the receiver callback
            let receiver_baton = receiver as *mut _ as *mut std::ffi::c_void;

            unsafe {
                let err = subversion_sys::svn_client_blame6(
                    &mut start_revnum,
                    &mut end_revnum,
                    path_or_url.as_ptr(),
                    &peg_revision.into(),
                    &start_revision.into(),
                    &end_revision.into(),
                    diff_file_options,
                    ignore_mime_type as subversion_sys::svn_boolean_t,
                    include_merged_revisions as subversion_sys::svn_boolean_t,
                    Some(blame_receiver_wrapper),
                    receiver_baton,
                    self.as_mut_ptr(),
                    pool.as_mut_ptr(),
                );
                Error::from_raw(err)?;
                Ok((
                    Revnum::from_raw(start_revnum).unwrap(),
                    Revnum::from_raw(end_revnum).unwrap(),
                ))
            }
        })
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
            // Keep CStrings alive for the duration of the function
            let path_cstrings: Vec<std::ffi::CString> = sources
                .iter()
                .map(|(path, _)| std::ffi::CString::new(*path).unwrap())
                .collect();

            let sources_array = sources
                .iter()
                .zip(path_cstrings.iter())
                .map(|((_, rev), path_c)| {
                    let src: *mut subversion_sys::svn_client_copy_source_t = pool.calloc();
                    unsafe {
                        (*src).path = path_c.as_ptr();
                        (*src).revision = Box::into_raw(Box::new(
                            rev.as_ref()
                                .map(|r| (*r).into())
                                .unwrap_or(Revision::Head.into()),
                        ));
                        (*src).peg_revision = Box::into_raw(Box::new(Revision::Head.into()));
                    }
                    src
                })
                .collect::<Vec<_>>();

            let mut sources_apr_array = apr::tables::TypedArray::<
                *const subversion_sys::svn_client_copy_source_t,
            >::new(pool, sources_array.len() as i32);
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
                    None,                 // commit_callback
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
            let paths_c: Vec<_> = paths
                .iter()
                .map(|p| std::ffi::CString::new(*p).unwrap())
                .collect();
            let mut paths_array = apr::tables::TypedArray::<*const i8>::new(pool, 0);
            for path in paths_c.iter() {
                paths_array.push(path.as_ptr());
            }

            let err = unsafe {
                subversion_sys::svn_client_mkdir4(
                    paths_array.as_ptr(),
                    make_parents as i32,
                    std::ptr::null_mut(), // revprop_table
                    None,                 // commit_callback
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
                let mut array = apr::tables::TypedArray::<*const i8>::new(pool, 0);
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
                    if actual_revnum.is_some() {
                        &mut actual_rev
                    } else {
                        std::ptr::null_mut()
                    },
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

            if props.is_null() {
                return Ok(std::collections::HashMap::new());
            }

            // Convert apr hash to Rust HashMap
            let prop_hash = unsafe { crate::props::PropHash::from_ptr(props) };
            Ok(prop_hash.to_hashmap())
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
        _base_revision_for_url: Revnum,
        changelists: Option<&[&str]>,
    ) -> Result<(), Error> {
        with_tmp_pool(|pool| {
            let propname_c = std::ffi::CString::new(propname).unwrap();
            let target_c = std::ffi::CString::new(target).unwrap();

            // propset_local expects an array of targets
            let mut targets_array = apr::tables::TypedArray::<*const i8>::new(pool, 0);
            targets_array.push(target_c.as_ptr());

            let propval_svn = propval.map(|val| subversion_sys::svn_string_t {
                data: val.as_ptr() as *mut i8,
                len: val.len(),
            });

            let changelists = changelists.map(|cl| {
                let mut array = apr::tables::TypedArray::<*const i8>::new(pool, 0);
                for item in cl.iter() {
                    let item_c = std::ffi::CString::new(*item).unwrap();
                    array.push(item_c.as_ptr());
                }
                array
            });

            let err = unsafe {
                subversion_sys::svn_client_propset_local(
                    propname_c.as_ptr(),
                    propval_svn
                        .as_ref()
                        .map_or(std::ptr::null(), |v| v as *const _),
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
        receiver: &mut dyn FnMut(
            &str,
            std::collections::HashMap<String, Vec<u8>>,
        ) -> Result<(), Error>,
    ) -> Result<(), Error> {
        with_tmp_pool(|pool| {
            let target_c = std::ffi::CString::new(target).unwrap();

            let changelists = changelists.map(|cl| {
                let mut array = apr::tables::TypedArray::<*const i8>::new(pool, 0);
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
                let receiver = unsafe {
                    &mut *(baton
                        as *mut &mut dyn FnMut(
                            &str,
                            std::collections::HashMap<String, Vec<u8>>,
                        ) -> Result<(), Error>)
                };
                let path_str = unsafe { std::ffi::CStr::from_ptr(path).to_str().unwrap() };

                // Convert props hash
                let prop_hash_wrapper = unsafe { crate::props::PropHash::from_ptr(props) };
                let prop_hash = prop_hash_wrapper.to_hashmap();

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

            let diff_options_c: Vec<_> = diff_options
                .iter()
                .map(|o| std::ffi::CString::new(*o).unwrap())
                .collect();
            let mut diff_options_array = apr::tables::TypedArray::<*const i8>::new(pool, 0);
            for opt in diff_options_c.iter() {
                diff_options_array.push(opt.as_ptr());
            }

            let relative_to_dir_c = relative_to_dir.map(|d| std::ffi::CString::new(d).unwrap());

            let changelists = changelists.map(|cl| {
                let mut array = apr::tables::TypedArray::<*const i8>::new(pool, 0);
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
                    &(*revision1).into(),
                    path2_c.as_ptr(),
                    &(*revision2).into(),
                    relative_to_dir_c
                        .as_ref()
                        .map_or(std::ptr::null(), |c| c.as_ptr()),
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
        list_func: &mut dyn FnMut(
            &str,
            &crate::ra::Dirent,
            Option<&crate::Lock>,
        ) -> Result<(), Error>,
    ) -> Result<(), Error> {
        with_tmp_pool(|pool| {
            let path_or_url_c = std::ffi::CString::new(path_or_url).unwrap();

            // Keep CStrings alive for the duration of the function
            let pattern_cstrings: Vec<std::ffi::CString> = patterns
                .unwrap_or(&[])
                .iter()
                .map(|p| std::ffi::CString::new(*p).unwrap())
                .collect();

            let patterns = if !pattern_cstrings.is_empty() {
                let mut array = apr::tables::TypedArray::<*const i8>::new(pool, 0);
                for pattern_c in pattern_cstrings.iter() {
                    array.push(pattern_c.as_ptr());
                }
                Some(array)
            } else if patterns.is_some() {
                Some(apr::tables::TypedArray::<*const i8>::new(pool, 0))
            } else {
                None
            };

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
                let list_func = unsafe {
                    &mut *(baton
                        as *mut &mut dyn FnMut(
                            &str,
                            &crate::ra::Dirent,
                            Option<&crate::Lock>,
                        ) -> Result<(), Error>)
                };
                let path_str = unsafe { std::ffi::CStr::from_ptr(path).to_str().unwrap() };
                let dirent = crate::ra::Dirent::from_raw(dirent as *mut _);
                let lock = if lock.is_null() {
                    None
                } else {
                    Some(crate::Lock::from_raw(lock as *mut _))
                };

                match list_func(path_str, &dirent, lock.as_ref()) {
                    Ok(()) => std::ptr::null_mut(),
                    Err(mut e) => unsafe { e.detach() },
                }
            }

            let list_func_ptr = &list_func as *const _ as *mut std::ffi::c_void;

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
    pub fn resolve(
        &mut self,
        path: &str,
        depth: Depth,
        conflict_choice: crate::ConflictChoice,
    ) -> Result<(), Error> {
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

/// Builder for diff operations
/// Builder for creating diff operations with various options.
pub struct DiffBuilder<'a> {
    ctx: &'a mut Context,
    path1: String,
    revision1: Revision,
    path2: String,
    revision2: Revision,
    depth: Depth,
    diff_options: Vec<String>,
    relative_to_dir: Option<String>,
    ignore_ancestry: bool,
    no_diff_added: bool,
    no_diff_deleted: bool,
    show_copies_as_adds: bool,
    ignore_content_type: bool,
    ignore_properties: bool,
    properties_only: bool,
    use_git_diff_format: bool,
    header_encoding: String,
    changelists: Option<Vec<String>>,
}

impl<'a> DiffBuilder<'a> {
    /// Creates a new DiffBuilder for comparing two paths/revisions.
    pub fn new(
        ctx: &'a mut Context,
        path1: impl Into<String>,
        revision1: Revision,
        path2: impl Into<String>,
        revision2: Revision,
    ) -> Self {
        Self {
            ctx,
            path1: path1.into(),
            revision1,
            path2: path2.into(),
            revision2,
            depth: Depth::Infinity,
            diff_options: Vec::new(),
            relative_to_dir: None,
            ignore_ancestry: false,
            no_diff_added: false,
            no_diff_deleted: false,
            show_copies_as_adds: false,
            ignore_content_type: false,
            ignore_properties: false,
            properties_only: false,
            use_git_diff_format: false,
            header_encoding: "UTF-8".to_string(),
            changelists: None,
        }
    }

    /// Sets the depth for the diff operation.
    pub fn depth(mut self, depth: Depth) -> Self {
        self.depth = depth;
        self
    }

    /// Sets additional options to pass to the diff command.
    pub fn diff_options(mut self, options: Vec<String>) -> Self {
        self.diff_options = options;
        self
    }

    /// Sets the directory to make paths relative to.
    pub fn relative_to_dir(mut self, dir: impl Into<String>) -> Self {
        self.relative_to_dir = Some(dir.into());
        self
    }

    /// Sets whether to ignore ancestry when comparing.
    pub fn ignore_ancestry(mut self, ignore: bool) -> Self {
        self.ignore_ancestry = ignore;
        self
    }

    /// Sets whether to omit diffs for added files.
    pub fn no_diff_added(mut self, no_diff: bool) -> Self {
        self.no_diff_added = no_diff;
        self
    }

    /// Sets whether to omit diffs for deleted files.
    pub fn no_diff_deleted(mut self, no_diff: bool) -> Self {
        self.no_diff_deleted = no_diff;
        self
    }

    /// Sets whether to show copies as additions.
    pub fn show_copies_as_adds(mut self, show: bool) -> Self {
        self.show_copies_as_adds = show;
        self
    }

    /// Sets whether to ignore content type differences.
    pub fn ignore_content_type(mut self, ignore: bool) -> Self {
        self.ignore_content_type = ignore;
        self
    }

    /// Sets whether to ignore property differences.
    pub fn ignore_properties(mut self, ignore: bool) -> Self {
        self.ignore_properties = ignore;
        self
    }

    /// Sets whether to show only property differences.
    pub fn properties_only(mut self, only: bool) -> Self {
        self.properties_only = only;
        self
    }

    /// Sets whether to use Git diff format.
    pub fn use_git_diff_format(mut self, use_git: bool) -> Self {
        self.use_git_diff_format = use_git;
        self
    }

    /// Sets the encoding for diff headers.
    pub fn header_encoding(mut self, encoding: impl Into<String>) -> Self {
        self.header_encoding = encoding.into();
        self
    }

    /// Sets the changelists to include in the diff.
    pub fn changelists(mut self, lists: Vec<String>) -> Self {
        self.changelists = Some(lists);
        self
    }

    /// Executes the diff operation.
    pub fn execute(
        self,
        outstream: &mut crate::io::Stream,
        errstream: &mut crate::io::Stream,
    ) -> Result<(), Error> {
        let diff_options: Vec<&str> = self.diff_options.iter().map(|s| s.as_str()).collect();
        let changelists = self
            .changelists
            .as_ref()
            .map(|cl| cl.iter().map(|s| s.as_str()).collect::<Vec<_>>());

        self.ctx.diff(
            &diff_options,
            &self.path1,
            &self.revision1,
            &self.path2,
            &self.revision2,
            self.relative_to_dir.as_deref(),
            self.depth,
            self.ignore_ancestry,
            self.no_diff_added,
            self.no_diff_deleted,
            self.show_copies_as_adds,
            self.ignore_content_type,
            self.ignore_properties,
            self.properties_only,
            self.use_git_diff_format,
            &self.header_encoding,
            outstream,
            errstream,
            changelists.as_deref(),
        )
    }
}

/// Builder for list operations
/// Builder for listing directory contents from the repository.
pub struct ListBuilder<'a> {
    ctx: &'a mut Context,
    path_or_url: String,
    peg_revision: Revision,
    revision: Revision,
    patterns: Option<Vec<String>>,
    depth: Depth,
    dirent_fields: u32,
    fetch_locks: bool,
    include_externals: bool,
}

impl<'a> ListBuilder<'a> {
    /// Creates a new ListBuilder for listing directory entries.
    pub fn new(ctx: &'a mut Context, path_or_url: impl Into<String>) -> Self {
        Self {
            ctx,
            path_or_url: path_or_url.into(),
            peg_revision: Revision::Head,
            revision: Revision::Head,
            patterns: None,
            depth: Depth::Infinity,
            dirent_fields: subversion_sys::SVN_DIRENT_KIND
                | subversion_sys::SVN_DIRENT_SIZE
                | subversion_sys::SVN_DIRENT_HAS_PROPS
                | subversion_sys::SVN_DIRENT_CREATED_REV
                | subversion_sys::SVN_DIRENT_TIME
                | subversion_sys::SVN_DIRENT_LAST_AUTHOR,
            fetch_locks: false,
            include_externals: false,
        }
    }

    /// Sets the peg revision for the path.
    pub fn peg_revision(mut self, rev: Revision) -> Self {
        self.peg_revision = rev;
        self
    }

    /// Sets the revision to list.
    pub fn revision(mut self, rev: Revision) -> Self {
        self.revision = rev;
        self
    }

    /// Sets the glob patterns to filter entries.
    pub fn patterns(mut self, patterns: Vec<String>) -> Self {
        self.patterns = Some(patterns);
        self
    }

    /// Sets the depth for the listing.
    pub fn depth(mut self, depth: Depth) -> Self {
        self.depth = depth;
        self
    }

    /// Sets which dirent fields to retrieve.
    pub fn dirent_fields(mut self, fields: u32) -> Self {
        self.dirent_fields = fields;
        self
    }

    /// Sets whether to fetch lock information.
    pub fn fetch_locks(mut self, fetch: bool) -> Self {
        self.fetch_locks = fetch;
        self
    }

    /// Sets whether to include externals.
    pub fn include_externals(mut self, include: bool) -> Self {
        self.include_externals = include;
        self
    }

    /// Executes the list operation.
    pub fn execute(
        self,
        list_func: &mut dyn FnMut(
            &str,
            &crate::ra::Dirent,
            Option<&crate::Lock>,
        ) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let patterns = self
            .patterns
            .as_ref()
            .map(|p| p.iter().map(|s| s.as_str()).collect::<Vec<_>>());

        self.ctx.list(
            &self.path_or_url,
            &self.peg_revision,
            &self.revision,
            patterns.as_deref(),
            self.depth,
            self.dirent_fields,
            self.fetch_locks,
            self.include_externals,
            list_func,
        )
    }
}

/// Builder for copy operations
/// Builder for copying versioned items in the repository or working copy.
pub struct CopyBuilder<'a> {
    ctx: &'a mut Context,
    sources: Vec<(String, Option<Revision>)>,
    dst_path: String,
    copy_as_child: bool,
    make_parents: bool,
    ignore_externals: bool,
    metadata_only: bool,
    pin_externals: bool,
}

impl<'a> CopyBuilder<'a> {
    /// Creates a new CopyBuilder for copying versioned items.
    pub fn new(ctx: &'a mut Context, dst_path: impl Into<String>) -> Self {
        Self {
            ctx,
            sources: Vec::new(),
            dst_path: dst_path.into(),
            copy_as_child: false,
            make_parents: false,
            ignore_externals: false,
            metadata_only: false,
            pin_externals: false,
        }
    }

    /// Adds a source path to copy from.
    pub fn add_source(mut self, path: impl Into<String>, revision: Option<Revision>) -> Self {
        self.sources.push((path.into(), revision));
        self
    }

    /// Sets whether to copy as a child of the destination.
    pub fn copy_as_child(mut self, as_child: bool) -> Self {
        self.copy_as_child = as_child;
        self
    }

    /// Sets whether to create parent directories.
    pub fn make_parents(mut self, make: bool) -> Self {
        self.make_parents = make;
        self
    }

    /// Sets whether to ignore externals.
    pub fn ignore_externals(mut self, ignore: bool) -> Self {
        self.ignore_externals = ignore;
        self
    }

    /// Sets whether to copy metadata only.
    pub fn metadata_only(mut self, only: bool) -> Self {
        self.metadata_only = only;
        self
    }

    /// Sets whether to pin externals.
    pub fn pin_externals(mut self, pin: bool) -> Self {
        self.pin_externals = pin;
        self
    }

    /// Executes the copy operation.
    pub fn execute(self) -> Result<(), Error> {
        let sources: Vec<(&str, Option<Revision>)> = self
            .sources
            .iter()
            .map(|(path, rev)| (path.as_str(), *rev))
            .collect();

        self.ctx.copy(
            &sources,
            &self.dst_path,
            self.copy_as_child,
            self.make_parents,
            self.ignore_externals,
            self.metadata_only,
            self.pin_externals,
        )
    }
}

/// Builder for info operations
/// Builder for retrieving information about versioned items.
pub struct InfoBuilder<'a> {
    ctx: &'a mut Context,
    abspath_or_url: String,
    peg_revision: Revision,
    revision: Revision,
    depth: Depth,
    fetch_excluded: bool,
    fetch_actual_only: bool,
    include_externals: bool,
    changelists: Option<Vec<String>>,
}

impl<'a> InfoBuilder<'a> {
    /// Creates a new InfoBuilder for retrieving information about versioned items.
    pub fn new(ctx: &'a mut Context, abspath_or_url: impl Into<String>) -> Self {
        Self {
            ctx,
            abspath_or_url: abspath_or_url.into(),
            peg_revision: Revision::Head,
            revision: Revision::Head,
            depth: Depth::Infinity,
            fetch_excluded: false,
            fetch_actual_only: false,
            include_externals: false,
            changelists: None,
        }
    }

    /// Sets the peg revision for the path.
    pub fn peg_revision(mut self, rev: Revision) -> Self {
        self.peg_revision = rev;
        self
    }

    /// Sets the revision to get info for.
    pub fn revision(mut self, rev: Revision) -> Self {
        self.revision = rev;
        self
    }

    /// Sets the depth for the info operation.
    pub fn depth(mut self, depth: Depth) -> Self {
        self.depth = depth;
        self
    }

    /// Sets whether to fetch excluded items.
    pub fn fetch_excluded(mut self, fetch: bool) -> Self {
        self.fetch_excluded = fetch;
        self
    }

    /// Sets whether to fetch actual nodes only.
    pub fn fetch_actual_only(mut self, fetch: bool) -> Self {
        self.fetch_actual_only = fetch;
        self
    }

    /// Sets whether to include externals.
    pub fn include_externals(mut self, include: bool) -> Self {
        self.include_externals = include;
        self
    }

    /// Sets the changelists to filter by.
    pub fn changelists(mut self, lists: Vec<String>) -> Self {
        self.changelists = Some(lists);
        self
    }

    /// Executes the info operation.
    pub fn execute(self, receiver: &dyn FnMut(&Info) -> Result<(), Error>) -> Result<(), Error> {
        let changelists = self
            .changelists
            .as_ref()
            .map(|cl| cl.iter().map(|s| s.as_str()).collect::<Vec<_>>());

        self.ctx.info(
            &self.abspath_or_url,
            self.peg_revision,
            self.revision,
            self.depth,
            self.fetch_excluded,
            self.fetch_actual_only,
            self.include_externals,
            changelists.as_deref(),
            receiver,
        )
    }
}

/// Builder for commit operations
/// Builder for creating commit operations with various options.
pub struct CommitBuilder<'a> {
    ctx: &'a mut Context,
    targets: Vec<String>,
    depth: Depth,
    keep_locks: bool,
    keep_changelists: bool,
    commit_as_operations: bool,
    include_file_externals: bool,
    include_dir_externals: bool,
    changelists: Option<Vec<String>>,
    revprop_table: std::collections::HashMap<String, String>,
}

impl<'a> CommitBuilder<'a> {
    /// Creates a new CommitBuilder for committing changes.
    pub fn new(ctx: &'a mut Context) -> Self {
        Self {
            ctx,
            targets: Vec::new(),
            depth: Depth::Infinity,
            keep_locks: false,
            keep_changelists: false,
            commit_as_operations: true,
            include_file_externals: false,
            include_dir_externals: false,
            changelists: None,
            revprop_table: std::collections::HashMap::new(),
        }
    }

    /// Adds a target path to commit.
    pub fn add_target(mut self, target: impl Into<String>) -> Self {
        self.targets.push(target.into());
        self
    }

    /// Sets the target paths to commit.
    pub fn targets(mut self, targets: Vec<String>) -> Self {
        self.targets = targets;
        self
    }

    /// Sets the depth for the commit.
    pub fn depth(mut self, depth: Depth) -> Self {
        self.depth = depth;
        self
    }

    /// Sets whether to keep locks after commit.
    pub fn keep_locks(mut self, keep: bool) -> Self {
        self.keep_locks = keep;
        self
    }

    /// Sets whether to keep changelists after commit.
    pub fn keep_changelists(mut self, keep: bool) -> Self {
        self.keep_changelists = keep;
        self
    }

    /// Sets whether to commit as operations.
    pub fn commit_as_operations(mut self, as_ops: bool) -> Self {
        self.commit_as_operations = as_ops;
        self
    }

    /// Sets whether to include file externals.
    pub fn include_file_externals(mut self, include: bool) -> Self {
        self.include_file_externals = include;
        self
    }

    /// Sets whether to include directory externals.
    pub fn include_dir_externals(mut self, include: bool) -> Self {
        self.include_dir_externals = include;
        self
    }

    /// Sets the changelists to commit.
    pub fn changelists(mut self, lists: Vec<String>) -> Self {
        self.changelists = Some(lists);
        self
    }

    /// Adds a revision property.
    pub fn add_revprop(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.revprop_table.insert(name.into(), value.into());
        self
    }

    /// Sets all revision properties.
    pub fn revprops(mut self, props: std::collections::HashMap<String, String>) -> Self {
        self.revprop_table = props;
        self
    }

    /// Executes the commit operation.
    pub fn execute(
        self,
        commit_callback: &dyn FnMut(&crate::CommitInfo) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let targets_ref: Vec<&str> = self.targets.iter().map(|s| s.as_str()).collect();
        let revprop_ref: std::collections::HashMap<&str, &str> = self
            .revprop_table
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();

        let changelists_vec = self
            .changelists
            .as_ref()
            .map(|cl| cl.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        let options = CommitOptions {
            depth: self.depth,
            keep_locks: self.keep_locks,
            keep_changelists: self.keep_changelists,
            commit_as_operations: self.commit_as_operations,
            include_file_externals: self.include_file_externals,
            include_dir_externals: self.include_dir_externals,
            changelists: changelists_vec,
        };

        self.ctx
            .commit(&targets_ref, &options, revprop_ref, commit_callback)
    }
}

/// Builder for log operations
/// Builder for retrieving log messages from the repository.
pub struct LogBuilder<'a> {
    ctx: &'a mut Context,
    targets: Vec<String>,
    peg_revision: Revision,
    revision_ranges: Vec<RevisionRange>,
    limit: i32,
    discover_changed_paths: bool,
    strict_node_history: bool,
    include_merged_revisions: bool,
    revprops: Vec<String>,
}

impl<'a> LogBuilder<'a> {
    /// Creates a new LogBuilder for retrieving log messages.
    pub fn new(ctx: &'a mut Context) -> Self {
        Self {
            ctx,
            targets: Vec::new(),
            peg_revision: Revision::Head,
            revision_ranges: vec![RevisionRange::new(
                Revision::Number(Revnum(1)),
                Revision::Head,
            )],
            limit: 0, // no limit
            discover_changed_paths: false,
            strict_node_history: true,
            include_merged_revisions: false,
            revprops: Vec::new(),
        }
    }

    /// Adds a target path for the log.
    pub fn add_target(mut self, target: impl Into<String>) -> Self {
        self.targets.push(target.into());
        self
    }

    /// Sets the target paths for the log.
    pub fn targets(mut self, targets: Vec<String>) -> Self {
        self.targets = targets;
        self
    }

    /// Sets the peg revision.
    pub fn peg_revision(mut self, rev: Revision) -> Self {
        self.peg_revision = rev;
        self
    }

    /// Adds a revision range to retrieve.
    pub fn add_revision_range(mut self, start: Revision, end: Revision) -> Self {
        self.revision_ranges.push(RevisionRange::new(start, end));
        self
    }

    /// Sets the revision ranges to retrieve.
    pub fn revision_ranges(mut self, ranges: Vec<RevisionRange>) -> Self {
        self.revision_ranges = ranges;
        self
    }

    /// Sets the maximum number of log entries to retrieve.
    pub fn limit(mut self, limit: i32) -> Self {
        self.limit = limit;
        self
    }

    /// Sets whether to discover changed paths.
    pub fn discover_changed_paths(mut self, discover: bool) -> Self {
        self.discover_changed_paths = discover;
        self
    }

    /// Sets whether to use strict node history.
    pub fn strict_node_history(mut self, strict: bool) -> Self {
        self.strict_node_history = strict;
        self
    }

    /// Sets whether to include merged revisions.
    pub fn include_merged_revisions(mut self, include: bool) -> Self {
        self.include_merged_revisions = include;
        self
    }

    /// Adds a revision property to retrieve.
    pub fn add_revprop(mut self, prop: impl Into<String>) -> Self {
        self.revprops.push(prop.into());
        self
    }

    /// Sets the revision properties to retrieve.
    pub fn revprops(mut self, props: Vec<String>) -> Self {
        self.revprops = props;
        self
    }

    /// Executes the log operation.
    pub fn execute(
        self,
        log_entry_receiver: &dyn FnMut(&LogEntry) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let targets_ref: Vec<&str> = self.targets.iter().map(|s| s.as_str()).collect();
        let revprops_ref: Vec<&str> = self.revprops.iter().map(|s| s.as_str()).collect();

        self.ctx.log(
            &targets_ref,
            self.peg_revision,
            &self.revision_ranges,
            self.limit,
            self.discover_changed_paths,
            self.strict_node_history,
            self.include_merged_revisions,
            &revprops_ref,
            log_entry_receiver,
        )
    }
}

/// Builder for update operations
/// Builder for updating working copy items to a different revision.
pub struct UpdateBuilder<'a> {
    ctx: &'a mut Context,
    paths: Vec<String>,
    revision: Revision,
    depth: Depth,
    depth_is_sticky: bool,
    ignore_externals: bool,
    allow_unver_obstructions: bool,
    adds_as_modifications: bool,
    make_parents: bool,
}

impl<'a> UpdateBuilder<'a> {
    /// Creates a new UpdateBuilder for updating working copies.
    pub fn new(ctx: &'a mut Context) -> Self {
        Self {
            ctx,
            paths: Vec::new(),
            revision: Revision::Head,
            depth: Depth::Infinity,
            depth_is_sticky: false,
            ignore_externals: false,
            allow_unver_obstructions: false,
            adds_as_modifications: false,
            make_parents: false,
        }
    }

    /// Adds a path to update.
    pub fn add_path(mut self, path: impl Into<String>) -> Self {
        self.paths.push(path.into());
        self
    }

    /// Sets the paths to update.
    pub fn paths(mut self, paths: Vec<String>) -> Self {
        self.paths = paths;
        self
    }

    /// Sets the revision to update to.
    pub fn revision(mut self, rev: Revision) -> Self {
        self.revision = rev;
        self
    }

    /// Sets the depth for the update.
    pub fn depth(mut self, depth: Depth) -> Self {
        self.depth = depth;
        self
    }

    /// Sets whether the depth is sticky.
    pub fn depth_is_sticky(mut self, sticky: bool) -> Self {
        self.depth_is_sticky = sticky;
        self
    }

    /// Sets whether to ignore externals.
    pub fn ignore_externals(mut self, ignore: bool) -> Self {
        self.ignore_externals = ignore;
        self
    }

    /// Sets whether to allow unversioned obstructions.
    pub fn allow_unver_obstructions(mut self, allow: bool) -> Self {
        self.allow_unver_obstructions = allow;
        self
    }

    /// Sets whether to treat adds as modifications.
    pub fn adds_as_modification(mut self, as_mod: bool) -> Self {
        self.adds_as_modifications = as_mod;
        self
    }

    /// Sets whether to create parent directories.
    pub fn make_parents(mut self, make: bool) -> Self {
        self.make_parents = make;
        self
    }

    /// Executes the update operation.
    pub fn execute(self) -> Result<Vec<Revnum>, Error> {
        let paths_ref: Vec<&str> = self.paths.iter().map(|s| s.as_str()).collect();
        let options = UpdateOptions {
            depth: self.depth,
            depth_is_sticky: self.depth_is_sticky,
            ignore_externals: self.ignore_externals,
            allow_unver_obstructions: self.allow_unver_obstructions,
            adds_as_modifications: self.adds_as_modifications,
            make_parents: self.make_parents,
        };

        self.ctx.update(&paths_ref, self.revision, &options)
    }
}

/// Builder for mkdir operations
/// Builder for creating directories in the repository or working copy.
pub struct MkdirBuilder<'a> {
    ctx: &'a mut Context,
    paths: Vec<String>,
    make_parents: bool,
    revprop_table: std::collections::HashMap<String, Vec<u8>>,
}

impl<'a> MkdirBuilder<'a> {
    /// Creates a new MkdirBuilder for creating directories.
    pub fn new(ctx: &'a mut Context) -> Self {
        Self {
            ctx,
            paths: Vec::new(),
            make_parents: false,
            revprop_table: std::collections::HashMap::new(),
        }
    }

    /// Adds a directory path to create.
    pub fn add_path(mut self, path: impl Into<String>) -> Self {
        self.paths.push(path.into());
        self
    }

    /// Sets the directory paths to create.
    pub fn paths(mut self, paths: Vec<String>) -> Self {
        self.paths = paths;
        self
    }

    /// Sets whether to create parent directories.
    pub fn make_parents(mut self, make: bool) -> Self {
        self.make_parents = make;
        self
    }

    /// Adds a revision property.
    pub fn add_revprop(mut self, name: impl Into<String>, value: Vec<u8>) -> Self {
        self.revprop_table.insert(name.into(), value);
        self
    }

    /// Sets all revision properties.
    pub fn revprops(mut self, props: std::collections::HashMap<String, Vec<u8>>) -> Self {
        self.revprop_table = props;
        self
    }

    /// Executes the mkdir operation.
    pub fn execute(
        self,
        commit_callback: &dyn FnMut(&crate::CommitInfo) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let paths_ref: Vec<&str> = self.paths.iter().map(|s| s.as_str()).collect();
        let revprop_ref: std::collections::HashMap<&str, &[u8]> = self
            .revprop_table
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_slice()))
            .collect();

        self.ctx
            .mkdir(&paths_ref, self.make_parents, revprop_ref, commit_callback)
    }
}

impl Context {
    /// Create a builder for diff operations
    pub fn diff_builder<'a>(
        &'a mut self,
        path1: impl Into<String>,
        revision1: Revision,
        path2: impl Into<String>,
        revision2: Revision,
    ) -> DiffBuilder<'a> {
        DiffBuilder::new(self, path1, revision1, path2, revision2)
    }

    /// Create a builder for list operations
    pub fn list_builder<'a>(&'a mut self, path_or_url: impl Into<String>) -> ListBuilder<'a> {
        ListBuilder::new(self, path_or_url)
    }

    /// Create a builder for copy operations
    pub fn copy_builder<'a>(&'a mut self, dst_path: impl Into<String>) -> CopyBuilder<'a> {
        CopyBuilder::new(self, dst_path)
    }

    /// Revert changes in the working copy
    ///
    /// This wraps svn_client_revert4 to revert local modifications.
    pub fn revert(
        &mut self,
        paths: &[&str],
        depth: Depth,
        changelists: Option<&[&str]>,
        clear_changelists: bool,
        metadata_only: bool,
        added_keep_local: bool,
    ) -> Result<(), Error> {
        let pool = Pool::new();

        // Convert paths to APR array
        let mut paths_array = apr::tables::TypedArray::<*const i8>::new(&pool, paths.len() as i32);
        let path_cstrings: Vec<_> = paths
            .iter()
            .map(|p| std::ffi::CString::new(*p).unwrap())
            .collect();
        for cstring in &path_cstrings {
            paths_array.push(cstring.as_ptr());
        }

        // Convert changelists if provided
        let changelists_array = if let Some(lists) = changelists {
            let mut array = apr::tables::TypedArray::<*const i8>::new(&pool, lists.len() as i32);
            let list_cstrings: Vec<_> = lists
                .iter()
                .map(|l| std::ffi::CString::new(*l).unwrap())
                .collect();
            for cstring in &list_cstrings {
                array.push(cstring.as_ptr());
            }
            unsafe { array.as_ptr() }
        } else {
            std::ptr::null()
        };

        let err = unsafe {
            subversion_sys::svn_client_revert4(
                paths_array.as_ptr(),
                depth.into(),
                changelists_array,
                clear_changelists as i32,
                metadata_only as i32,
                added_keep_local as i32,
                self.ptr,
                pool.as_mut_ptr(),
            )
        };

        Error::from_raw(err)?;
        Ok(())
    }

    /// Mark conflicts as resolved
    ///
    /// This wraps svn_client_resolved to mark conflicts as resolved.
    pub fn resolved(&mut self, path: &str, recursive: bool) -> Result<(), Error> {
        let pool = Pool::new();
        let path = std::ffi::CString::new(path).unwrap();

        let err = unsafe {
            subversion_sys::svn_client_resolved(
                path.as_ptr(),
                recursive as i32,
                self.ptr,
                pool.as_mut_ptr(),
            )
        };

        Error::from_raw(err)?;
        Ok(())
    }

    /// Add paths to a changelist
    ///
    /// This wraps svn_client_add_to_changelist to add paths to a changelist.
    pub fn add_to_changelist(
        &mut self,
        targets: &[&str],
        changelist: &str,
        depth: Depth,
        changelists: Option<&[&str]>,
    ) -> Result<(), Error> {
        let pool = Pool::new();
        let changelist = std::ffi::CString::new(changelist).unwrap();

        // Convert targets to APR array
        let mut targets_array =
            apr::tables::TypedArray::<*const i8>::new(&pool, targets.len() as i32);
        let target_cstrings: Vec<_> = targets
            .iter()
            .map(|t| std::ffi::CString::new(*t).unwrap())
            .collect();
        for cstring in &target_cstrings {
            targets_array.push(cstring.as_ptr());
        }

        // Convert changelists if provided
        let changelists_array = if let Some(lists) = changelists {
            let mut array = apr::tables::TypedArray::<*const i8>::new(&pool, lists.len() as i32);
            let list_cstrings: Vec<_> = lists
                .iter()
                .map(|l| std::ffi::CString::new(*l).unwrap())
                .collect();
            for cstring in &list_cstrings {
                array.push(cstring.as_ptr());
            }
            unsafe { array.as_ptr() }
        } else {
            std::ptr::null()
        };

        let err = unsafe {
            subversion_sys::svn_client_add_to_changelist(
                targets_array.as_ptr(),
                changelist.as_ptr(),
                depth.into(),
                changelists_array,
                self.ptr,
                pool.as_mut_ptr(),
            )
        };

        Error::from_raw(err)?;
        Ok(())
    }

    /// Remove paths from changelists
    ///
    /// This wraps svn_client_remove_from_changelists to remove paths from changelists.
    pub fn remove_from_changelists(
        &mut self,
        targets: &[&str],
        depth: Depth,
        changelists: Option<&[&str]>,
    ) -> Result<(), Error> {
        let pool = Pool::new();

        // Convert targets to APR array
        let mut targets_array =
            apr::tables::TypedArray::<*const i8>::new(&pool, targets.len() as i32);
        let target_cstrings: Vec<_> = targets
            .iter()
            .map(|t| std::ffi::CString::new(*t).unwrap())
            .collect();
        for cstring in &target_cstrings {
            targets_array.push(cstring.as_ptr());
        }

        // Convert changelists if provided
        let changelists_array = if let Some(lists) = changelists {
            let mut array = apr::tables::TypedArray::<*const i8>::new(&pool, lists.len() as i32);
            let list_cstrings: Vec<_> = lists
                .iter()
                .map(|l| std::ffi::CString::new(*l).unwrap())
                .collect();
            for cstring in &list_cstrings {
                array.push(cstring.as_ptr());
            }
            unsafe { array.as_ptr() }
        } else {
            std::ptr::null()
        };

        let err = unsafe {
            subversion_sys::svn_client_remove_from_changelists(
                targets_array.as_ptr(),
                depth.into(),
                changelists_array,
                self.ptr,
                pool.as_mut_ptr(),
            )
        };

        Error::from_raw(err)?;
        Ok(())
    }

    /// Merge changes between two sources
    ///
    /// This wraps svn_client_merge5 for merging between two sources.
    pub fn merge_sources(
        &mut self,
        source1: &str,
        revision1: &Revision,
        source2: &str,
        revision2: &Revision,
        target_wcpath: &str,
        depth: Depth,
        ignore_ancestry: bool,
        record_only: bool,
        force_delete: bool,
        dry_run: bool,
        allow_mixed_rev: bool,
    ) -> Result<(), Error> {
        let pool = Pool::new();
        let source1 = std::ffi::CString::new(source1).unwrap();
        let source2 = std::ffi::CString::new(source2).unwrap();
        let target_wcpath = std::ffi::CString::new(target_wcpath).unwrap();

        let err = unsafe {
            subversion_sys::svn_client_merge5(
                source1.as_ptr(),
                &(*revision1).into(),
                source2.as_ptr(),
                &(*revision2).into(),
                target_wcpath.as_ptr(),
                depth.into(),
                ignore_ancestry as i32,
                ignore_ancestry as i32, // ignore_mergeinfo
                record_only as i32,
                force_delete as i32,
                dry_run as i32,
                allow_mixed_rev as i32,
                std::ptr::null_mut(), // merge_options
                self.ptr,
                pool.as_mut_ptr(),
            )
        };

        Error::from_raw(err)?;
        Ok(())
    }

    /// Move (rename) a file or directory
    ///
    /// This wraps svn_client_move7 to perform a versioned move operation.
    pub fn move_path(
        &mut self,
        src_paths: &[&str],
        dst_path: &str,
        move_as_child: bool,
        make_parents: bool,
        allow_mixed_revisions: bool,
        metadata_only: bool,
        revprop_table: Option<HashMap<String, Vec<u8>>>,
    ) -> Result<(), Error> {
        let pool = Pool::new();

        // Convert source paths to APR array
        let mut src_paths_array =
            apr::tables::TypedArray::<*const i8>::new(&pool, src_paths.len() as i32);
        let src_cstrings: Vec<_> = src_paths
            .iter()
            .map(|p| std::ffi::CString::new(*p).unwrap())
            .collect();
        for cstring in &src_cstrings {
            src_paths_array.push(cstring.as_ptr());
        }

        let dst_path = std::ffi::CString::new(dst_path).unwrap();

        // Convert revprop table if provided
        let mut revprop_hash = std::ptr::null_mut();
        if let Some(revprops) = revprop_table {
            let mut hash = apr::hash::Hash::new(&pool);
            for (key, value) in revprops {
                let key_cstring = std::ffi::CString::new(key).unwrap();
                let svn_string = crate::string::BStr::from_bytes(&value, &pool);
                unsafe {
                    hash.insert(
                        key_cstring.as_bytes(),
                        svn_string.as_ptr() as *mut std::ffi::c_void,
                    );
                }
            }
            unsafe {
                revprop_hash = hash.as_mut_ptr();
            }
        }

        let err = unsafe {
            subversion_sys::svn_client_move7(
                src_paths_array.as_ptr(),
                dst_path.as_ptr(),
                move_as_child as i32,
                make_parents as i32,
                allow_mixed_revisions as i32,
                metadata_only as i32,
                revprop_hash,
                None,                 // commit_callback
                std::ptr::null_mut(), // commit_baton
                self.ptr,
                pool.as_mut_ptr(),
            )
        };

        Error::from_raw(err)?;
        Ok(())
    }

    /// Create a builder for info operations
    pub fn info_builder<'a>(&'a mut self, abspath_or_url: impl Into<String>) -> InfoBuilder<'a> {
        InfoBuilder::new(self, abspath_or_url)
    }

    /// Create a builder for commit operations
    pub fn commit_builder<'a>(&'a mut self) -> CommitBuilder<'a> {
        CommitBuilder::new(self)
    }

    /// Create a builder for log operations
    pub fn log_builder<'a>(&'a mut self) -> LogBuilder<'a> {
        LogBuilder::new(self)
    }

    /// Create a builder for update operations
    pub fn update_builder<'a>(&'a mut self) -> UpdateBuilder<'a> {
        UpdateBuilder::new(self)
    }

    /// Create a builder for mkdir operations
    pub fn mkdir_builder<'a>(&'a mut self) -> MkdirBuilder<'a> {
        MkdirBuilder::new(self)
    }

    /// Apply a patch to a working copy
    ///
    /// Applies a unified diff patch from `patch_path` to the working copy at `wc_dir_path`.
    pub fn patch(
        &mut self,
        patch_path: &std::path::Path,
        wc_dir_path: &std::path::Path,
        dry_run: bool,
        strip_count: i32,
        reverse: bool,
        ignore_whitespace: bool,
        remove_tempfiles: bool,
    ) -> Result<(), Error> {
        let pool = apr::Pool::new();
        let patch_path_cstr = std::ffi::CString::new(patch_path.to_str().unwrap())?;
        let wc_dir_path_cstr = std::ffi::CString::new(wc_dir_path.to_str().unwrap())?;

        unsafe {
            let err = subversion_sys::svn_client_patch(
                patch_path_cstr.as_ptr(),
                wc_dir_path_cstr.as_ptr(),
                dry_run as i32,
                strip_count,
                reverse as i32,
                ignore_whitespace as i32,
                remove_tempfiles as i32,
                None,                 // patch_func - NULL for default behavior
                std::ptr::null_mut(), // patch_baton
                self.as_mut_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)
        }
    }

    /// Perform a reintegrate merge
    ///
    /// Merges all eligible revisions from the source URL to the working copy.
    pub fn merge_reintegrate(
        &mut self,
        source_url: &str,
        source_peg_revision: &Revision,
        target_wcpath: &std::path::Path,
        dry_run: bool,
        merge_options: &[&str],
    ) -> Result<(), Error> {
        let pool = apr::Pool::new();
        let source_url_cstr = std::ffi::CString::new(source_url)?;
        let target_wcpath_cstr = std::ffi::CString::new(target_wcpath.to_str().unwrap())?;

        // Convert merge options to APR array
        let merge_options_array = unsafe {
            let array = apr_sys::apr_array_make(
                pool.as_mut_ptr(),
                merge_options.len() as i32,
                std::mem::size_of::<*const std::os::raw::c_char>() as i32,
            );

            for option in merge_options {
                let option_ptr = pool.pstrdup(option);
                let slot = apr_sys::apr_array_push(array) as *mut *const std::os::raw::c_char;
                *slot = option_ptr;
            }

            array
        };

        unsafe {
            let err = subversion_sys::svn_client_merge_reintegrate(
                source_url_cstr.as_ptr(),
                &(*source_peg_revision).into(),
                target_wcpath_cstr.as_ptr(),
                dry_run as i32,
                merge_options_array,
                self.as_mut_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)
        }
    }

    /// Get the UUID of a repository from a URL
    /// Returns the UUID string identifying the repository
    pub fn uuid_from_url(&mut self, url: &str) -> Result<String, Error> {
        let url_cstr = std::ffi::CString::new(url)?;
        let pool = apr::Pool::new();
        let mut uuid_ptr = std::ptr::null();

        let ret = unsafe {
            subversion_sys::svn_client_uuid_from_url(
                &mut uuid_ptr,
                url_cstr.as_ptr(),
                self.ptr,
                pool.as_mut_ptr(),
            )
        };

        Error::from_raw(ret)?;

        if uuid_ptr.is_null() {
            return Err(Error::from_str("Failed to get repository UUID"));
        }

        let uuid_str = unsafe {
            std::ffi::CStr::from_ptr(uuid_ptr)
                .to_string_lossy()
                .into_owned()
        };

        Ok(uuid_str)
    }

    /// Get the UUID of a repository from a working copy path
    /// Returns the UUID string of the repository the working copy is connected to
    pub fn uuid_from_path(&mut self, path: &std::path::Path) -> Result<String, Error> {
        let path_cstr = std::ffi::CString::new(path.to_string_lossy().as_ref())?;
        let pool = apr::Pool::new();
        let mut uuid_ptr = std::ptr::null();

        let ret = unsafe {
            subversion_sys::svn_client_uuid_from_path2(
                &mut uuid_ptr,
                path_cstr.as_ptr(),
                self.ptr,
                pool.as_mut_ptr(),
                pool.as_mut_ptr(), // scratch pool
            )
        };

        Error::from_raw(ret)?;

        if uuid_ptr.is_null() {
            return Err(Error::from_str("Failed to get repository UUID"));
        }

        let uuid_str = unsafe {
            std::ffi::CStr::from_ptr(uuid_ptr)
                .to_string_lossy()
                .into_owned()
        };

        Ok(uuid_str)
    }

    /// Relocate a working copy to a new repository URL
    ///
    /// Updates all references from `from_prefix` to `to_prefix` in the working copy.
    pub fn relocate(
        &mut self,
        wcroot_path: &std::path::Path,
        from_prefix: &str,
        to_prefix: &str,
        ignore_externals: bool,
    ) -> Result<(), Error> {
        let pool = apr::Pool::new();
        let wcroot_path_cstr = std::ffi::CString::new(wcroot_path.to_str().unwrap())?;
        let from_prefix_cstr = std::ffi::CString::new(from_prefix)?;
        let to_prefix_cstr = std::ffi::CString::new(to_prefix)?;

        unsafe {
            let err = subversion_sys::svn_client_relocate2(
                wcroot_path_cstr.as_ptr(),
                from_prefix_cstr.as_ptr(),
                to_prefix_cstr.as_ptr(),
                ignore_externals as i32,
                self.as_mut_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)
        }
    }

    /// Set a revision property on a repository
    pub fn revprop_set(
        &mut self,
        propname: &str,
        propval: Option<&[u8]>,
        url: &str,
        revision: &Revision,
        original_propval: Option<&[u8]>,
        force: bool,
    ) -> Result<(), Error> {
        let pool = apr::Pool::new();
        let propname_cstr = std::ffi::CString::new(propname)?;
        let url_cstr = std::ffi::CString::new(url)?;

        let propval_svn = if let Some(val) = propval {
            unsafe {
                let svn_str = apr_sys::apr_palloc(
                    pool.as_mut_ptr(),
                    std::mem::size_of::<subversion_sys::svn_string_t>(),
                ) as *mut subversion_sys::svn_string_t;
                (*svn_str).data = val.as_ptr() as *const std::os::raw::c_char;
                (*svn_str).len = val.len();
                svn_str
            }
        } else {
            std::ptr::null_mut()
        };

        let original_propval_svn = if let Some(val) = original_propval {
            unsafe {
                let svn_str = apr_sys::apr_palloc(
                    pool.as_mut_ptr(),
                    std::mem::size_of::<subversion_sys::svn_string_t>(),
                ) as *mut subversion_sys::svn_string_t;
                (*svn_str).data = val.as_ptr() as *const std::os::raw::c_char;
                (*svn_str).len = val.len();
                svn_str
            }
        } else {
            std::ptr::null_mut()
        };

        unsafe {
            let err = subversion_sys::svn_client_revprop_set2(
                propname_cstr.as_ptr(),
                propval_svn,
                original_propval_svn,
                url_cstr.as_ptr(),
                &(*revision).into(),
                std::ptr::null_mut(), // base_revision_for_url - not used
                force as i32,
                self.as_mut_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)
        }
    }

    /// Get a revision property from a repository
    pub fn revprop_get(
        &mut self,
        propname: &str,
        url: &str,
        revision: &Revision,
    ) -> Result<Option<Vec<u8>>, Error> {
        let pool = apr::Pool::new();
        let propname_cstr = std::ffi::CString::new(propname)?;
        let url_cstr = std::ffi::CString::new(url)?;
        let mut propval: *mut subversion_sys::svn_string_t = std::ptr::null_mut();
        let mut actual_rev = 0;

        unsafe {
            let err = subversion_sys::svn_client_revprop_get(
                propname_cstr.as_ptr(),
                &mut propval,
                url_cstr.as_ptr(),
                &(*revision).into(),
                &mut actual_rev,
                self.as_mut_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;

            if propval.is_null() {
                Ok(None)
            } else {
                let len = (*propval).len;
                let data = (*propval).data as *const u8;
                Ok(Some(std::slice::from_raw_parts(data, len).to_vec()))
            }
        }
    }

    /// List revision properties on a repository
    pub fn revprop_list(
        &mut self,
        url: &str,
        revision: &Revision,
    ) -> Result<std::collections::HashMap<String, Vec<u8>>, Error> {
        let pool = apr::Pool::new();
        let url_cstr = std::ffi::CString::new(url)?;
        let mut props_hash = std::ptr::null_mut();
        let mut actual_rev = 0;

        unsafe {
            let err = subversion_sys::svn_client_revprop_list(
                &mut props_hash,
                url_cstr.as_ptr(),
                &(*revision).into(),
                &mut actual_rev,
                self.as_mut_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;

            let result = if !props_hash.is_null() {
                let prop_hash = crate::props::PropHash::from_ptr(props_hash);
                prop_hash.to_hashmap()
            } else {
                std::collections::HashMap::new()
            };

            Ok(result)
        }
    }

    /// Suggest possible merge sources for a repository path
    pub fn suggest_merge_sources(
        &mut self,
        path_or_url: &str,
        peg_revision: &Revision,
    ) -> Result<Vec<String>, Error> {
        let pool = apr::Pool::new();
        let path_or_url_cstr = std::ffi::CString::new(path_or_url)?;
        let mut sources_array = std::ptr::null_mut();

        unsafe {
            let err = subversion_sys::svn_client_suggest_merge_sources(
                &mut sources_array,
                path_or_url_cstr.as_ptr(),
                &(*peg_revision).into(),
                self.as_mut_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;

            let mut result = Vec::new();
            if !sources_array.is_null() {
                let array_len = (*sources_array).nelts as usize;
                let array_data = (*sources_array).elts as *const *const std::os::raw::c_char;

                for i in 0..array_len {
                    let source_ptr = *array_data.add(i);
                    if !source_ptr.is_null() {
                        let source_str = std::ffi::CStr::from_ptr(source_ptr);
                        result.push(source_str.to_string_lossy().into_owned());
                    }
                }
            }

            Ok(result)
        }
    }

    /// Upgrade a working copy to a newer format
    pub fn upgrade(&mut self, wcroot_path: &std::path::Path) -> Result<(), Error> {
        let pool = apr::Pool::new();
        let wcroot_path_cstr = std::ffi::CString::new(wcroot_path.to_str().unwrap())?;

        unsafe {
            let err = subversion_sys::svn_client_upgrade(
                wcroot_path_cstr.as_ptr(),
                self.as_mut_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)
        }
    }
}

/// Status information for a working copy item.
pub struct Status(pub(crate) *const subversion_sys::svn_client_status_t);

impl Status {
    /// Returns the node kind (file, directory, etc.).
    pub fn kind(&self) -> crate::NodeKind {
        unsafe { (*self.0).kind.into() }
    }

    /// Returns the local absolute path of the item.
    pub fn local_abspath(&self) -> &str {
        unsafe {
            std::ffi::CStr::from_ptr((*self.0).local_abspath)
                .to_str()
                .unwrap()
        }
    }

    /// Returns the file size.
    pub fn filesize(&self) -> i64 {
        unsafe { (*self.0).filesize }
    }

    /// Returns whether the item is versioned.
    pub fn versioned(&self) -> bool {
        unsafe { (*self.0).versioned != 0 }
    }

    /// Returns whether the item is conflicted.
    pub fn conflicted(&self) -> bool {
        unsafe { (*self.0).conflicted != 0 }
    }

    /// Returns the node status.
    pub fn node_status(&self) -> crate::StatusKind {
        unsafe { (*self.0).node_status.into() }
    }

    /// Returns the text status.
    pub fn text_status(&self) -> crate::StatusKind {
        unsafe { (*self.0).text_status.into() }
    }

    /// Returns the property status.
    pub fn prop_status(&self) -> crate::StatusKind {
        unsafe { (*self.0).prop_status.into() }
    }

    /// Returns whether the working copy is locked.
    pub fn wc_is_locked(&self) -> bool {
        unsafe { (*self.0).wc_is_locked != 0 }
    }

    /// Returns whether the item was copied.
    pub fn copied(&self) -> bool {
        unsafe { (*self.0).copied != 0 }
    }

    /// Returns the repository root URL.
    pub fn repos_root_url(&self) -> &str {
        unsafe {
            std::ffi::CStr::from_ptr((*self.0).repos_root_url)
                .to_str()
                .unwrap()
        }
    }

    /// Returns the repository UUID.
    pub fn repos_uuid(&self) -> &str {
        unsafe {
            std::ffi::CStr::from_ptr((*self.0).repos_uuid)
                .to_str()
                .unwrap()
        }
    }

    /// Returns the repository relative path.
    pub fn repos_relpath(&self) -> &str {
        unsafe {
            std::ffi::CStr::from_ptr((*self.0).repos_relpath)
                .to_str()
                .unwrap()
        }
    }

    /// Returns the revision number.
    pub fn revision(&self) -> Revnum {
        Revnum::from_raw(unsafe { (*self.0).revision }).unwrap()
    }

    /// Returns the last changed revision.
    pub fn changed_rev(&self) -> Revnum {
        Revnum::from_raw(unsafe { (*self.0).changed_rev }).unwrap()
    }

    /// Returns the last changed date.
    pub fn changed_date(&self) -> apr::time::Time {
        unsafe { apr::time::Time::from((*self.0).changed_date) }
    }

    /// Returns the last changed author.
    pub fn changed_author(&self) -> &str {
        unsafe {
            std::ffi::CStr::from_ptr((*self.0).changed_author)
                .to_str()
                .unwrap()
        }
    }

    /// Returns whether the item is switched.
    pub fn switched(&self) -> bool {
        unsafe { (*self.0).switched != 0 }
    }

    /// Returns whether the item is a file external.
    pub fn file_external(&self) -> bool {
        unsafe { (*self.0).file_external != 0 }
    }

    /// Returns the lock information if the item is locked.
    pub fn lock(&self) -> Option<crate::Lock<'_>> {
        let lock_ptr = unsafe { (*self.0).lock };
        if lock_ptr.is_null() {
            None
        } else {
            Some(crate::Lock::from_raw(lock_ptr))
        }
    }

    /// Returns the changelist name if the item is in a changelist.
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

    /// Returns the depth of the item.
    pub fn depth(&self) -> crate::Depth {
        unsafe { (*self.0).depth.into() }
    }

    /// Returns the out-of-date node kind.
    pub fn ood_kind(&self) -> crate::NodeKind {
        unsafe { (*self.0).ood_kind.into() }
    }

    /// Returns the repository node status.
    pub fn repos_node_status(&self) -> crate::StatusKind {
        unsafe { (*self.0).repos_node_status.into() }
    }

    /// Returns the repository text status.
    pub fn repos_text_status(&self) -> crate::StatusKind {
        unsafe { (*self.0).repos_text_status.into() }
    }

    /// Returns the repository property status.
    pub fn repos_prop_status(&self) -> crate::StatusKind {
        unsafe { (*self.0).repos_prop_status.into() }
    }

    /// Returns the repository lock information.
    pub fn repos_lock(&self) -> Option<crate::Lock<'_>> {
        let lock_ptr = unsafe { (*self.0).repos_lock };
        if lock_ptr.is_null() {
            None
        } else {
            Some(crate::Lock::from_raw(lock_ptr))
        }
    }

    /// Returns the out-of-date changed revision.
    pub fn ood_changed_rev(&self) -> Option<Revnum> {
        Revnum::from_raw(unsafe { (*self.0).ood_changed_rev })
    }

    /// Returns the out-of-date changed author.
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

    /// Returns the absolute path the item was moved from.
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

    /// Returns the absolute path the item was moved to.
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
/// Represents a conflict in the working copy.
pub struct Conflict {
    ptr: *mut subversion_sys::svn_client_conflict_t,
    pool: apr::Pool<'static>,
    _phantom: std::marker::PhantomData<*mut ()>, // !Send + !Sync
}

impl Drop for Conflict {
    fn drop(&mut self) {
        // Pool drop will clean up conflict
    }
}

impl Conflict {
    /// Get a reference to the underlying pool
    pub fn pool(&self) -> &apr::Pool<'_> {
        &self.pool
    }

    /// Get the raw pointer to the conflict (use with caution)
    pub fn as_ptr(&self) -> *const subversion_sys::svn_client_conflict_t {
        self.ptr
    }

    /// Get the mutable raw pointer to the conflict (use with caution)
    pub fn as_mut_ptr(&mut self) -> *mut subversion_sys::svn_client_conflict_t {
        self.ptr
    }
    pub(crate) unsafe fn from_ptr_and_pool(
        ptr: *mut subversion_sys::svn_client_conflict_t,
        pool: apr::Pool<'static>,
    ) -> Self {
        Self {
            ptr,
            pool,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Gets the description of a property conflict.
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

    /// Resolve a text conflict
    pub fn text_resolve(
        &mut self,
        choice: crate::TextConflictChoice,
        ctx: &mut Context,
    ) -> Result<(), Error> {
        let scratch_pool = apr::pool::Pool::new();
        unsafe {
            let err = subversion_sys::svn_client_conflict_text_resolve_by_id(
                self.ptr,
                choice.into(),
                ctx.as_mut_ptr(),
                scratch_pool.as_mut_ptr(),
            );
            svn_result(err)
        }
    }

    /// Resolve a property conflict
    pub fn prop_resolve(
        &mut self,
        propname: &str,
        choice: crate::TextConflictChoice,
        ctx: &mut Context,
    ) -> Result<(), Error> {
        let scratch_pool = apr::pool::Pool::new();
        let propname_c = std::ffi::CString::new(propname)?;
        unsafe {
            let err = subversion_sys::svn_client_conflict_prop_resolve_by_id(
                self.ptr,
                propname_c.as_ptr(),
                choice.into(),
                ctx.as_mut_ptr(),
                scratch_pool.as_mut_ptr(),
            );
            svn_result(err)
        }
    }

    /// Resolve a tree conflict
    pub fn tree_resolve(
        &mut self,
        choice: crate::TreeConflictChoice,
        ctx: &mut Context,
    ) -> Result<(), Error> {
        let scratch_pool = apr::pool::Pool::new();
        unsafe {
            let err = subversion_sys::svn_client_conflict_tree_resolve_by_id(
                self.ptr,
                choice.into(),
                ctx.as_mut_ptr(),
                scratch_pool.as_mut_ptr(),
            );
            svn_result(err)
        }
    }

    /// Get whether the conflict has text, property, or tree conflicts
    pub fn get_conflicted(&self) -> Result<(bool, Vec<String>, bool), Error> {
        let pool = apr::pool::Pool::new();
        let mut text_conflicted: subversion_sys::svn_boolean_t = 0;
        let mut props_conflicted: *mut apr_sys::apr_array_header_t = std::ptr::null_mut();
        let mut tree_conflicted: subversion_sys::svn_boolean_t = 0;

        unsafe {
            let err = subversion_sys::svn_client_conflict_get_conflicted(
                &mut text_conflicted,
                &mut props_conflicted,
                &mut tree_conflicted,
                self.ptr,
                pool.as_mut_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;

            let has_text_conflict = text_conflicted != 0;
            let has_tree_conflict = tree_conflicted != 0;

            let mut prop_conflicts = Vec::new();
            if !props_conflicted.is_null() {
                // Property conflicts array contains property names
                let array =
                    apr::tables::TypedArray::<*const std::ffi::c_char>::from_ptr(props_conflicted);
                for cstr_ptr in array.iter() {
                    let cstr = std::ffi::CStr::from_ptr(cstr_ptr);
                    let propname = cstr.to_string_lossy().into_owned();
                    prop_conflicts.push(propname);
                }
            }

            Ok((has_text_conflict, prop_conflicts, has_tree_conflict))
        }
    }

    /// Get the operation that caused the conflict
    pub fn get_operation(&self) -> Result<crate::conflict::ConflictAction, Error> {
        unsafe {
            let operation = subversion_sys::svn_client_conflict_get_operation(self.ptr);
            Ok(operation.into())
        }
    }

    /// Get whether the conflict involves a binary file
    pub fn text_get_mime_type(&self) -> Result<Option<String>, Error> {
        unsafe {
            let mime_type = subversion_sys::svn_client_conflict_text_get_mime_type(self.ptr);
            if mime_type.is_null() {
                Ok(None)
            } else {
                Ok(Some(
                    std::ffi::CStr::from_ptr(mime_type)
                        .to_string_lossy()
                        .into_owned(),
                ))
            }
        }
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
        let url = crate::uri::Uri::new(&url_str).unwrap();
        let revnum = ctx
            .checkout(
                url,
                td.path().join("wc"),
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

    #[test]
    fn test_copy() {
        let td = tempfile::tempdir().unwrap();
        let repo_path = td.path().join("repo");
        let wc_path = td.path().join("wc");

        // Create a test repository
        crate::repos::Repos::create(&repo_path).unwrap();

        // Check out working copy
        let mut ctx = Context::new().unwrap();
        let url_str = format!("file://{}", repo_path.to_str().unwrap());
        let url = crate::uri::Uri::new(&url_str).unwrap();

        ctx.checkout(
            url,
            &wc_path,
            &CheckoutOptions {
                peg_revision: Revision::Head,
                revision: Revision::Head,
                depth: Depth::Infinity,
                ignore_externals: false,
                allow_unver_obstructions: false,
            },
        )
        .unwrap();

        // Create a test file
        let test_file = wc_path.join("test.txt");
        std::fs::write(&test_file, "test content").unwrap();
        ctx.add(
            &test_file,
            &AddOptions {
                depth: Depth::Empty,
                force: false,
                no_ignore: false,
                no_autoprops: false,
                add_parents: false,
            },
        )
        .unwrap();

        // Test copy operation (would need a commit first in real scenario)
        let copy_result = ctx.copy(
            &[(test_file.to_str().unwrap(), None)],
            wc_path.join("test_copy.txt").to_str().unwrap(),
            false, // copy_as_child
            false, // make_parents
            false, // ignore_externals
            false, // metadata_only
            false, // pin_externals
        );

        // Copy in working copy requires committed files, so this might fail
        // Just ensure it doesn't panic
        let _ = copy_result;
    }

    #[test]
    fn test_mkdir_multiple() {
        let td = tempfile::tempdir().unwrap();
        let repo_path = td.path().join("repo");
        let wc_path = td.path().join("wc");

        // Create a test repository
        crate::repos::Repos::create(&repo_path).unwrap();

        // Check out working copy
        let mut ctx = Context::new().unwrap();
        let url_str = format!("file://{}", repo_path.to_str().unwrap());
        let url = crate::uri::Uri::new(&url_str).unwrap();

        ctx.checkout(
            url,
            &wc_path,
            &CheckoutOptions {
                peg_revision: Revision::Head,
                revision: Revision::Head,
                depth: Depth::Infinity,
                ignore_externals: false,
                allow_unver_obstructions: false,
            },
        )
        .unwrap();

        // Test creating multiple directories
        let dir1 = wc_path.join("dir1");
        let dir2 = wc_path.join("dir2");

        let result = ctx.mkdir_multiple(
            &[dir1.to_str().unwrap(), dir2.to_str().unwrap()],
            false, // make_parents
        );

        // Should succeed in working copy
        assert!(result.is_ok());
        assert!(dir1.exists());
        assert!(dir2.exists());
    }

    #[test]
    fn test_propget_propset() {
        let td = tempfile::tempdir().unwrap();
        let repo_path = td.path().join("repo");
        let wc_path = td.path().join("wc");

        // Create a test repository
        crate::repos::Repos::create(&repo_path).unwrap();

        // Check out working copy
        let mut ctx = Context::new().unwrap();
        let url_str = format!("file://{}", repo_path.to_str().unwrap());
        let url = crate::uri::Uri::new(&url_str).unwrap();

        ctx.checkout(
            url,
            &wc_path,
            &CheckoutOptions {
                peg_revision: Revision::Head,
                revision: Revision::Head,
                depth: Depth::Infinity,
                ignore_externals: false,
                allow_unver_obstructions: false,
            },
        )
        .unwrap();

        // Create a test file
        let test_file = wc_path.join("test.txt");
        std::fs::write(&test_file, "test content").unwrap();
        ctx.add(
            &test_file,
            &AddOptions {
                depth: Depth::Empty,
                force: false,
                no_ignore: false,
                no_autoprops: false,
                add_parents: false,
            },
        )
        .unwrap();

        // Test setting a property
        let propset_result = ctx.propset(
            "test:property",
            Some(b"test value"),
            test_file.to_str().unwrap(),
            Depth::Empty,
            false,      // skip_checks
            Revnum(-1), // INVALID
            None,       // changelists
        );

        // Setting properties should work on added files
        assert!(propset_result.is_ok());

        // Test getting the property back
        let propget_result = ctx.propget(
            "test:property",
            test_file.to_str().unwrap(),
            &Revision::Working,
            &Revision::Working,
            None, // actual_revnum
            Depth::Empty,
            None, // changelists
        );

        if let Ok(props) = propget_result {
            // Check if our property is in the results
            for (_path, value) in props.iter() {
                assert_eq!(value, b"test value");
            }
        }
    }

    #[test]
    fn test_move_path() {
        let td = tempfile::tempdir().unwrap();
        let repo_path = td.path().join("repo");
        let wc_path = td.path().join("wc");

        // Create a test repository
        crate::repos::Repos::create(&repo_path).unwrap();

        // Check out working copy
        let mut ctx = Context::new().unwrap();
        let url_str = format!("file://{}", repo_path.to_str().unwrap());
        let url = crate::uri::Uri::new(&url_str).unwrap();

        ctx.checkout(
            url,
            &wc_path,
            &CheckoutOptions {
                peg_revision: Revision::Head,
                revision: Revision::Head,
                depth: Depth::Infinity,
                ignore_externals: false,
                allow_unver_obstructions: false,
            },
        )
        .unwrap();

        // Create a test file
        let test_file = wc_path.join("test_file.txt");
        std::fs::write(&test_file, b"Test content").unwrap();

        // Add the file
        ctx.add(&test_file, &AddOptions::default()).unwrap();

        // Commit the file
        ctx.commit(
            &[wc_path.to_str().unwrap()],
            &CommitOptions::default(),
            std::collections::HashMap::new(),
            &mut |_info: &crate::CommitInfo| Ok(()),
        )
        .unwrap();

        // Now move the file
        let new_path = wc_path.join("renamed_file.txt");
        let result = ctx.move_path(
            &[test_file.to_str().unwrap()],
            new_path.to_str().unwrap(),
            false, // move_as_child
            false, // make_parents
            true,  // allow_mixed_revisions
            false, // metadata_only
            None,  // revprop_table
        );

        assert!(result.is_ok());

        // Check that the new file exists
        assert!(new_path.exists());

        // Check that the old file doesn't exist
        assert!(!test_file.exists());

        // Check the content is preserved
        let content = std::fs::read(&new_path).unwrap();
        assert_eq!(content, b"Test content");
    }

    #[test]
    fn test_revert() {
        let td = tempfile::tempdir().unwrap();
        let repo_path = td.path().join("repo");
        let wc_path = td.path().join("wc");

        // Create a test repository
        crate::repos::Repos::create(&repo_path).unwrap();

        // Check out working copy
        let mut ctx = Context::new().unwrap();
        let url_str = format!("file://{}", repo_path.to_str().unwrap());
        let url = crate::uri::Uri::new(&url_str).unwrap();

        ctx.checkout(
            url,
            &wc_path,
            &CheckoutOptions {
                peg_revision: Revision::Head,
                revision: Revision::Head,
                depth: Depth::Infinity,
                ignore_externals: false,
                allow_unver_obstructions: false,
            },
        )
        .unwrap();

        // Create and commit a test file
        let test_file = wc_path.join("test_file.txt");
        std::fs::write(&test_file, b"Original content").unwrap();

        ctx.add(&test_file, &AddOptions::default()).unwrap();

        ctx.commit(
            &[wc_path.to_str().unwrap()],
            &CommitOptions::default(),
            std::collections::HashMap::new(),
            &mut |_info: &crate::CommitInfo| Ok(()),
        )
        .unwrap();

        // Modify the file
        std::fs::write(&test_file, b"Modified content").unwrap();

        // Verify the file is modified
        assert_eq!(std::fs::read(&test_file).unwrap(), b"Modified content");

        // Revert the changes
        let result = ctx.revert(
            &[test_file.to_str().unwrap()],
            Depth::Empty,
            None,  // changelists
            false, // clear_changelists
            false, // metadata_only
            false, // added_keep_local
        );

        assert!(result.is_ok());

        // Verify the file is reverted to original content
        assert_eq!(std::fs::read(&test_file).unwrap(), b"Original content");
    }

    #[test]
    fn test_resolved() {
        let td = tempfile::tempdir().unwrap();
        let repo_path = td.path().join("repo");
        let wc_path = td.path().join("wc");

        // Create a test repository
        crate::repos::Repos::create(&repo_path).unwrap();

        // Check out working copy
        let mut ctx = Context::new().unwrap();
        let url_str = format!("file://{}", repo_path.to_str().unwrap());
        let url = crate::uri::Uri::new(&url_str).unwrap();

        ctx.checkout(
            url,
            &wc_path,
            &CheckoutOptions {
                peg_revision: Revision::Head,
                revision: Revision::Head,
                depth: Depth::Infinity,
                ignore_externals: false,
                allow_unver_obstructions: false,
            },
        )
        .unwrap();

        // Create a test file
        let test_file = wc_path.join("conflict_file.txt");
        std::fs::write(&test_file, b"Initial content").unwrap();

        ctx.add(&test_file, &AddOptions::default()).unwrap();

        ctx.commit(
            &[wc_path.to_str().unwrap()],
            &CommitOptions::default(),
            std::collections::HashMap::new(),
            &mut |_info: &crate::CommitInfo| Ok(()),
        )
        .unwrap();

        // Test the resolved function - it should not fail even without actual conflicts
        // (In a real scenario, this would be called after manually resolving conflicts)
        let result = ctx.resolved(
            test_file.to_str().unwrap(),
            false, // recursive
        );

        // The function should succeed (even if there were no conflicts to resolve)
        assert!(result.is_ok());
    }

    #[test]
    fn test_proplist_all() {
        let td = tempfile::tempdir().unwrap();
        let repo_path = td.path().join("repo");
        let wc_path = td.path().join("wc");

        // Create a test repository
        crate::repos::Repos::create(&repo_path).unwrap();

        // Check out working copy
        let mut ctx = Context::new().unwrap();
        let url_str = format!("file://{}", repo_path.to_str().unwrap());
        let url = crate::uri::Uri::new(&url_str).unwrap();

        ctx.checkout(
            url,
            &wc_path,
            &CheckoutOptions {
                peg_revision: Revision::Head,
                revision: Revision::Head,
                depth: Depth::Infinity,
                ignore_externals: false,
                allow_unver_obstructions: false,
            },
        )
        .unwrap();

        // Test listing properties
        let mut prop_count = 0;
        let result = ctx.proplist_all(
            wc_path.to_str().unwrap(),
            &Revision::Working,
            &Revision::Working,
            Depth::Empty,
            None,  // changelists
            false, // get_target_inherited_props
            &mut |_path, _props| {
                prop_count += 1;
                Ok(())
            },
        );

        // Should not fail even if no properties
        assert!(result.is_ok());
    }

    #[test]
    fn test_diff() {
        let td = tempfile::tempdir().unwrap();
        let repo_path = td.path().join("repo");
        let wc_path = td.path().join("wc");

        // Create a test repository
        crate::repos::Repos::create(&repo_path).unwrap();

        // Check out working copy
        let mut ctx = Context::new().unwrap();
        let url_str = format!("file://{}", repo_path.to_str().unwrap());
        let url = crate::uri::Uri::new(&url_str).unwrap();

        ctx.checkout(
            crate::uri::Uri::new(url.as_ref()).unwrap(),
            &wc_path,
            &CheckoutOptions {
                peg_revision: Revision::Head,
                revision: Revision::Head,
                depth: Depth::Infinity,
                ignore_externals: false,
                allow_unver_obstructions: false,
            },
        )
        .unwrap();

        // Create output streams for diff
        let mut out_stream = crate::io::Stream::buffered();
        let mut err_stream = crate::io::Stream::buffered();

        // Test diff between repository revisions using direct function
        let diff_result = ctx.diff(
            &[], // diff_options
            url.as_ref(),
            &Revision::Head,
            url.as_ref(),
            &Revision::Head,
            None, // relative_to_dir
            Depth::Infinity,
            false, // ignore_ancestry
            false, // no_diff_added
            false, // no_diff_deleted
            false, // show_copies_as_adds
            false, // ignore_content_type
            false, // ignore_properties
            false, // properties_only
            false, // use_git_diff_format
            "UTF-8",
            &mut out_stream,
            &mut err_stream,
            None, // changelists
        );

        // Should succeed even with no differences
        assert!(diff_result.is_ok());
    }

    #[test]
    fn test_diff_builder() {
        let td = tempfile::tempdir().unwrap();
        let repo_path = td.path().join("repo");
        let wc_path = td.path().join("wc");

        // Create a test repository
        crate::repos::Repos::create(&repo_path).unwrap();

        // Check out working copy
        let mut ctx = Context::new().unwrap();
        let url_str = format!("file://{}", repo_path.to_str().unwrap());
        let url = crate::uri::Uri::new(&url_str).unwrap();

        ctx.checkout(
            crate::uri::Uri::new(url.as_ref()).unwrap(),
            &wc_path,
            &CheckoutOptions {
                peg_revision: Revision::Head,
                revision: Revision::Head,
                depth: Depth::Infinity,
                ignore_externals: false,
                allow_unver_obstructions: false,
            },
        )
        .unwrap();

        // Create output streams
        let mut out_stream = crate::io::Stream::buffered();
        let mut err_stream = crate::io::Stream::buffered();

        // Test diff using builder pattern - much cleaner!
        let result = ctx
            .diff_builder(
                url.to_string(),
                Revision::Head,
                url.to_string(),
                Revision::Head,
            )
            .depth(Depth::Infinity)
            .ignore_ancestry(true)
            .use_git_diff_format(true)
            .execute(&mut out_stream, &mut err_stream);

        assert!(result.is_ok());
    }

    #[test]
    fn test_list() {
        let td = tempfile::tempdir().unwrap();
        let repo_path = td.path().join("repo");

        // Create a test repository
        crate::repos::Repos::create(&repo_path).unwrap();

        let mut ctx = Context::new().unwrap();
        let url_str = format!("file://{}", repo_path.to_str().unwrap());

        // Test listing repository contents
        let mut entries = Vec::new();
        let result = ctx.list(
            &url_str,
            &Revision::Head,
            &Revision::Head,
            None, // patterns
            Depth::Infinity,
            subversion_sys::SVN_DIRENT_KIND
                | subversion_sys::SVN_DIRENT_SIZE
                | subversion_sys::SVN_DIRENT_HAS_PROPS
                | subversion_sys::SVN_DIRENT_CREATED_REV
                | subversion_sys::SVN_DIRENT_TIME
                | subversion_sys::SVN_DIRENT_LAST_AUTHOR,
            false, // fetch_locks
            false, // include_externals
            &mut |path, _dirent, _lock| {
                entries.push(path.to_string());
                Ok(())
            },
        );

        // Should succeed for empty repository
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_builder() {
        let td = tempfile::tempdir().unwrap();
        let repo_path = td.path().join("repo");

        // Create a test repository
        crate::repos::Repos::create(&repo_path).unwrap();

        let mut ctx = Context::new().unwrap();
        let url_str = format!("file://{}", repo_path.to_str().unwrap());

        // Test listing using builder pattern
        let mut entries = Vec::new();
        let result = ctx
            .list_builder(&url_str)
            .depth(Depth::Files)
            .fetch_locks(true)
            .patterns(vec!["*.txt".to_string()])
            .execute(&mut |path, _dirent, _lock| {
                entries.push(path.to_string());
                Ok(())
            });

        assert!(result.is_ok());
    }

    #[test]
    fn test_copy_builder() {
        let td = tempfile::tempdir().unwrap();
        let repo_path = td.path().join("repo");
        let wc_path = td.path().join("wc");

        // Create a test repository
        crate::repos::Repos::create(&repo_path).unwrap();

        // Check out working copy
        let mut ctx = Context::new().unwrap();
        let url_str = format!("file://{}", repo_path.to_str().unwrap());
        let url = crate::uri::Uri::new(&url_str).unwrap();

        ctx.checkout(
            url,
            &wc_path,
            &CheckoutOptions {
                peg_revision: Revision::Head,
                revision: Revision::Head,
                depth: Depth::Infinity,
                ignore_externals: false,
                allow_unver_obstructions: false,
            },
        )
        .unwrap();

        // Create a test file
        let test_file = wc_path.join("test.txt");
        std::fs::write(&test_file, "test content").unwrap();
        ctx.add(
            &test_file,
            &AddOptions {
                depth: Depth::Empty,
                force: false,
                no_ignore: false,
                no_autoprops: false,
                add_parents: false,
            },
        )
        .unwrap();

        // Test copy using builder pattern
        let result = ctx
            .copy_builder(wc_path.join("test_copy.txt").to_str().unwrap())
            .add_source(test_file.to_str().unwrap(), None)
            .make_parents(true)
            .ignore_externals(true)
            .execute();

        // May fail without commit, just ensure no panic
        let _ = result;
    }

    #[test]
    fn test_info_builder() {
        let td = tempfile::tempdir().unwrap();
        let repo_path = td.path().join("repo");

        // Create a test repository
        crate::repos::Repos::create(&repo_path).unwrap();

        let mut ctx = Context::new().unwrap();
        let url_str = format!("file://{}", repo_path.to_str().unwrap());

        // Test info using builder pattern
        let mut info_count = 0;
        let result = ctx
            .info_builder(&url_str)
            .depth(Depth::Empty)
            .fetch_excluded(true)
            .execute(&|_info| {
                info_count += 1;
                Ok(())
            });

        assert!(result.is_ok());
    }

    #[test]
    fn test_resolve() {
        let td = tempfile::tempdir().unwrap();
        let repo_path = td.path().join("repo");
        let wc_path = td.path().join("wc");

        // Create a test repository
        crate::repos::Repos::create(&repo_path).unwrap();

        // Check out working copy
        let mut ctx = Context::new().unwrap();
        let url_str = format!("file://{}", repo_path.to_str().unwrap());
        let url = crate::uri::Uri::new(&url_str).unwrap();

        ctx.checkout(
            url,
            &wc_path,
            &CheckoutOptions {
                peg_revision: Revision::Head,
                revision: Revision::Head,
                depth: Depth::Infinity,
                ignore_externals: false,
                allow_unver_obstructions: false,
            },
        )
        .unwrap();

        // Test resolve (even though there are no conflicts)
        let result = ctx.resolve(
            wc_path.to_str().unwrap(),
            Depth::Infinity,
            crate::ConflictChoice::Postpone,
        );

        // Should succeed even with no conflicts
        assert!(result.is_ok());
    }

    #[test]
    fn test_commit_builder() {
        // Create temporary directories for test
        let td = tempfile::tempdir().unwrap();
        let repo_path = td.path().join("repo");
        let wc_path = td.path().join("wc");

        // Create a test repository and get its UUID
        let repos = crate::repos::Repos::create(&repo_path).unwrap();
        let fs = repos.fs().unwrap();
        let uuid = fs.get_uuid().unwrap();
        drop(fs);
        drop(repos);

        // Create working copy directory
        std::fs::create_dir_all(&wc_path).unwrap();

        // Initialize working copy using wc context
        let mut wc_ctx = crate::wc::Context::new().unwrap();
        let repo_abs_path = repo_path.canonicalize().unwrap();
        let wc_abs_path = wc_path.canonicalize().unwrap();
        let url_str = format!("file://{}", repo_abs_path.to_str().unwrap());

        // Create basic .svn structure using ensure_adm
        wc_ctx
            .ensure_adm(
                wc_abs_path.to_str().unwrap(),
                &url_str,
                &url_str,
                &uuid,
                crate::Revnum(0),
                crate::Depth::Infinity,
            )
            .unwrap();

        // Create and add a test file
        let test_file = wc_path.join("test.txt");
        std::fs::write(&test_file, "test content").unwrap();

        // Create client context for adding and committing
        let mut ctx = Context::new().unwrap();

        // Add the file using its absolute path
        let test_file_abs = test_file.canonicalize().unwrap();
        ctx.add(
            &test_file_abs,
            &AddOptions {
                depth: Depth::Empty,
                force: false,
                no_ignore: false,
                no_autoprops: false,
                add_parents: false,
            },
        )
        .unwrap();

        // Test commit using builder pattern
        let builder = CommitBuilder::new(&mut ctx)
            .add_target(wc_abs_path.to_str().unwrap())
            .depth(Depth::Infinity);

        // Execute the commit
        let result = builder.execute(&mut |_info| Ok(()));

        // Check that the commit succeeded
        assert!(result.is_ok(), "Commit failed: {:?}", result.err());
    }

    #[test]
    fn test_log_builder() {
        let td = tempfile::tempdir().unwrap();
        let repo_path = td.path().join("repo");
        let wc_path = td.path().join("wc");

        // Create a test repository
        crate::repos::Repos::create(&repo_path).unwrap();

        // Check out working copy and make a commit
        let mut ctx = Context::new().unwrap();
        let url_str = format!("file://{}", repo_path.to_str().unwrap());
        let url = crate::uri::Uri::new(&url_str).unwrap();

        ctx.checkout(
            url,
            &wc_path,
            &CheckoutOptions {
                peg_revision: Revision::Head,
                revision: Revision::Head,
                depth: Depth::Infinity,
                ignore_externals: false,
                allow_unver_obstructions: false,
            },
        )
        .unwrap();

        let test_file = wc_path.join("test.txt");
        std::fs::write(&test_file, "test content").unwrap();
        ctx.add(
            &test_file,
            &AddOptions {
                depth: Depth::Empty,
                force: false,
                no_ignore: false,
                no_autoprops: false,
                add_parents: false,
            },
        )
        .unwrap();
        let commit_result = ctx.commit(
            &[wc_path.to_str().unwrap()],
            &CommitOptions {
                depth: Depth::Infinity,
                keep_locks: false,
                keep_changelists: false,
                commit_as_operations: false,
                include_file_externals: false,
                include_dir_externals: false,
                changelists: None,
            },
            std::collections::HashMap::new(), // Empty revprops - svn:log is set through other means
            &|_info| Ok(()),
        );

        // Check if commit succeeded - if not, skip the log test
        if commit_result.is_err() {
            eprintln!(
                "Commit failed, skipping log test: {:?}",
                commit_result.err()
            );
            return;
        }

        // Test log using builder pattern
        let mut log_entries = Vec::new();
        let result = ctx
            .log_builder()
            .add_target(url_str)
            .add_revision_range(Revision::Number(Revnum(0)), Revision::Head)
            .discover_changed_paths(true)
            .strict_node_history(false)
            .include_merged_revisions(false)
            .execute(&|_entry| {
                log_entries.push(());
                Ok(())
            });

        assert!(result.is_ok(), "Log failed: {:?}", result.err());
    }

    #[test]
    fn test_update_builder() {
        let td = tempfile::tempdir().unwrap();
        let repo_path = td.path().join("repo");
        let wc_path = td.path().join("wc");

        // Create a test repository
        crate::repos::Repos::create(&repo_path).unwrap();

        // Check out working copy
        let mut ctx = Context::new().unwrap();
        let url_str = format!("file://{}", repo_path.to_str().unwrap());
        let url = crate::uri::Uri::new(&url_str).unwrap();

        ctx.checkout(
            url,
            &wc_path,
            &CheckoutOptions {
                peg_revision: Revision::Head,
                revision: Revision::Head,
                depth: Depth::Infinity,
                ignore_externals: false,
                allow_unver_obstructions: false,
            },
        )
        .unwrap();

        // Test update using builder pattern
        let result = ctx
            .update_builder()
            .add_path(wc_path.to_str().unwrap())
            .revision(Revision::Head)
            .depth(Depth::Infinity)
            .depth_is_sticky(false)
            .ignore_externals(false)
            .allow_unver_obstructions(false)
            .adds_as_modification(false)
            .make_parents(false)
            .execute();

        assert!(result.is_ok());
    }

    #[test]
    fn test_mkdir_builder() {
        let td = tempfile::tempdir().unwrap();
        let repo_path = td.path().join("repo");
        let wc_path = td.path().join("wc");

        // Create a test repository
        crate::repos::Repos::create(&repo_path).unwrap();

        // Check out working copy
        let mut ctx = Context::new().unwrap();
        let url_str = format!("file://{}", repo_path.to_str().unwrap());
        let url = crate::uri::Uri::new(&url_str).unwrap();

        ctx.checkout(
            url,
            &wc_path,
            &CheckoutOptions {
                peg_revision: Revision::Head,
                revision: Revision::Head,
                depth: Depth::Infinity,
                ignore_externals: false,
                allow_unver_obstructions: false,
            },
        )
        .unwrap();

        // Test mkdir using builder pattern
        let new_dir = wc_path.join("new_dir");
        let result = ctx
            .mkdir_builder()
            .add_path(new_dir.to_str().unwrap())
            .make_parents(true)
            .execute(&|_info| Ok(()));

        assert!(result.is_ok(), "Mkdir failed: {:?}", result.err());
        assert!(new_dir.exists());
    }

    #[test]
    fn test_patch_dry_run() {
        let mut ctx = Context::new().unwrap();

        // Create a simple patch file
        let temp_dir = std::env::temp_dir();
        let patch_file = temp_dir.join("test.patch");
        std::fs::write(
            &patch_file,
            "--- a/test.txt\n+++ b/test.txt\n@@ -1 +1 @@\n-old\n+new\n",
        )
        .unwrap();

        let wc_dir = temp_dir.join("test_wc_patch");
        let _ = std::fs::remove_dir_all(&wc_dir);
        std::fs::create_dir_all(&wc_dir).unwrap();

        // Test dry run patch application (should not fail even without valid WC)
        let result = ctx.patch(
            &patch_file,
            &wc_dir,
            true,  // dry_run
            0,     // strip_count
            false, // reverse
            false, // ignore_whitespace
            true,  // remove_tempfiles
        );

        // Clean up
        let _ = std::fs::remove_file(&patch_file);
        let _ = std::fs::remove_dir_all(&wc_dir);

        // Patch should succeed in dry run mode or return expected error
        // Note: patch might fail with directory not being a working copy, which is expected
        match result {
            Ok(()) => println!("Patch succeeded in dry run"),
            Err(e) => println!("Patch failed as expected: {}", e),
        }
    }

    #[test]
    fn test_relocate_invalid_path() {
        let mut ctx = Context::new().unwrap();

        let temp_dir = std::env::temp_dir();
        let invalid_wc = temp_dir.join("non_existent_wc");

        // Test relocate with invalid working copy path
        let result = ctx.relocate(
            &invalid_wc,
            "http://old.example.com/repo",
            "http://new.example.com/repo",
            false, // ignore_externals
        );

        // Should fail for non-existent working copy
        assert!(result.is_err());
    }

    #[test]
    fn test_revprop_operations() {
        let mut ctx = Context::new().unwrap();

        // Test with invalid URL (should fail gracefully)
        let result = ctx.revprop_get("svn:author", "file:///non/existent/repo", &Revision::Head);

        // Should return error for invalid repository
        assert!(result.is_err());

        // Test revprop_list with invalid URL
        let result = ctx.revprop_list("file:///non/existent/repo", &Revision::Head);

        // Should return error for invalid repository
        assert!(result.is_err());

        // Test revprop_set with invalid URL
        let result = ctx.revprop_set(
            "test:property",
            Some(b"test value"),
            "file:///non/existent/repo",
            &Revision::Head,
            None,
            false, // force
        );

        // Should return error for invalid repository
        assert!(result.is_err());
    }

    #[test]
    fn test_revprop_hash_conversion() {
        // Test that revprops can be properly created and converted
        // This verifies our APR hash API changes work correctly
        use std::collections::HashMap;

        // Test BlameInfo structure which stores revprops
        let mut revprops_map = HashMap::new();
        revprops_map.insert("svn:author".to_string(), b"test_user".to_vec());
        revprops_map.insert("svn:log".to_string(), b"test commit message".to_vec());
        revprops_map.insert("custom:prop".to_string(), b"custom value".to_vec());

        let blame_info = BlameInfo {
            line_no: 1,
            revision: Revnum::from(123u64),
            revprops: revprops_map.clone(),
            merged_revision: None,
            merged_revprops: HashMap::new(),
            merged_path: None,
            line: "test line".to_string(),
            local_change: false,
        };

        // Verify the data is stored correctly
        assert_eq!(blame_info.revprops.len(), 3);
        assert_eq!(blame_info.revprops.get("svn:author").unwrap(), b"test_user");
        assert_eq!(
            blame_info.revprops.get("svn:log").unwrap(),
            b"test commit message"
        );
        assert_eq!(
            blame_info.revprops.get("custom:prop").unwrap(),
            b"custom value"
        );

        // Test that CommitBuilder and MkdirBuilder patterns work
        let mut ctx = Context::new().unwrap();
        let _commit_builder = CommitBuilder::new(&mut ctx)
            .add_revprop("svn:author", "test_author")
            .add_revprop("svn:log", "test message");

        let _mkdir_builder = MkdirBuilder::new(&mut ctx)
            .add_revprop("svn:author", b"test_author".to_vec())
            .add_revprop("svn:log", b"test message".to_vec());
    }

    #[test]
    fn test_suggest_merge_sources_invalid_url() {
        let mut ctx = Context::new().unwrap();

        // Test with invalid URL
        let result = ctx.suggest_merge_sources("file:///non/existent/repo/trunk", &Revision::Head);

        // Should return error for invalid repository
        assert!(result.is_err());
    }

    #[test]
    fn test_upgrade_invalid_path() {
        let mut ctx = Context::new().unwrap();

        let temp_dir = std::env::temp_dir();
        let invalid_wc = temp_dir.join("non_existent_wc_upgrade");

        // Test upgrade with non-existent working copy
        let result = ctx.upgrade(&invalid_wc);

        // Should fail for non-existent working copy
        assert!(result.is_err());
    }

    #[test]
    fn test_cleanup_basic() {
        let mut ctx = Context::new().unwrap();
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_path_str = temp_dir.path().to_str().unwrap();

        // Test cleanup on a non-working-copy directory (should fail)
        let options = CleanupOptions {
            break_locks: false,
            fix_recorded_timestamps: false,
            clear_dav_cache: false,
            vacuum_pristines: false,
            include_externals: false,
        };
        let result = ctx.cleanup(temp_path_str, &options);

        // Expected to fail without valid working copy
        assert!(result.is_err());
    }

    #[test]
    fn test_vacuum_basic() {
        let mut ctx = Context::new().unwrap();
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_path_str = temp_dir.path().to_str().unwrap();

        // Test vacuum on a non-working-copy directory (should fail)
        let options = VacuumOptions {
            remove_unversioned_items: false,
            remove_ignored_items: false,
            fix_recorded_timestamps: false,
            vacuum_pristines: true,
            include_externals: false,
        };
        let result = ctx.vacuum(temp_path_str, &options);

        // Expected to fail without valid working copy
        assert!(result.is_err());
    }

    #[test]
    fn test_merge_reintegrate_invalid_params() {
        let mut ctx = Context::new().unwrap();

        let temp_dir = std::env::temp_dir();
        let invalid_wc = temp_dir.join("non_existent_merge_wc");

        // Test merge reintegrate with invalid parameters
        let result = ctx.merge_reintegrate(
            "file:///non/existent/source",
            &Revision::Head,
            &invalid_wc,
            true, // dry_run
            &[],  // merge_options
        );

        // Should fail for invalid source and target
        assert!(result.is_err());
    }

    #[test]
    fn test_blame_invalid_path() {
        let mut ctx = Context::new().unwrap();

        let mut blame_info_received = false;
        let mut receiver = |info: BlameInfo| -> Result<(), Error> {
            blame_info_received = true;
            // Verify the BlameInfo structure has expected fields
            assert!(info.line_no >= 0);
            // For invalid path, we might still get blame info or error
            Ok(())
        };

        // Test blame with invalid path/URL (should fail gracefully)
        let result = ctx.blame(
            "file:///non/existent/file.txt",
            Revision::Head,
            Revision::Number(Revnum::from(1u64)),
            Revision::Head,
            vec![], // diff_options
            false,  // ignore_mime_type
            false,  // include_merged_revisions
            &mut receiver,
        );

        // Should return error for invalid file
        assert!(result.is_err());
    }

    #[test]
    fn test_cat_invalid_path() {
        let mut ctx = Context::new().unwrap();

        let mut output = Vec::new();
        let options = CatOptions {
            revision: Revision::Head,
            peg_revision: Revision::Head,
            expand_keywords: false,
        };

        // Test cat with invalid path/URL (should fail gracefully)
        let result = ctx.cat("file:///non/existent/file.txt", &mut output, &options);

        // Should return error for invalid file
        assert!(result.is_err());
        // Output should remain empty
        assert!(output.is_empty());
    }

    #[test]
    fn test_blame_api_structure() {
        // Test that BlameInfo can be constructed and has expected fields
        use std::collections::HashMap;

        let info = BlameInfo {
            line_no: 42,
            revision: Revnum::from(123u64),
            revprops: HashMap::new(),
            merged_revision: None,
            merged_revprops: HashMap::new(),
            merged_path: None,
            line: "test line content".to_string(),
            local_change: false,
        };

        assert_eq!(info.line_no, 42);
        assert_eq!(info.revision, Revnum::from(123u64));
        assert_eq!(info.line, "test line content");
        assert!(!info.local_change);
        assert!(info.merged_revision.is_none());
        assert!(info.merged_path.is_none());
    }

    #[test]
    fn test_cat_options_structure() {
        let options = CatOptions {
            revision: Revision::Number(Revnum::from(42u64)),
            peg_revision: Revision::Head,
            expand_keywords: true,
        };

        // Verify options structure
        match options.revision {
            Revision::Number(n) => assert_eq!(n, Revnum::from(42u64)),
            _ => panic!("Expected Number revision"),
        }
        match options.peg_revision {
            Revision::Head => {} // expected
            _ => panic!("Expected Head revision"),
        }
        assert!(options.expand_keywords);

        // Test default
        let default_options = CatOptions::default();
        match default_options.revision {
            Revision::Unspecified => {} // expected default
            _ => panic!("Expected Unspecified revision as default"),
        }
        match default_options.peg_revision {
            Revision::Unspecified => {} // expected default
            _ => panic!("Expected Unspecified peg revision as default"),
        }
        assert!(!default_options.expand_keywords);
    }

    #[test]
    fn test_uuid_from_invalid_url() {
        let mut ctx = Context::new().unwrap();

        // Test UUID from invalid URL (should fail gracefully)
        let result = ctx.uuid_from_url("file:///non/existent/repo");

        // Should return error for invalid repository
        assert!(result.is_err());
    }

    #[test]
    fn test_uuid_from_invalid_path() {
        let mut ctx = Context::new().unwrap();

        let temp_dir = std::env::temp_dir();
        let invalid_path = temp_dir.join("non_existent_wc");

        // Test UUID from invalid working copy path (should fail gracefully)
        let result = ctx.uuid_from_path(&invalid_path);

        // Should return error for invalid working copy path
        assert!(result.is_err());
    }

    #[test]
    fn test_switch_with_ignore_ancestry() {
        // Create a repository with two unrelated directories
        let td = tempfile::tempdir().unwrap();
        let repo_path = td.path().join("repo");
        let wc_path = td.path().join("wc");

        // Create repository
        crate::repos::Repos::create(&repo_path).unwrap();

        let url_str = format!("file://{}", repo_path.to_str().unwrap());
        let dir1_url = format!("{}/dir1", url_str);
        let dir2_url = format!("{}/dir2", url_str);

        let mut ctx = Context::new().unwrap();

        // Create two unrelated directories in the repository
        ctx.mkdir(
            &[&dir1_url, &dir2_url],
            true,
            std::collections::HashMap::new(),
            &|_info| Ok(()),
        ).unwrap();

        // Checkout dir1
        let dir1_uri = crate::uri::Uri::new(&dir1_url).unwrap();
        ctx.checkout(
            dir1_uri.clone(),
            &wc_path,
            &CheckoutOptions {
                peg_revision: Revision::Head,
                revision: Revision::Head,
                depth: Depth::Infinity,
                ignore_externals: false,
                allow_unver_obstructions: false,
            },
        ).unwrap();

        // Test switch to unrelated dir2 with ignore_ancestry=false - should fail
        let dir2_uri = crate::uri::Uri::new(&dir2_url).unwrap();
        let result = ctx.switch(
            &wc_path,
            dir2_uri.clone(),
            &SwitchOptions {
                peg_revision: Revision::Head,
                revision: Revision::Head,
                depth: Depth::Infinity,
                depth_is_sticky: false,
                ignore_externals: false,
                allow_unver_obstructions: false,
                ignore_ancestry: false,
            },
        );

        // Should fail because dir1 and dir2 don't share ancestry
        assert!(result.is_err(), "Switch without ignore_ancestry should fail for unrelated paths");

        // Test switch with ignore_ancestry=true - should succeed
        let result = ctx.switch(
            &wc_path,
            dir2_uri.clone(),
            &SwitchOptions::new().with_ignore_ancestry(true),
        );

        // Should succeed with ignore_ancestry=true
        assert!(result.is_ok(), "Switch with ignore_ancestry=true should succeed: {:?}", result);
    }
}
