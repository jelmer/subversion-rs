//! Deprecated working copy access baton API.
//!
//! This module provides the deprecated `svn_wc_adm_access_t` based API.
//! New code should use [`super::Context`] instead.

#[cfg(feature = "ra")]
use super::{box_notify_baton_borrowed, drop_notify_baton_borrowed, wrap_notify_func, Notify};
use super::{CommittedQueue, ConflictChoice, Lock, PropChange};
use crate::{svn_result, with_tmp_pool};
use std::collections::HashMap;

/// Helper to convert a C string pointer to an owned Option<String>.
unsafe fn cstr_to_option(ptr: *const std::ffi::c_char) -> Option<String> {
    if ptr.is_null() {
        None
    } else {
        Some(
            std::ffi::CStr::from_ptr(ptr)
                .to_str()
                .expect("expected valid UTF-8")
                .to_string(),
        )
    }
}

/// A deprecated working copy entry (`svn_wc_entry_t`).
///
/// All fields are owned copies -- safe to use after the access baton is closed.
#[derive(Debug, Clone)]
pub struct Entry {
    /// Entry's name.
    pub name: Option<String>,
    /// Base revision.
    pub revision: crate::Revnum,
    /// URL in repository.
    pub url: Option<String>,
    /// Canonical repository URL.
    pub repos: Option<String>,
    /// Repository UUID.
    pub uuid: Option<String>,
    /// Node kind (file, dir, ...).
    pub kind: crate::NodeKind,
    /// Scheduling (add, delete, replace, ...).
    pub schedule: u32,
    /// Whether this entry is in a copied state.
    pub copied: bool,
    /// Whether this entry has been deleted.
    pub deleted: bool,
    /// Whether this entry is absent (e.g. due to authz restrictions).
    pub absent: bool,
    /// Whether the entries file is incomplete.
    pub incomplete: bool,
    /// Copyfrom URL.
    pub copyfrom_url: Option<String>,
    /// Copyfrom revision.
    pub copyfrom_rev: crate::Revnum,
    /// Old version of conflicted file.
    pub conflict_old: Option<String>,
    /// New version of conflicted file.
    pub conflict_new: Option<String>,
    /// Working version of conflicted file.
    pub conflict_wrk: Option<String>,
    /// Property reject file.
    pub prejfile: Option<String>,
    /// Last up-to-date time for text contents.
    pub text_time: i64,
    /// Last up-to-date time for properties.
    pub prop_time: i64,
    /// Hex MD5 checksum for the untranslated text base file.
    pub checksum: Option<String>,
    /// Last revision this was changed.
    pub cmt_rev: crate::Revnum,
    /// Last date this was changed.
    pub cmt_date: i64,
    /// Last commit author.
    pub cmt_author: Option<String>,
    /// Lock token, or `None` if not locked.
    pub lock_token: Option<String>,
    /// Lock owner, or `None` if not locked.
    pub lock_owner: Option<String>,
    /// Lock comment, or `None` if not locked or no comment.
    pub lock_comment: Option<String>,
    /// Lock creation date, or 0 if not locked.
    pub lock_creation_date: i64,
    /// Whether this entry has any working properties.
    pub has_props: bool,
    /// Whether this entry has property modifications.
    pub has_prop_mods: bool,
    /// Changelist this item belongs to.
    pub changelist: Option<String>,
    /// Size of the file after translation to local representation.
    pub working_size: i64,
    /// Whether a local copy should be kept after deletion.
    pub keep_local: bool,
    /// The depth of this entry.
    pub depth: crate::Depth,
}

impl Entry {
    /// Create an `Entry` by copying all fields from a raw `svn_wc_entry_t` pointer.
    ///
    /// # Safety
    ///
    /// `ptr` must be a valid, non-null pointer to a `svn_wc_entry_t`.
    pub unsafe fn from_raw(ptr: *const subversion_sys::svn_wc_entry_t) -> Self {
        let e = &*ptr;
        Entry {
            name: cstr_to_option(e.name),
            revision: crate::Revnum(e.revision),
            url: cstr_to_option(e.url),
            repos: cstr_to_option(e.repos),
            uuid: cstr_to_option(e.uuid),
            kind: crate::NodeKind::from(e.kind),
            schedule: e.schedule as u32,
            copied: e.copied != 0,
            deleted: e.deleted != 0,
            absent: e.absent != 0,
            incomplete: e.incomplete != 0,
            copyfrom_url: cstr_to_option(e.copyfrom_url),
            copyfrom_rev: crate::Revnum(e.copyfrom_rev),
            conflict_old: cstr_to_option(e.conflict_old),
            conflict_new: cstr_to_option(e.conflict_new),
            conflict_wrk: cstr_to_option(e.conflict_wrk),
            prejfile: cstr_to_option(e.prejfile),
            text_time: e.text_time,
            prop_time: e.prop_time,
            checksum: cstr_to_option(e.checksum),
            cmt_rev: crate::Revnum(e.cmt_rev),
            cmt_date: e.cmt_date,
            cmt_author: cstr_to_option(e.cmt_author),
            lock_token: cstr_to_option(e.lock_token),
            lock_owner: cstr_to_option(e.lock_owner),
            lock_comment: cstr_to_option(e.lock_comment),
            lock_creation_date: e.lock_creation_date,
            has_props: e.has_props != 0,
            has_prop_mods: e.has_prop_mods != 0,
            changelist: cstr_to_option(e.changelist),
            working_size: e.working_size,
            keep_local: e.keep_local != 0,
            depth: crate::Depth::from(e.depth),
        }
    }
}

/// Deprecated working copy status (version 2, wrapping `svn_wc_status2_t`).
///
/// All fields are owned copies.
#[derive(Debug, Clone)]
pub struct Status {
    /// The entry, or `None` if not under version control.
    pub entry: Option<Entry>,
    /// The status of the entry's text.
    pub text_status: u32,
    /// The status of the entry's properties.
    pub prop_status: u32,
    /// Whether the directory is locked (interrupted update).
    pub locked: bool,
    /// Whether the entry is copied (scheduled for addition-with-history).
    pub copied: bool,
    /// Whether the entry is switched.
    pub switched: bool,
    /// The entry's text status in the repository.
    pub repos_text_status: u32,
    /// The entry's property status in the repository.
    pub repos_prop_status: u32,
    /// The URI of the item.
    pub url: Option<String>,
    /// Youngest committed revision, or invalid if not out of date.
    pub ood_last_cmt_rev: crate::Revnum,
    /// Most recent commit date, or 0 if not out of date.
    pub ood_last_cmt_date: i64,
    /// Node kind of the youngest commit.
    pub ood_kind: u32,
    /// User name of the youngest commit author.
    pub ood_last_cmt_author: Option<String>,
    /// Whether the item is a file external.
    pub file_external: bool,
    /// Pristine text status (not masked by other statuses).
    pub pristine_text_status: u32,
    /// Pristine property status (not masked by other statuses).
    pub pristine_prop_status: u32,
}

impl Status {
    /// Create a `Status` by copying all fields from a raw `svn_wc_status2_t` pointer.
    ///
    /// # Safety
    ///
    /// `ptr` must be a valid, non-null pointer to a `svn_wc_status2_t`.
    pub unsafe fn from_raw(ptr: *const subversion_sys::svn_wc_status2_t) -> Self {
        let s = &*ptr;
        let entry = if s.entry.is_null() {
            None
        } else {
            Some(Entry::from_raw(s.entry))
        };
        Status {
            entry,
            text_status: s.text_status as u32,
            prop_status: s.prop_status as u32,
            locked: s.locked != 0,
            copied: s.copied != 0,
            switched: s.switched != 0,
            repos_text_status: s.repos_text_status as u32,
            repos_prop_status: s.repos_prop_status as u32,
            url: cstr_to_option(s.url),
            ood_last_cmt_rev: crate::Revnum(s.ood_last_cmt_rev),
            ood_last_cmt_date: s.ood_last_cmt_date,
            ood_kind: s.ood_kind as u32,
            ood_last_cmt_author: cstr_to_option(s.ood_last_cmt_author),
            file_external: s.file_external != 0,
            pristine_text_status: s.pristine_text_status as u32,
            pristine_prop_status: s.pristine_prop_status as u32,
        }
    }
}

/// Working copy administrative access baton.
///
/// Provides write locking for working copy operations. The `svn_wc_*`
/// functions that modify the working copy require a write lock, which
/// is acquired by opening an `Adm` with `write_lock=true`.
#[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
pub struct Adm<'a> {
    ptr: *mut subversion_sys::svn_wc_adm_access_t,
    /// Pool that owns the baton. `Some` for root adms, `None` for sub-adms
    /// obtained via `probe_try` (whose baton lives in the parent's pool).
    _pool: Option<apr::Pool<'static>>,
    _marker: std::marker::PhantomData<&'a ()>,
}

#[allow(deprecated)]
impl Adm<'_> {
    /// Open an access baton for a working copy directory.
    ///
    /// If `write_lock` is true, acquire a write lock on the directory.
    /// `levels_to_lock` controls depth: 0 = just this dir, -1 = infinite.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn open(
        path: &str,
        write_lock: bool,
        levels_to_lock: i32,
    ) -> Result<Adm<'static>, crate::Error<'static>> {
        let pool = apr::Pool::new();
        let path_cstr = crate::dirent::to_absolute_cstring(path)?;
        let mut adm_access: *mut subversion_sys::svn_wc_adm_access_t = std::ptr::null_mut();
        with_tmp_pool(|_scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_adm_open3(
                    &mut adm_access,
                    std::ptr::null_mut(), // associated
                    path_cstr.as_ptr(),
                    if write_lock { 1 } else { 0 },
                    levels_to_lock,
                    None,                 // cancel_func
                    std::ptr::null_mut(), // cancel_baton
                    pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })?;
        Ok(Adm {
            ptr: adm_access,
            _pool: Some(pool),
            _marker: std::marker::PhantomData,
        })
    }

    /// Check whether this baton holds a write lock.
    pub fn is_locked(&self) -> bool {
        unsafe { subversion_sys::svn_wc_adm_locked(self.ptr) != 0 }
    }

    /// Return the path associated with this access baton.
    pub fn access_path(&self) -> &str {
        let cstr = unsafe { subversion_sys::svn_wc_adm_access_path(self.ptr) };
        unsafe { std::ffi::CStr::from_ptr(cstr) }
            .to_str()
            .expect("access path should be valid UTF-8")
    }

    /// Return the internal pointer for use with deprecated svn_wc_* functions.
    pub fn as_ptr(&self) -> *mut subversion_sys::svn_wc_adm_access_t {
        self.ptr
    }

    /// Set a property on a path in the working copy.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn prop_set(
        &self,
        name: &str,
        value: Option<&[u8]>,
        path: &str,
    ) -> Result<(), crate::Error<'static>> {
        let name_cstr = std::ffi::CString::new(name).unwrap();
        let path_cstr = crate::dirent::to_absolute_cstring(path)?;
        let value_pool = apr::Pool::new();
        let value_svn = value.map(|v| crate::string::BStr::from_bytes(v, &value_pool));
        let value_ptr = value_svn
            .as_ref()
            .map(|v| v.as_ptr() as *const subversion_sys::svn_string_t)
            .unwrap_or(std::ptr::null());
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_prop_set3(
                    name_cstr.as_ptr(),
                    value_ptr,
                    path_cstr.as_ptr(),
                    self.ptr,
                    0,                    // skip_checks
                    None,                 // notify_func
                    std::ptr::null_mut(), // notify_baton
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })
    }

    /// Get a property value for a path in the working copy.
    ///
    /// Returns `None` if the path is not versioned or the property is not set.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn prop_get(
        &self,
        name: &str,
        path: &str,
    ) -> Result<Option<Vec<u8>>, crate::Error<'static>> {
        let name_cstr = std::ffi::CString::new(name).unwrap();
        let path_cstr = crate::dirent::to_absolute_cstring(path)?;
        let mut value: *const subversion_sys::svn_string_t = std::ptr::null();
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_prop_get(
                    &mut value,
                    name_cstr.as_ptr(),
                    path_cstr.as_ptr(),
                    self.ptr,
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)?;
            if value.is_null() {
                Ok(None)
            } else {
                let svn_str = unsafe { &*value };
                let data = unsafe {
                    std::slice::from_raw_parts(svn_str.data as *const u8, svn_str.len as usize)
                };
                Ok(Some(data.to_vec()))
            }
        })
    }

    /// List all properties for a path in the working copy.
    ///
    /// Returns a map of property names to values.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn prop_list(&self, path: &str) -> Result<HashMap<String, Vec<u8>>, crate::Error<'static>> {
        let path_cstr = crate::dirent::to_absolute_cstring(path)?;
        let mut props: *mut apr::hash::apr_hash_t = std::ptr::null_mut();
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_prop_list(
                    &mut props,
                    path_cstr.as_ptr(),
                    self.ptr,
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)?;
            let mut result = HashMap::new();
            if !props.is_null() {
                let mut hi = unsafe { apr_sys::apr_hash_first(scratch_pool.as_mut_ptr(), props) };
                while !hi.is_null() {
                    let mut key: *const std::ffi::c_void = std::ptr::null();
                    let mut val: *mut std::ffi::c_void = std::ptr::null_mut();
                    let mut klen: apr_sys::apr_ssize_t = 0;
                    unsafe {
                        apr_sys::apr_hash_this(hi, &mut key, &mut klen, &mut val);
                    }
                    let name = unsafe { std::ffi::CStr::from_ptr(key as *const std::ffi::c_char) }
                        .to_str()
                        .expect("property name should be valid UTF-8")
                        .to_string();
                    let svn_str = unsafe { &*(val as *const subversion_sys::svn_string_t) };
                    let data = unsafe {
                        std::slice::from_raw_parts(svn_str.data as *const u8, svn_str.len as usize)
                    };
                    result.insert(name, data.to_vec());
                    hi = unsafe { apr_sys::apr_hash_next(hi) };
                }
            }
            Ok(result)
        })
    }

    /// Add a path to the working copy.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn add(
        &self,
        path: &str,
        copyfrom_url: Option<&str>,
        copyfrom_rev: Option<crate::Revnum>,
    ) -> Result<(), crate::Error<'static>> {
        let path_cstr = crate::dirent::to_absolute_cstring(path)?;
        let copyfrom_url_cstr = copyfrom_url.map(std::ffi::CString::new).transpose()?;
        let copyfrom_rev_raw = copyfrom_rev.map(|r| r.0).unwrap_or(-1);
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_add3(
                    path_cstr.as_ptr(),
                    self.ptr,
                    subversion_sys::svn_depth_t_svn_depth_infinity,
                    copyfrom_url_cstr
                        .as_ref()
                        .map_or(std::ptr::null(), |c| c.as_ptr()),
                    copyfrom_rev_raw,
                    None,                 // cancel_func
                    std::ptr::null_mut(), // cancel_baton
                    None,                 // notify_func
                    std::ptr::null_mut(), // notify_baton
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })
    }

    /// Add a file from the repository to the working copy, installing
    /// its pristine text.
    ///
    /// This wraps the deprecated `svn_wc_add_repos_file3` which takes
    /// an `svn_wc_adm_access_t` and installs the pristine content into
    /// the WC's pristine store.
    ///
    /// `new_base_props` are the unmodified properties from the repository.
    /// `new_props` are the actual working copy properties (or `None` to
    /// use the base props).
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn add_repos_file(
        &self,
        path: &str,
        new_base_contents: &mut crate::io::Stream,
        new_contents: Option<&mut crate::io::Stream>,
        new_base_props: &std::collections::HashMap<String, Vec<u8>>,
        new_props: Option<&std::collections::HashMap<String, Vec<u8>>>,
        copyfrom_url: Option<&str>,
        copyfrom_rev: Option<crate::Revnum>,
    ) -> Result<(), crate::Error<'static>> {
        let path_cstr = std::ffi::CString::new(path)?;
        let copyfrom_url_cstr = copyfrom_url.map(std::ffi::CString::new).transpose()?;
        let copyfrom_rev_raw = copyfrom_rev.map(|r| r.0).unwrap_or(-1);
        with_tmp_pool(|scratch_pool| {
            let mut base_props_hash = apr::hash::Hash::new(scratch_pool);
            for (key, value) in new_base_props {
                let svn_str = crate::string::BStr::from_bytes(value, scratch_pool);
                unsafe {
                    base_props_hash
                        .insert(key.as_bytes(), svn_str.as_ptr() as *mut std::ffi::c_void);
                }
            }

            let mut props_hash_storage;
            let props_ptr = if let Some(props) = new_props {
                props_hash_storage = apr::hash::Hash::new(scratch_pool);
                for (key, value) in props {
                    let svn_str = crate::string::BStr::from_bytes(value, scratch_pool);
                    unsafe {
                        props_hash_storage
                            .insert(key.as_bytes(), svn_str.as_ptr() as *mut std::ffi::c_void);
                    }
                }
                unsafe { props_hash_storage.as_mut_ptr() }
            } else {
                std::ptr::null_mut()
            };

            let err = unsafe {
                subversion_sys::svn_wc_add_repos_file3(
                    path_cstr.as_ptr(),
                    self.ptr,
                    new_base_contents.as_mut_ptr(),
                    new_contents
                        .map(|s| s.as_mut_ptr())
                        .unwrap_or(std::ptr::null_mut()),
                    base_props_hash.as_mut_ptr(),
                    props_ptr,
                    copyfrom_url_cstr
                        .as_ref()
                        .map_or(std::ptr::null(), |c| c.as_ptr()),
                    copyfrom_rev_raw,
                    None,                 // cancel_func
                    std::ptr::null_mut(), // cancel_baton
                    None,                 // notify_func
                    std::ptr::null_mut(), // notify_baton
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })
    }

    /// Delete a path from the working copy.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn delete(&self, path: &str, keep_local: bool) -> Result<(), crate::Error<'static>> {
        let path_cstr = crate::dirent::to_absolute_cstring(path)?;
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_delete3(
                    path_cstr.as_ptr(),
                    self.ptr,
                    None,                 // cancel_func
                    std::ptr::null_mut(), // cancel_baton
                    None,                 // notify_func
                    std::ptr::null_mut(), // notify_baton
                    if keep_local { 1 } else { 0 },
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })
    }

    /// Copy a path within the working copy.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn copy(&self, src: &str, dst_basename: &str) -> Result<(), crate::Error<'static>> {
        let src_cstr = crate::dirent::to_absolute_cstring(src)?;
        let dst_cstr = std::ffi::CString::new(dst_basename)?;
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_copy2(
                    src_cstr.as_ptr(),
                    self.ptr,
                    dst_cstr.as_ptr(),
                    None,                 // cancel_func
                    std::ptr::null_mut(), // cancel_baton
                    None,                 // notify_func
                    std::ptr::null_mut(), // notify_baton
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })
    }

    /// Check whether a file has a binary svn:mime-type property.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn has_binary_prop(&self, path: &str) -> Result<bool, crate::Error<'static>> {
        let path_cstr = crate::dirent::to_absolute_cstring(path)?;
        let mut has_binary: subversion_sys::svn_boolean_t = 0;
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_has_binary_prop(
                    &mut has_binary,
                    path_cstr.as_ptr(),
                    self.ptr,
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)?;
            Ok(has_binary != 0)
        })
    }

    /// Check whether a file's text is modified with respect to the base revision.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn text_modified(
        &self,
        path: &str,
        force_comparison: bool,
    ) -> Result<bool, crate::Error<'static>> {
        let path_cstr = crate::dirent::to_absolute_cstring(path)?;
        let mut modified: subversion_sys::svn_boolean_t = 0;
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_text_modified_p(
                    &mut modified,
                    path_cstr.as_ptr(),
                    if force_comparison { 1 } else { 0 },
                    self.ptr,
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)?;
            Ok(modified != 0)
        })
    }

    /// Check whether a path's properties are modified with respect to the base revision.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn props_modified(&self, path: &str) -> Result<bool, crate::Error<'static>> {
        let path_cstr = crate::dirent::to_absolute_cstring(path)?;
        let mut modified: subversion_sys::svn_boolean_t = 0;
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_props_modified_p(
                    &mut modified,
                    path_cstr.as_ptr(),
                    self.ptr,
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)?;
            Ok(modified != 0)
        })
    }

    /// Check whether a path is a working copy root.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn is_wc_root(&self, path: &str) -> Result<bool, crate::Error<'static>> {
        let path_cstr = crate::dirent::to_absolute_cstring(path)?;
        let mut wc_root: subversion_sys::svn_boolean_t = 0;
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_is_wc_root(
                    &mut wc_root,
                    path_cstr.as_ptr(),
                    self.ptr,
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)?;
            Ok(wc_root != 0)
        })
    }

    /// Check whether a path has text, property, or tree conflicts.
    ///
    /// Returns `(text_conflicted, prop_conflicted, tree_conflicted)`.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn conflicted(&self, path: &str) -> Result<(bool, bool, bool), crate::Error<'static>> {
        let path_cstr = crate::dirent::to_absolute_cstring(path)?;
        let mut text: subversion_sys::svn_boolean_t = 0;
        let mut prop: subversion_sys::svn_boolean_t = 0;
        let mut tree: subversion_sys::svn_boolean_t = 0;
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_conflicted_p2(
                    &mut text,
                    &mut prop,
                    &mut tree,
                    path_cstr.as_ptr(),
                    self.ptr,
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)?;
            Ok((text != 0, prop != 0, tree != 0))
        })
    }

    /// Get the ancestry (URL and revision) for a versioned path.
    ///
    /// Returns `(url, revision)`.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn get_ancestry(
        &self,
        path: &str,
    ) -> Result<(Option<String>, crate::Revnum), crate::Error<'static>> {
        let path_cstr = crate::dirent::to_absolute_cstring(path)?;
        let mut url: *mut std::ffi::c_char = std::ptr::null_mut();
        let mut rev: subversion_sys::svn_revnum_t = -1;
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_get_ancestry(
                    &mut url,
                    &mut rev,
                    path_cstr.as_ptr(),
                    self.ptr,
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)?;
            let url_str = if url.is_null() {
                None
            } else {
                Some(
                    unsafe { std::ffi::CStr::from_ptr(url) }
                        .to_str()
                        .expect("URL should be valid UTF-8")
                        .to_string(),
                )
            };
            Ok((url_str, crate::Revnum(rev)))
        })
    }

    /// Get the property differences between the working copy and the base revision.
    ///
    /// Returns `(changes, original_props)`.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn get_prop_diffs(
        &self,
        path: &str,
    ) -> Result<(Vec<PropChange>, Option<HashMap<String, Vec<u8>>>), crate::Error<'static>> {
        let path_cstr = crate::dirent::to_absolute_cstring(path)?;
        with_tmp_pool(|scratch_pool| {
            let mut propchanges: *mut apr::tables::apr_array_header_t = std::ptr::null_mut();
            let mut original_props: *mut apr::hash::apr_hash_t = std::ptr::null_mut();
            let err = unsafe {
                subversion_sys::svn_wc_get_prop_diffs(
                    &mut propchanges,
                    &mut original_props,
                    path_cstr.as_ptr(),
                    self.ptr,
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)?;
            let changes = if propchanges.is_null() {
                Vec::new()
            } else {
                let array = unsafe {
                    apr::tables::TypedArray::<subversion_sys::svn_prop_t>::from_ptr(propchanges)
                };
                array
                    .iter()
                    .map(|prop| unsafe {
                        let name = std::ffi::CStr::from_ptr(prop.name)
                            .to_str()
                            .expect("Property name is not valid UTF-8")
                            .to_owned();
                        let value = if prop.value.is_null() {
                            None
                        } else {
                            Some(crate::svn_string_helpers::to_vec(&*prop.value))
                        };
                        PropChange { name, value }
                    })
                    .collect()
            };
            let original = if original_props.is_null() {
                None
            } else {
                let prop_hash = unsafe { crate::props::PropHash::from_ptr(original_props) };
                Some(prop_hash.to_hashmap())
            };
            Ok((changes, original))
        })
    }

    /// Mark a missing directory as deleted in the working copy.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn mark_missing_deleted(&self, path: &str) -> Result<(), crate::Error<'static>> {
        let path_cstr = crate::dirent::to_absolute_cstring(path)?;
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_mark_missing_deleted(
                    path_cstr.as_ptr(),
                    self.ptr,
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })
    }

    /// Set the repository root URL for a path, if not already set.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn maybe_set_repos_root(
        &self,
        path: &str,
        repos: &str,
    ) -> Result<(), crate::Error<'static>> {
        let path_cstr = crate::dirent::to_absolute_cstring(path)?;
        let repos_cstr = std::ffi::CString::new(repos)?;
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_maybe_set_repos_root(
                    self.ptr,
                    path_cstr.as_ptr(),
                    repos_cstr.as_ptr(),
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })
    }

    /// Add a repository lock to a path in the working copy.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn add_lock(&self, path: &str, lock: &Lock) -> Result<(), crate::Error<'static>> {
        let path_cstr = crate::dirent::to_absolute_cstring(path)?;
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_add_lock(
                    path_cstr.as_ptr(),
                    lock.as_ptr(),
                    self.ptr,
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })
    }

    /// Remove a repository lock from a path in the working copy.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn remove_lock(&self, path: &str) -> Result<(), crate::Error<'static>> {
        let path_cstr = crate::dirent::to_absolute_cstring(path)?;
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_remove_lock(
                    path_cstr.as_ptr(),
                    self.ptr,
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })
    }

    /// Remove a path from revision control.
    ///
    /// If `destroy_wf` is true, destroy the working file/directory.
    /// If `instant_error` is true, return an error immediately if a file
    /// is modified rather than trying to continue.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn remove_from_revision_control(
        &self,
        name: &str,
        destroy_wf: bool,
        instant_error: bool,
    ) -> Result<(), crate::Error<'static>> {
        let name_cstr = std::ffi::CString::new(name)?;
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_remove_from_revision_control(
                    self.ptr,
                    name_cstr.as_ptr(),
                    if destroy_wf { 1 } else { 0 },
                    if instant_error { 1 } else { 0 },
                    None,                 // cancel_func
                    std::ptr::null_mut(), // cancel_baton
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })
    }

    /// Resolve a conflict on a path.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn resolved_conflict(
        &self,
        path: &str,
        resolve_text: bool,
        resolve_props: bool,
        resolve_tree: bool,
        depth: crate::Depth,
        conflict_choice: ConflictChoice,
    ) -> Result<(), crate::Error<'static>> {
        let path_cstr = crate::dirent::to_absolute_cstring(path)?;
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_resolved_conflict4(
                    path_cstr.as_ptr(),
                    self.ptr,
                    if resolve_text { 1 } else { 0 },
                    if resolve_props { 1 } else { 0 },
                    if resolve_tree { 1 } else { 0 },
                    depth.into(),
                    conflict_choice.into(),
                    None,                 // notify_func
                    std::ptr::null_mut(), // notify_baton
                    None,                 // cancel_func
                    std::ptr::null_mut(), // cancel_baton
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })
    }

    /// Revert changes to a path.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn revert(
        &self,
        path: &str,
        depth: crate::Depth,
        use_commit_times: bool,
    ) -> Result<(), crate::Error<'static>> {
        let path_cstr = crate::dirent::to_absolute_cstring(path)?;
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_revert3(
                    path_cstr.as_ptr(),
                    self.ptr,
                    depth.into(),
                    if use_commit_times { 1 } else { 0 },
                    std::ptr::null(),     // changelist_filter
                    None,                 // cancel_func
                    std::ptr::null_mut(), // cancel_baton
                    None,                 // notify_func
                    std::ptr::null_mut(), // notify_baton
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })
    }

    /// Set the changelist for a path.
    ///
    /// Pass `None` for `changelist` to remove the path from its changelist.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn set_changelist(
        &self,
        path: &str,
        changelist: Option<&str>,
    ) -> Result<(), crate::Error<'static>> {
        let path_cstr = crate::dirent::to_absolute_cstring(path)?;
        let changelist_cstr = changelist.map(std::ffi::CString::new).transpose()?;
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_set_changelist(
                    path_cstr.as_ptr(),
                    changelist_cstr
                        .as_ref()
                        .map_or(std::ptr::null(), |c| c.as_ptr()),
                    self.ptr,
                    None,                 // cancel_func
                    std::ptr::null_mut(), // cancel_baton
                    None,                 // notify_func
                    std::ptr::null_mut(), // notify_baton
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })
    }

    /// Crop a working copy tree to a given depth.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn crop_tree(
        &self,
        target: &str,
        depth: crate::Depth,
    ) -> Result<(), crate::Error<'static>> {
        let target_cstr = std::ffi::CString::new(target)?;
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_crop_tree(
                    self.ptr,
                    target_cstr.as_ptr(),
                    depth.into(),
                    None,                 // notify_func
                    std::ptr::null_mut(), // notify_baton
                    None,                 // cancel_func
                    std::ptr::null_mut(), // cancel_baton
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })
    }

    /// Merge property changes into a path.
    ///
    /// Applies `propchanges` (a list of `(name, value)` pairs) to `path`.
    /// If `base_merge` is true, changes are applied to base props instead of working props.
    /// If `dry_run` is true, no actual changes are made.
    ///
    /// Returns the notification state for the merge.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn merge_prop_diffs(
        &self,
        path: &str,
        propchanges: &[PropChange],
        base_merge: bool,
        dry_run: bool,
    ) -> Result<u32, crate::Error<'static>> {
        let path_cstr = crate::dirent::to_absolute_cstring(path)?;
        let mut state: subversion_sys::svn_wc_notify_state_t = 0;
        with_tmp_pool(|scratch_pool| {
            // Build apr_array of svn_prop_t
            let arr = unsafe {
                apr_sys::apr_array_make(
                    scratch_pool.as_mut_ptr(),
                    propchanges.len() as i32,
                    std::mem::size_of::<subversion_sys::svn_prop_t>() as i32,
                )
            };
            for pc in propchanges {
                let name_cstr = std::ffi::CString::new(pc.name.as_str()).unwrap();
                let name_ptr =
                    unsafe { apr_sys::apr_pstrdup(scratch_pool.as_mut_ptr(), name_cstr.as_ptr()) };
                let value_ptr = match &pc.value {
                    Some(v) => {
                        let svn_str = crate::string::BStr::from_bytes(v, scratch_pool);
                        svn_str.as_ptr() as *const subversion_sys::svn_string_t
                    }
                    None => std::ptr::null(),
                };
                let prop = subversion_sys::svn_prop_t {
                    name: name_ptr,
                    value: value_ptr,
                };
                unsafe {
                    let dest = apr_sys::apr_array_push(arr) as *mut subversion_sys::svn_prop_t;
                    *dest = prop;
                }
            }
            let err = unsafe {
                subversion_sys::svn_wc_merge_prop_diffs(
                    &mut state,
                    path_cstr.as_ptr(),
                    self.ptr,
                    arr,
                    if base_merge { 1 } else { 0 },
                    if dry_run { 1 } else { 0 },
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)?;
            Ok(state as u32)
        })
    }

    /// Get a translated version of a file.
    ///
    /// Returns the path to a translated copy of `src`, using the eol-style
    /// and keyword properties of `versioned_file`.
    ///
    /// Flags are a bitmask of `SVN_WC_TRANSLATE_*` constants.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn translated_file(
        &self,
        src: &str,
        versioned_file: &str,
        flags: u32,
    ) -> Result<String, crate::Error<'static>> {
        let src_cstr = crate::dirent::to_absolute_cstring(src)?;
        let versioned_cstr = crate::dirent::to_absolute_cstring(versioned_file)?;
        let mut xlated_path: *const std::ffi::c_char = std::ptr::null();
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_translated_file2(
                    &mut xlated_path,
                    src_cstr.as_ptr(),
                    versioned_cstr.as_ptr(),
                    self.ptr,
                    flags,
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)?;
            Ok(unsafe { std::ffi::CStr::from_ptr(xlated_path) }
                .to_str()
                .expect("path should be valid UTF-8")
                .to_string())
        })
    }

    /// Relocate a working copy from one repository root to another.
    ///
    /// `from` and `to` are the old and new repository URL prefixes.
    /// If `recurse` is true, the relocation applies recursively.
    /// The `validator` function is called with `(uuid, url, root_url)` for each
    /// relocated entry to validate the new URL.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn relocate(
        &self,
        path: &str,
        from: &str,
        to: &str,
        recurse: bool,
        validator: Option<&dyn Fn(&str, &str, &str) -> Result<(), crate::Error<'static>>>,
    ) -> Result<(), crate::Error<'static>> {
        let path_cstr = crate::dirent::to_absolute_cstring(path)?;
        let from_cstr = std::ffi::CString::new(from)?;
        let to_cstr = std::ffi::CString::new(to)?;

        unsafe extern "C" fn validator_trampoline(
            baton: *mut std::ffi::c_void,
            uuid: *const std::ffi::c_char,
            url: *const std::ffi::c_char,
            root_url: *const std::ffi::c_char,
            _pool: *mut apr_sys::apr_pool_t,
        ) -> *mut subversion_sys::svn_error_t {
            let validator =
                &*(baton as *const &dyn Fn(&str, &str, &str) -> Result<(), crate::Error<'static>>);
            let uuid_str = std::ffi::CStr::from_ptr(uuid).to_str().unwrap();
            let url_str = std::ffi::CStr::from_ptr(url).to_str().unwrap();
            let root_str = std::ffi::CStr::from_ptr(root_url).to_str().unwrap();
            match validator(uuid_str, url_str, root_str) {
                Ok(()) => std::ptr::null_mut(),
                Err(e) => e.into_raw(),
            }
        }

        unsafe extern "C" fn noop_validator(
            _baton: *mut std::ffi::c_void,
            _uuid: *const std::ffi::c_char,
            _url: *const std::ffi::c_char,
            _root_url: *const std::ffi::c_char,
            _pool: *mut apr_sys::apr_pool_t,
        ) -> *mut subversion_sys::svn_error_t {
            std::ptr::null_mut()
        }

        let (func, baton): (
            subversion_sys::svn_wc_relocation_validator3_t,
            *mut std::ffi::c_void,
        ) = match &validator {
            Some(v) => (
                Some(validator_trampoline),
                v as *const _ as *mut std::ffi::c_void,
            ),
            None => (Some(noop_validator), std::ptr::null_mut()),
        };

        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_relocate3(
                    path_cstr.as_ptr(),
                    self.ptr,
                    from_cstr.as_ptr(),
                    to_cstr.as_ptr(),
                    if recurse { 1 } else { 0 },
                    func,
                    baton,
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })
    }

    /// Get a single entry from the working copy.
    ///
    /// Returns `None` if the path is not versioned.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn entry(
        &self,
        path: &str,
        show_hidden: bool,
    ) -> Result<Option<Entry>, crate::Error<'static>> {
        let path_cstr = crate::dirent::to_absolute_cstring(path)?;
        let mut entry_ptr: *const subversion_sys::svn_wc_entry_t = std::ptr::null();
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_entry(
                    &mut entry_ptr,
                    path_cstr.as_ptr(),
                    self.ptr,
                    if show_hidden { 1 } else { 0 },
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)?;
            if entry_ptr.is_null() {
                Ok(None)
            } else {
                Ok(Some(unsafe { Entry::from_raw(entry_ptr) }))
            }
        })
    }

    /// Read all entries in this directory.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn entries_read(
        &self,
        show_hidden: bool,
    ) -> Result<HashMap<String, Entry>, crate::Error<'static>> {
        let mut entries: *mut apr::hash::apr_hash_t = std::ptr::null_mut();
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_entries_read(
                    &mut entries,
                    self.ptr,
                    if show_hidden { 1 } else { 0 },
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)?;
            let mut result = HashMap::new();
            if !entries.is_null() {
                let mut hi = unsafe { apr_sys::apr_hash_first(scratch_pool.as_mut_ptr(), entries) };
                while !hi.is_null() {
                    let mut key: *const std::ffi::c_void = std::ptr::null();
                    let mut val: *mut std::ffi::c_void = std::ptr::null_mut();
                    let mut klen: apr_sys::apr_ssize_t = 0;
                    unsafe { apr_sys::apr_hash_this(hi, &mut key, &mut klen, &mut val) };
                    let name = unsafe { std::ffi::CStr::from_ptr(key as *const std::ffi::c_char) }
                        .to_str()
                        .expect("entry name should be valid UTF-8")
                        .to_string();
                    let entry =
                        unsafe { Entry::from_raw(val as *const subversion_sys::svn_wc_entry_t) };
                    result.insert(name, entry);
                    hi = unsafe { apr_sys::apr_hash_next(hi) };
                }
            }
            Ok(result)
        })
    }

    /// Get the status of a path (deprecated version 2 API).
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn status(&self, path: &str) -> Result<Status, crate::Error<'static>> {
        let path_cstr = crate::dirent::to_absolute_cstring(path)?;
        let mut status_ptr: *mut subversion_sys::svn_wc_status2_t = std::ptr::null_mut();
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_status2(
                    &mut status_ptr,
                    path_cstr.as_ptr(),
                    self.ptr,
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)?;
            Ok(unsafe { Status::from_raw(status_ptr) })
        })
    }

    /// Walk entries in the working copy, calling `callback` for each entry.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn walk_entries<F>(
        &self,
        path: &str,
        mut callback: F,
        depth: crate::Depth,
        show_hidden: bool,
    ) -> Result<(), crate::Error<'static>>
    where
        F: FnMut(&str, &Entry) -> Result<(), crate::Error<'static>>,
    {
        let path_cstr = crate::dirent::to_absolute_cstring(path)?;

        unsafe extern "C" fn found_entry_trampoline<F>(
            path: *const std::ffi::c_char,
            entry: *const subversion_sys::svn_wc_entry_t,
            walk_baton: *mut std::ffi::c_void,
            _pool: *mut apr_sys::apr_pool_t,
        ) -> *mut subversion_sys::svn_error_t
        where
            F: FnMut(&str, &Entry) -> Result<(), crate::Error<'static>>,
        {
            let callback = &mut *(walk_baton as *mut F);
            let path_str = std::ffi::CStr::from_ptr(path).to_str().unwrap();
            let entry = Entry::from_raw(entry);
            match callback(path_str, &entry) {
                Ok(()) => std::ptr::null_mut(),
                Err(e) => e.into_raw(),
            }
        }

        unsafe extern "C" fn handle_error_trampoline(
            _path: *const std::ffi::c_char,
            err: *mut subversion_sys::svn_error_t,
            _walk_baton: *mut std::ffi::c_void,
            _pool: *mut apr_sys::apr_pool_t,
        ) -> *mut subversion_sys::svn_error_t {
            // Propagate the error as-is
            err
        }

        let callbacks = subversion_sys::svn_wc_entry_callbacks2_t {
            found_entry: Some(found_entry_trampoline::<F>),
            handle_error: Some(handle_error_trampoline),
        };
        let baton = &mut callback as *mut F as *mut std::ffi::c_void;

        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_walk_entries3(
                    path_cstr.as_ptr(),
                    self.ptr,
                    &callbacks,
                    baton,
                    depth.into(),
                    if show_hidden { 1 } else { 0 },
                    None,                 // cancel_func
                    std::ptr::null_mut(), // cancel_baton
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })
    }

    /// Crawl working copy revisions, reporting to a reporter.
    #[cfg(feature = "ra")]
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn crawl_revisions(
        &self,
        path: &str,
        reporter: &crate::ra::WrapReporter,
        restore_files: bool,
        depth: crate::Depth,
        honor_depth_exclude: bool,
        depth_compatibility_trick: bool,
        use_commit_times: bool,
        notify_func: Option<&dyn Fn(&Notify)>,
    ) -> Result<(), crate::Error<'static>> {
        let path_cstr = crate::dirent::to_absolute_cstring(path)?;
        let notify_baton = notify_func
            .map(|f| box_notify_baton_borrowed(f))
            .unwrap_or(std::ptr::null_mut());

        let result = with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_crawl_revisions4(
                    path_cstr.as_ptr(),
                    self.ptr,
                    reporter.as_ptr(),
                    reporter.as_baton(),
                    if restore_files { 1 } else { 0 },
                    depth.into(),
                    if honor_depth_exclude { 1 } else { 0 },
                    if depth_compatibility_trick { 1 } else { 0 },
                    if use_commit_times { 1 } else { 0 },
                    if notify_func.is_some() {
                        Some(wrap_notify_func)
                    } else {
                        None
                    },
                    notify_baton,
                    std::ptr::null_mut(), // traversal_info
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        });

        if !notify_baton.is_null() {
            unsafe { drop_notify_baton_borrowed(notify_baton) };
        }

        result
    }

    /// Transmit local text changes through a delta editor.
    ///
    /// Returns `(tempfile_path, md5_digest)` where `md5_digest` is 16 bytes.
    #[cfg(feature = "delta")]
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn transmit_text_deltas(
        &self,
        path: &str,
        fulltext: bool,
        file_editor: &crate::delta::WrapFileEditor<'_>,
    ) -> Result<(Option<String>, [u8; 16]), crate::Error<'static>> {
        let path_cstr = crate::dirent::to_absolute_cstring(path)?;
        let mut tempfile: *const std::ffi::c_char = std::ptr::null();
        let mut digest = [0u8; 16]; // APR_MD5_DIGESTSIZE
        let (editor, file_baton) = file_editor.as_raw_parts();
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_transmit_text_deltas2(
                    &mut tempfile,
                    digest.as_mut_ptr(),
                    path_cstr.as_ptr(),
                    self.ptr,
                    if fulltext { 1 } else { 0 },
                    editor,
                    file_baton,
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)?;
            let tempfile_str = unsafe { cstr_to_option(tempfile) };
            Ok((tempfile_str, digest))
        })
    }

    /// Transmit local property changes through a delta editor.
    ///
    /// Looks up the entry for `path` internally.
    #[cfg(feature = "delta")]
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn transmit_prop_deltas(
        &self,
        path: &str,
        file_editor: &crate::delta::WrapFileEditor<'_>,
    ) -> Result<Option<String>, crate::Error<'static>> {
        let path_cstr = crate::dirent::to_absolute_cstring(path)?;
        let mut tempfile: *const std::ffi::c_char = std::ptr::null();
        let (editor, baton) = file_editor.as_raw_parts();
        with_tmp_pool(|scratch_pool| {
            // Look up the entry to pass to the C function
            let mut entry_ptr: *const subversion_sys::svn_wc_entry_t = std::ptr::null();
            let err = unsafe {
                subversion_sys::svn_wc_entry(
                    &mut entry_ptr,
                    path_cstr.as_ptr(),
                    self.ptr,
                    0, // show_hidden = false
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)?;
            if entry_ptr.is_null() {
                return Err(crate::Error::with_raw_status(
                    subversion_sys::svn_errno_t_SVN_ERR_ENTRY_NOT_FOUND as i32,
                    None,
                    &format!("No entry for '{path}'"),
                ));
            }
            let err = unsafe {
                subversion_sys::svn_wc_transmit_prop_deltas(
                    path_cstr.as_ptr(),
                    self.ptr,
                    entry_ptr,
                    editor,
                    baton,
                    &mut tempfile,
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)?;
            Ok(unsafe { cstr_to_option(tempfile) })
        })
    }

    /// Perform a 3-way file merge.
    ///
    /// Returns the merge outcome.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn merge(
        &self,
        left: &str,
        right: &str,
        merge_target: &str,
        left_label: Option<&str>,
        right_label: Option<&str>,
        target_label: Option<&str>,
        dry_run: bool,
        diff3_cmd: Option<&str>,
        merge_options: &[&str],
        prop_diff: &[PropChange],
    ) -> Result<u32, crate::Error<'static>> {
        let left_cstr = crate::dirent::to_absolute_cstring(left)?;
        let right_cstr = crate::dirent::to_absolute_cstring(right)?;
        let target_cstr = crate::dirent::to_absolute_cstring(merge_target)?;
        let left_label_cstr = left_label.map(std::ffi::CString::new).transpose()?;
        let right_label_cstr = right_label.map(std::ffi::CString::new).transpose()?;
        let target_label_cstr = target_label.map(std::ffi::CString::new).transpose()?;
        let diff3_cmd_cstr = diff3_cmd.map(std::ffi::CString::new).transpose()?;
        let mut merge_outcome: subversion_sys::svn_wc_merge_outcome_t = 0;

        with_tmp_pool(|scratch_pool| {
            // Build merge_options array
            let merge_opts_arr = if merge_options.is_empty() {
                std::ptr::null()
            } else {
                let arr = unsafe {
                    apr_sys::apr_array_make(
                        scratch_pool.as_mut_ptr(),
                        merge_options.len() as i32,
                        std::mem::size_of::<*const std::ffi::c_char>() as i32,
                    )
                };
                for opt in merge_options {
                    let cstr = std::ffi::CString::new(*opt).unwrap();
                    let ptr =
                        unsafe { apr_sys::apr_pstrdup(scratch_pool.as_mut_ptr(), cstr.as_ptr()) };
                    unsafe {
                        let dest = apr_sys::apr_array_push(arr) as *mut *const std::ffi::c_char;
                        *dest = ptr;
                    }
                }
                arr as *const _
            };

            // Build prop_diff array
            let prop_diff_arr = unsafe {
                apr_sys::apr_array_make(
                    scratch_pool.as_mut_ptr(),
                    prop_diff.len() as i32,
                    std::mem::size_of::<subversion_sys::svn_prop_t>() as i32,
                )
            };
            for pc in prop_diff {
                let name_cstr = std::ffi::CString::new(pc.name.as_str()).unwrap();
                let name_ptr =
                    unsafe { apr_sys::apr_pstrdup(scratch_pool.as_mut_ptr(), name_cstr.as_ptr()) };
                let value_ptr = match &pc.value {
                    Some(v) => {
                        let svn_str = crate::string::BStr::from_bytes(v, scratch_pool);
                        svn_str.as_ptr() as *const subversion_sys::svn_string_t
                    }
                    None => std::ptr::null(),
                };
                unsafe {
                    let dest =
                        apr_sys::apr_array_push(prop_diff_arr) as *mut subversion_sys::svn_prop_t;
                    *dest = subversion_sys::svn_prop_t {
                        name: name_ptr,
                        value: value_ptr,
                    };
                }
            }

            let err = unsafe {
                subversion_sys::svn_wc_merge3(
                    &mut merge_outcome,
                    left_cstr.as_ptr(),
                    right_cstr.as_ptr(),
                    target_cstr.as_ptr(),
                    self.ptr,
                    left_label_cstr
                        .as_ref()
                        .map_or(std::ptr::null(), |c| c.as_ptr()),
                    right_label_cstr
                        .as_ref()
                        .map_or(std::ptr::null(), |c| c.as_ptr()),
                    target_label_cstr
                        .as_ref()
                        .map_or(std::ptr::null(), |c| c.as_ptr()),
                    if dry_run { 1 } else { 0 },
                    diff3_cmd_cstr
                        .as_ref()
                        .map_or(std::ptr::null(), |c| c.as_ptr()),
                    merge_opts_arr,
                    prop_diff_arr,
                    None,                 // conflict_func
                    std::ptr::null_mut(), // conflict_baton
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)?;
            Ok(merge_outcome as u32)
        })
    }

    /// Merge properties into a path, with base properties.
    ///
    /// Returns the notification state for the merge.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn merge_props(
        &self,
        path: &str,
        baseprops: &HashMap<String, Vec<u8>>,
        propchanges: &[PropChange],
        base_merge: bool,
        dry_run: bool,
    ) -> Result<u32, crate::Error<'static>> {
        let path_cstr = crate::dirent::to_absolute_cstring(path)?;
        let mut state: subversion_sys::svn_wc_notify_state_t = 0;
        with_tmp_pool(|scratch_pool| {
            // Build baseprops hash
            let baseprops_hash = unsafe { apr_sys::apr_hash_make(scratch_pool.as_mut_ptr()) };
            for (name, value) in baseprops {
                let name_cstr = std::ffi::CString::new(name.as_str()).unwrap();
                let name_ptr =
                    unsafe { apr_sys::apr_pstrdup(scratch_pool.as_mut_ptr(), name_cstr.as_ptr()) };
                let svn_str = crate::string::BStr::from_bytes(value, scratch_pool);
                unsafe {
                    apr_sys::apr_hash_set(
                        baseprops_hash,
                        name_ptr as *const _,
                        apr_sys::APR_HASH_KEY_STRING as apr_sys::apr_ssize_t,
                        svn_str.as_ptr() as *const _,
                    );
                }
            }

            // Build propchanges array
            let arr = unsafe {
                apr_sys::apr_array_make(
                    scratch_pool.as_mut_ptr(),
                    propchanges.len() as i32,
                    std::mem::size_of::<subversion_sys::svn_prop_t>() as i32,
                )
            };
            for pc in propchanges {
                let name_cstr = std::ffi::CString::new(pc.name.as_str()).unwrap();
                let name_ptr =
                    unsafe { apr_sys::apr_pstrdup(scratch_pool.as_mut_ptr(), name_cstr.as_ptr()) };
                let value_ptr = match &pc.value {
                    Some(v) => {
                        let svn_str = crate::string::BStr::from_bytes(v, scratch_pool);
                        svn_str.as_ptr() as *const subversion_sys::svn_string_t
                    }
                    None => std::ptr::null(),
                };
                unsafe {
                    let dest = apr_sys::apr_array_push(arr) as *mut subversion_sys::svn_prop_t;
                    *dest = subversion_sys::svn_prop_t {
                        name: name_ptr,
                        value: value_ptr,
                    };
                }
            }

            let err = unsafe {
                subversion_sys::svn_wc_merge_props2(
                    &mut state,
                    path_cstr.as_ptr(),
                    self.ptr,
                    baseprops_hash,
                    arr,
                    if base_merge { 1 } else { 0 },
                    if dry_run { 1 } else { 0 },
                    None,                 // conflict_func
                    std::ptr::null_mut(), // conflict_baton
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)?;
            Ok(state as u32)
        })
    }

    /// Try to obtain an access baton for a path, using this baton as the
    /// associated (parent) baton.
    ///
    /// Returns `None` if the path is not a versioned directory.
    /// The returned baton is tied to this baton's lifetime.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn probe_try(
        &mut self,
        path: &str,
        write_lock: bool,
        levels_to_lock: i32,
    ) -> Result<Option<Adm<'_>>, crate::Error<'static>> {
        let path_cstr = crate::dirent::to_absolute_cstring(path)?;
        let mut adm_access: *mut subversion_sys::svn_wc_adm_access_t = std::ptr::null_mut();
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_adm_probe_try3(
                    &mut adm_access,
                    self.ptr,
                    path_cstr.as_ptr(),
                    if write_lock { 1 } else { 0 },
                    levels_to_lock,
                    None,                 // cancel_func
                    std::ptr::null_mut(), // cancel_baton
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)?;
            if adm_access.is_null() {
                Ok(None)
            } else {
                Ok(Some(Adm {
                    ptr: adm_access,
                    _pool: None,
                    _marker: std::marker::PhantomData,
                }))
            }
        })
    }

    /// Queue a path for post-commit processing using this access baton.
    ///
    /// This calls the deprecated `svn_wc_queue_committed` which takes
    /// an `svn_wc_adm_access_t` rather than an `svn_wc_context_t`.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn queue_committed(
        &self,
        path: &str,
        committed_queue: &mut CommittedQueue,
        recurse: bool,
        remove_lock: bool,
        remove_changelist: bool,
        digest: Option<&[u8; 16]>,
    ) -> Result<(), crate::Error<'static>> {
        let path_cstr = crate::dirent::to_absolute_cstring(path)?;
        let mut queue_ptr = committed_queue.as_mut_ptr();
        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_queue_committed(
                    &mut queue_ptr,
                    path_cstr.as_ptr(),
                    self.ptr,
                    if recurse { 1 } else { 0 },
                    std::ptr::null(), // wcprop_changes
                    if remove_lock { 1 } else { 0 },
                    if remove_changelist { 1 } else { 0 },
                    digest.map(|d| d.as_ptr()).unwrap_or(std::ptr::null()),
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })
    }

    /// Process a committed queue using this access baton.
    ///
    /// This calls the deprecated `svn_wc_process_committed_queue` which takes
    /// an `svn_wc_adm_access_t` rather than an `svn_wc_context_t`.
    #[deprecated(note = "Use svn_wc_context_t based APIs where possible")]
    pub fn process_committed_queue(
        &self,
        committed_queue: &mut CommittedQueue,
        new_revnum: crate::Revnum,
        rev_date: Option<&str>,
        rev_author: Option<&str>,
    ) -> Result<(), crate::Error<'static>> {
        let rev_date_cstr = rev_date.map(std::ffi::CString::new).transpose()?;
        let rev_author_cstr = rev_author.map(std::ffi::CString::new).transpose()?;

        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_wc_process_committed_queue(
                    committed_queue.as_mut_ptr(),
                    self.ptr,
                    new_revnum.0,
                    rev_date_cstr
                        .as_ref()
                        .map_or(std::ptr::null(), |s| s.as_ptr()),
                    rev_author_cstr
                        .as_ref()
                        .map_or(std::ptr::null(), |s| s.as_ptr()),
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })
    }
}

#[allow(deprecated)]
impl Adm<'_> {
    /// Explicitly close the access baton, releasing all resources and locks.
    pub fn close(&mut self) {
        if !self.ptr.is_null() {
            match &mut self._pool {
                Some(pool) => unsafe {
                    subversion_sys::svn_wc_adm_close2(self.ptr, pool.as_mut_ptr());
                },
                None => {
                    with_tmp_pool(|scratch_pool| unsafe {
                        subversion_sys::svn_wc_adm_close2(self.ptr, scratch_pool.as_mut_ptr());
                    });
                }
            }
            self.ptr = std::ptr::null_mut();
        }
    }
}

#[allow(deprecated)]
impl Drop for Adm<'_> {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(deprecated)]
    fn test_add_repos_file() {
        use std::io::Cursor;

        let temp_dir = tempfile::tempdir().unwrap();
        let wc_path = temp_dir.path().join("wc");
        let repo_path = temp_dir.path().join("repo");

        // Create repo and checkout
        crate::repos::Repos::create(&repo_path).unwrap();
        let repo_url = format!("file://{}", repo_path.display());
        std::process::Command::new("svn")
            .args(["checkout", &repo_url, wc_path.to_str().unwrap()])
            .output()
            .unwrap();

        // Create a file on disk
        let file_path = wc_path.join("test.txt");
        let content = b"hello world";
        std::fs::write(&file_path, content).unwrap();

        // Add via Adm.add_repos_file
        let mut adm = Adm::open(wc_path.to_str().unwrap(), true, -1).unwrap();

        let backend = crate::io::ReadOnlyBackend::new(Cursor::new(content.to_vec()));
        let mut stream = crate::io::Stream::from_backend(backend).unwrap();
        let base_props = std::collections::HashMap::new();

        adm.add_repos_file(
            file_path.to_str().unwrap(),
            &mut stream,
            None,
            &base_props,
            None,
            None,
            None,
        )
        .unwrap();

        adm.close();
    }

    #[test]
    #[allow(deprecated)]
    fn test_probe_try_versioned_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let wc_path = temp_dir.path().join("wc");
        let repo_path = temp_dir.path().join("repo");

        // Create repo and checkout
        crate::repos::Repos::create(&repo_path).unwrap();
        let repo_url = format!("file://{}", repo_path.display());
        std::process::Command::new("svn")
            .args(["checkout", &repo_url, wc_path.to_str().unwrap()])
            .output()
            .unwrap();

        // Create and add a subdirectory
        let sub_dir = wc_path.join("subdir");
        std::fs::create_dir(&sub_dir).unwrap();
        std::process::Command::new("svn")
            .args(["add", sub_dir.to_str().unwrap()])
            .output()
            .unwrap();

        let mut adm = Adm::open(wc_path.to_str().unwrap(), true, -1).unwrap();

        // probe_try on the versioned subdirectory should return Some
        let sub_adm = adm.probe_try(sub_dir.to_str().unwrap(), true, 0).unwrap();
        assert!(sub_adm.is_some());

        // Drop sub_adm before parent
        drop(sub_adm);
        adm.close();
    }

    #[test]
    #[allow(deprecated)]
    fn test_probe_try_unversioned_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let wc_path = temp_dir.path().join("wc");
        let repo_path = temp_dir.path().join("repo");

        // Create repo and checkout
        crate::repos::Repos::create(&repo_path).unwrap();
        let repo_url = format!("file://{}", repo_path.display());
        std::process::Command::new("svn")
            .args(["checkout", &repo_url, wc_path.to_str().unwrap()])
            .output()
            .unwrap();

        let mut adm = Adm::open(wc_path.to_str().unwrap(), true, -1).unwrap();

        // probe_try on an unversioned file probes the closest versioned
        // directory, which is the WC root itself — so it returns Some.
        let probed = adm
            .probe_try(wc_path.join("nonexistent").to_str().unwrap(), false, 0)
            .unwrap();
        assert!(probed.is_some());
        drop(probed);

        adm.close();
    }
}
