use crate::config::Config;
use crate::delta::Editor;
use crate::generated::svn_ra_session_t;
use crate::{Depth, Error, Revnum};
use apr::pool::{Pool, PooledPtr};
use std::collections::HashMap;

pub struct Session(PooledPtr<svn_ra_session_t>);
unsafe impl Send for Session {}

pub(crate) extern "C" fn wrap_dirent_receiver(
    rel_path: *const std::os::raw::c_char,
    dirent: *mut crate::generated::svn_dirent_t,
    baton: *mut std::os::raw::c_void,
    pool: *mut apr::apr_pool_t,
) -> *mut crate::generated::svn_error_t {
    let rel_path = unsafe { std::ffi::CStr::from_ptr(rel_path) };
    let baton = unsafe {
        &*(baton as *const _ as *const &dyn Fn(&str, &Dirent) -> Result<(), crate::Error>)
    };
    let pool = Pool::from_raw(pool);
    match baton(
        rel_path.to_str().unwrap(),
        &Dirent(unsafe { PooledPtr::in_pool(std::rc::Rc::new(pool), dirent) }),
    ) {
        Ok(()) => std::ptr::null_mut(),
        Err(mut e) => e.as_mut_ptr(),
    }
}

impl Session {
    pub fn open(
        url: &str,
        uuid: Option<&str>,
        mut callbacks: Option<&mut Callbacks>,
        mut config: Option<&mut Config>,
    ) -> Result<(Self, Option<String>, Option<String>), Error> {
        let url = std::ffi::CString::new(url).unwrap();
        let mut corrected_url = std::ptr::null();
        let mut redirect_url = std::ptr::null();
        let mut pool = Pool::new();
        let mut session = std::ptr::null_mut();
        let uuid = uuid.map(|uuid| std::ffi::CString::new(uuid).unwrap());
        let err = unsafe {
            crate::generated::svn_ra_open5(
                &mut session,
                &mut corrected_url,
                &mut redirect_url,
                url.as_ptr(),
                if let Some(uuid) = uuid {
                    uuid.as_ptr()
                } else {
                    std::ptr::null()
                },
                if let Some(callbacks) = callbacks.as_mut() {
                    callbacks.as_mut_ptr()
                } else {
                    Callbacks::default().as_mut_ptr()
                },
                std::ptr::null_mut(),
                if let Some(config) = config.as_mut() {
                    config.as_mut_ptr()
                } else {
                    std::ptr::null_mut()
                },
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok((
            Self::from_raw(unsafe { PooledPtr::in_pool(std::rc::Rc::new(pool), session) }),
            if corrected_url.is_null() {
                None
            } else {
                Some(
                    unsafe { std::ffi::CStr::from_ptr(corrected_url) }
                        .to_str()
                        .unwrap()
                        .to_string(),
                )
            },
            if redirect_url.is_null() {
                None
            } else {
                Some(
                    unsafe { std::ffi::CStr::from_ptr(redirect_url) }
                        .to_str()
                        .unwrap()
                        .to_string(),
                )
            },
        ))
    }

    pub fn reparent(&mut self, url: &str) -> Result<(), Error> {
        let url = std::ffi::CString::new(url).unwrap();
        let mut pool = Pool::new();
        let err = unsafe {
            crate::generated::svn_ra_reparent(self.0.as_mut_ptr(), url.as_ptr(), pool.as_mut_ptr())
        };
        Error::from_raw(err)?;
        Ok(())
    }

    pub fn from_raw(raw: PooledPtr<svn_ra_session_t>) -> Self {
        Self(raw)
    }

    pub fn get_session_url(&mut self) -> Result<String, Error> {
        let mut pool = Pool::new();
        let mut url = std::ptr::null();
        let err = unsafe {
            crate::generated::svn_ra_get_session_url(
                self.0.as_mut_ptr(),
                &mut url,
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        let url = unsafe { std::ffi::CStr::from_ptr(url) };
        Ok(url.to_string_lossy().into_owned())
    }

    pub fn get_path_relative_to_session(&mut self, url: &str) -> Result<String, Error> {
        let url = std::ffi::CString::new(url).unwrap();
        let mut pool = Pool::new();
        let mut path = std::ptr::null();
        let err = unsafe {
            crate::generated::svn_ra_get_path_relative_to_session(
                self.0.as_mut_ptr(),
                &mut path,
                url.as_ptr(),
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        let path = unsafe { std::ffi::CStr::from_ptr(path) };
        Ok(path.to_string_lossy().into_owned())
    }

    pub fn get_path_relative_to_root(&mut self, url: &str) -> String {
        let url = std::ffi::CString::new(url).unwrap();
        let mut pool = Pool::new();
        let mut path = std::ptr::null();
        unsafe {
            crate::generated::svn_ra_get_path_relative_to_root(
                self.0.as_mut_ptr(),
                &mut path,
                url.as_ptr(),
                pool.as_mut_ptr(),
            );
        }
        let path = unsafe { std::ffi::CStr::from_ptr(path) };
        path.to_string_lossy().into_owned()
    }

    pub fn get_latest_revnum(&mut self) -> Result<Revnum, Error> {
        let mut pool = Pool::new();
        let mut revnum = 0;
        let err = unsafe {
            crate::generated::svn_ra_get_latest_revnum(
                self.0.as_mut_ptr(),
                &mut revnum,
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(revnum)
    }

    pub fn get_dated_revision(&mut self, tm: impl apr::time::IntoTime) -> Result<Revnum, Error> {
        let mut pool = Pool::new();
        let mut revnum = 0;
        let err = unsafe {
            crate::generated::svn_ra_get_dated_revision(
                self.0.as_mut_ptr(),
                &mut revnum,
                tm.as_apr_time().into(),
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(revnum)
    }

    pub fn change_revprop(
        &mut self,
        rev: Revnum,
        name: &str,
        old_value: Option<&[u8]>,
        new_value: &[u8],
    ) -> Result<(), Error> {
        let name = std::ffi::CString::new(name).unwrap();
        let mut pool = Pool::new();
        let new_value = crate::generated::svn_string_t {
            data: new_value.as_ptr() as *mut _,
            len: new_value.len(),
        };
        let old_value = old_value.map(|v| crate::generated::svn_string_t {
            data: v.as_ptr() as *mut _,
            len: v.len(),
        });
        let err = unsafe {
            crate::generated::svn_ra_change_rev_prop2(
                self.0.as_mut_ptr(),
                rev,
                name.as_ptr(),
                &old_value.map_or(std::ptr::null(), |v| &v),
                &new_value,
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(())
    }

    pub fn rev_proplist(&mut self, rev: Revnum) -> Result<HashMap<String, Vec<u8>>, Error> {
        let mut pool = Pool::new();
        let mut props = std::ptr::null_mut();
        let err = unsafe {
            crate::generated::svn_ra_rev_proplist(
                self.0.as_mut_ptr(),
                rev,
                &mut props,
                pool.as_mut_ptr(),
            )
        };
        let mut hash =
            apr::hash::Hash::<&str, *const crate::generated::svn_string_t>::from_raw(unsafe {
                PooledPtr::in_pool(std::rc::Rc::new(pool), props)
            });
        Error::from_raw(err)?;
        Ok(hash
            .iter()
            .map(|(k, v)| {
                (
                    String::from_utf8_lossy(k).into_owned(),
                    Vec::from(unsafe {
                        std::slice::from_raw_parts((**v).data as *const u8, (**v).len)
                    }),
                )
            })
            .collect())
    }

    pub fn rev_prop(&mut self, rev: Revnum, name: &str) -> Result<Option<Vec<u8>>, Error> {
        let name = std::ffi::CString::new(name).unwrap();
        let mut pool = Pool::new();
        let mut value = std::ptr::null_mut();
        let err = unsafe {
            crate::generated::svn_ra_rev_prop(
                self.0.as_mut_ptr(),
                rev,
                name.as_ptr(),
                &mut value,
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        if value.is_null() {
            Ok(None)
        } else {
            Ok(Some(Vec::from(unsafe {
                std::slice::from_raw_parts((*value).data as *const u8, (*value).len)
            })))
        }
    }

    pub fn get_commit_editor(
        &mut self,
        revprop_table: HashMap<String, Vec<u8>>,
        commit_callback: &dyn FnMut(&crate::CommitInfo) -> Result<(), Error>,
        lock_tokens: HashMap<String, String>,
        keep_locks: bool,
    ) -> Result<Box<dyn Editor>, Error> {
        let pool = std::rc::Rc::new(Pool::new());
        let commit_callback = Box::into_raw(Box::new(commit_callback));
        let mut editor = std::ptr::null();
        let mut edit_baton = std::ptr::null_mut();
        let mut hash_revprop_table =
            apr::hash::Hash::<&str, *const crate::generated::svn_string_t>::in_pool(&pool);
        for (k, v) in revprop_table.iter() {
            let v: crate::string::String = v.as_slice().into();
            hash_revprop_table.set(k, &v.as_ptr());
        }
        let mut hash_lock_tokens =
            apr::hash::Hash::<&str, *const std::os::raw::c_char>::in_pool(&pool);
        for (k, v) in lock_tokens.iter() {
            hash_lock_tokens.set(k, &(v.as_ptr() as *const _));
        }
        let mut result_pool = Pool::new();
        let err = unsafe {
            crate::generated::svn_ra_get_commit_editor3(
                self.0.as_mut_ptr(),
                &mut editor,
                &mut edit_baton,
                hash_revprop_table.as_mut_ptr(),
                Some(crate::client::wrap_commit_callback2),
                commit_callback as *mut _ as *mut _,
                hash_lock_tokens.as_mut_ptr(),
                keep_locks.into(),
                result_pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(Box::new(crate::delta::WrapEditor(editor, unsafe {
            PooledPtr::in_pool(std::rc::Rc::new(result_pool), edit_baton)
        })))
    }

    pub fn get_file(
        &mut self,
        path: &str,
        rev: Revnum,
        mut stream: crate::io::Stream,
    ) -> Result<(Revnum, HashMap<String, Vec<u8>>), Error> {
        let path = std::ffi::CString::new(path).unwrap();
        let mut pool = Pool::new();
        let mut props = std::ptr::null_mut();
        let mut fetched_rev = 0;
        let err = unsafe {
            crate::generated::svn_ra_get_file(
                self.0.as_mut_ptr(),
                path.as_ptr(),
                rev,
                stream.as_mut_ptr(),
                &mut fetched_rev,
                &mut props,
                pool.as_mut_ptr(),
            )
        };
        crate::Error::from_raw(err)?;
        let mut hash =
            apr::hash::Hash::<&str, *const crate::generated::svn_string_t>::from_raw(unsafe {
                PooledPtr::in_pool(std::rc::Rc::new(pool), props)
            });
        Ok((
            fetched_rev,
            hash.iter()
                .map(|(k, v)| {
                    (
                        String::from_utf8_lossy(k).into_owned(),
                        Vec::from(unsafe {
                            std::slice::from_raw_parts((**v).data as *const u8, (**v).len)
                        }),
                    )
                })
                .collect(),
        ))
    }

    pub fn get_dir(
        &mut self,
        path: &str,
        rev: Revnum,
    ) -> Result<(Revnum, HashMap<String, Dirent>, HashMap<String, Vec<u8>>), Error> {
        let path = std::ffi::CString::new(path).unwrap();
        let mut pool = Pool::new();
        let mut props = std::ptr::null_mut();
        let mut fetched_rev = 0;
        let mut dirents = std::ptr::null_mut();
        let dirent_fields = crate::generated::SVN_DIRENT_KIND
            | crate::generated::SVN_DIRENT_SIZE
            | crate::generated::SVN_DIRENT_HAS_PROPS
            | crate::generated::SVN_DIRENT_CREATED_REV
            | crate::generated::SVN_DIRENT_TIME
            | crate::generated::SVN_DIRENT_LAST_AUTHOR;
        let err = unsafe {
            crate::generated::svn_ra_get_dir2(
                self.0.as_mut_ptr(),
                &mut dirents,
                &mut fetched_rev,
                &mut props,
                path.as_ptr(),
                rev,
                dirent_fields,
                pool.as_mut_ptr(),
            )
        };
        let pool = std::rc::Rc::new(pool);
        crate::Error::from_raw(err)?;
        let mut props_hash =
            apr::hash::Hash::<&str, *const crate::generated::svn_string_t>::from_raw(unsafe {
                PooledPtr::in_pool(pool.clone(), props)
            });
        let mut dirents_hash =
            apr::hash::Hash::<&str, *mut crate::generated::svn_dirent_t>::from_raw(unsafe {
                PooledPtr::in_pool(pool.clone(), dirents)
            });
        let props = props_hash
            .iter()
            .map(|(k, v)| {
                (
                    String::from_utf8_lossy(k).into_owned(),
                    Vec::from(unsafe {
                        std::slice::from_raw_parts((**v).data as *const u8, (**v).len)
                    }),
                )
            })
            .collect();
        let dirents = dirents_hash
            .iter()
            .map(|(k, v)| {
                (
                    String::from_utf8_lossy(k).into_owned(),
                    Dirent(unsafe { PooledPtr::in_pool(pool.clone(), *v) }),
                )
            })
            .collect();
        Ok((fetched_rev, dirents, props))
    }

    pub fn list(
        &mut self,
        path: &str,
        rev: Revnum,
        patterns: Option<&[&str]>,
        depth: Depth,
        dirent_receiver: impl Fn(&str, &Dirent) -> Result<(), crate::Error>,
    ) -> Result<(), Error> {
        let path = std::ffi::CString::new(path).unwrap();
        let mut pool = Pool::new();
        let patterns: Option<apr::tables::ArrayHeader<*const std::os::raw::c_char>> =
            patterns.map(|patterns| {
                patterns
                    .iter()
                    .map(|pattern| pattern.as_ptr() as _)
                    .collect()
            });
        let dirent_fields = crate::generated::SVN_DIRENT_KIND
            | crate::generated::SVN_DIRENT_SIZE
            | crate::generated::SVN_DIRENT_HAS_PROPS
            | crate::generated::SVN_DIRENT_CREATED_REV
            | crate::generated::SVN_DIRENT_TIME
            | crate::generated::SVN_DIRENT_LAST_AUTHOR;
        let err = unsafe {
            crate::generated::svn_ra_list(
                self.0.as_mut_ptr(),
                path.as_ptr(),
                rev,
                if let Some(patterns) = patterns.as_ref() {
                    patterns.as_ptr()
                } else {
                    std::ptr::null()
                },
                depth.into(),
                dirent_fields,
                Some(wrap_dirent_receiver),
                &dirent_receiver as *const _ as *mut _,
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(())
    }

    pub fn get_mergeinfo(
        &mut self,
        paths: &[&str],
        revision: Revnum,
        inherit: crate::mergeinfo::MergeinfoInheritance,
        include_descendants: bool,
    ) -> Result<HashMap<String, crate::mergeinfo::Mergeinfo>, Error> {
        let paths: apr::tables::ArrayHeader<*const std::os::raw::c_char> =
            paths.iter().map(|path| path.as_ptr() as _).collect();
        let mut pool = Pool::new();
        let mut mergeinfo = std::ptr::null_mut();
        let err = unsafe {
            crate::generated::svn_ra_get_mergeinfo(
                self.0.as_mut_ptr(),
                &mut mergeinfo,
                paths.as_ptr(),
                revision,
                inherit.into(),
                include_descendants.into(),
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        let pool = std::rc::Rc::new(pool);
        let mut mergeinfo =
            apr::hash::Hash::<&[u8], *mut crate::generated::svn_mergeinfo_t>::from_raw(unsafe {
                PooledPtr::in_pool(pool.clone(), mergeinfo)
            });
        Ok(mergeinfo
            .iter()
            .map(|(k, v)| {
                (
                    String::from_utf8_lossy(k).into_owned(),
                    crate::mergeinfo::Mergeinfo(unsafe { PooledPtr::in_pool(pool.clone(), *v) }),
                )
            })
            .collect())
    }

    pub fn do_update(
        &mut self,
        revision_to_update_to: Revnum,
        update_target: &str,
        depth: Depth,
        send_copyfrom_args: bool,
        ignore_ancestry: bool,
        editor: &mut dyn Editor,
    ) -> Result<Box<dyn Reporter>, Error> {
        let mut pool = Pool::new();
        let mut scratch_pool = Pool::new();
        let mut reporter = std::ptr::null();
        let mut report_baton = std::ptr::null_mut();
        let err = unsafe {
            crate::generated::svn_ra_do_update3(
                self.0.as_mut_ptr(),
                &mut reporter,
                &mut report_baton,
                revision_to_update_to,
                update_target.as_ptr() as *const _,
                depth.into(),
                send_copyfrom_args.into(),
                ignore_ancestry.into(),
                &crate::delta::WRAP_EDITOR,
                editor as *mut _ as *mut std::ffi::c_void,
                scratch_pool.as_mut_ptr(),
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(Box::new(WrapReporter(reporter, unsafe {
            PooledPtr::in_pool(std::rc::Rc::new(pool), report_baton)
        })) as Box<dyn Reporter>)
    }

    pub fn check_path(&mut self, path: &str, rev: Revnum) -> Result<crate::NodeKind, Error> {
        let path = std::ffi::CString::new(path).unwrap();
        let mut kind = 0;
        let mut pool = Pool::new();
        let err = unsafe {
            crate::generated::svn_ra_check_path(
                self.0.as_mut_ptr(),
                path.as_ptr(),
                rev,
                &mut kind,
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(crate::NodeKind::from(kind))
    }

    pub fn stat(&mut self, path: &str, rev: Revnum) -> Result<Dirent, Error> {
        let path = std::ffi::CString::new(path).unwrap();
        let mut dirent = std::ptr::null_mut();
        let mut pool = Pool::new();
        let err = unsafe {
            crate::generated::svn_ra_stat(
                self.0.as_mut_ptr(),
                path.as_ptr(),
                rev,
                &mut dirent,
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(Dirent(unsafe {
            PooledPtr::in_pool(std::rc::Rc::new(pool), dirent)
        }))
    }

    pub fn get_uuid(&mut self) -> Result<String, Error> {
        let mut pool = Pool::new();
        let mut uuid = std::ptr::null();
        let err = unsafe {
            crate::generated::svn_ra_get_uuid2(self.0.as_mut_ptr(), &mut uuid, pool.as_mut_ptr())
        };
        Error::from_raw(err)?;
        let uuid = unsafe { std::ffi::CStr::from_ptr(uuid) };
        Ok(uuid.to_string_lossy().into_owned())
    }

    pub fn get_repos_root(&mut self) -> Result<String, Error> {
        let mut pool = Pool::new();
        let mut url = std::ptr::null();
        let err = unsafe {
            crate::generated::svn_ra_get_repos_root2(
                self.0.as_mut_ptr(),
                &mut url,
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        let url = unsafe { std::ffi::CStr::from_ptr(url) };
        Ok(url.to_str().unwrap().to_string())
    }

    pub fn get_deleted_rev(
        &mut self,
        path: &str,
        peg_revision: Revnum,
        end_revision: Revnum,
    ) -> Result<Revnum, Error> {
        let path = std::ffi::CString::new(path).unwrap();
        let mut rev = 0;
        let mut pool = Pool::new();
        let err = unsafe {
            crate::generated::svn_ra_get_deleted_rev(
                self.0.as_mut_ptr(),
                path.as_ptr(),
                peg_revision,
                end_revision,
                &mut rev,
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(rev)
    }

    pub fn has_capability(&mut self, capability: &str) -> Result<bool, Error> {
        let capability = std::ffi::CString::new(capability).unwrap();
        let mut has = 0;
        let mut pool = Pool::new();
        let err = unsafe {
            crate::generated::svn_ra_has_capability(
                self.0.as_mut_ptr(),
                &mut has,
                capability.as_ptr(),
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(has != 0)
    }
}

pub struct WrapReporter(
    *const crate::generated::svn_ra_reporter3_t,
    PooledPtr<std::ffi::c_void>,
);

impl Reporter for WrapReporter {
    fn set_path(
        &mut self,
        path: &str,
        rev: Revnum,
        depth: Depth,
        start_empty: bool,
        lock_token: &str,
    ) -> Result<(), Error> {
        let path = std::ffi::CString::new(path).unwrap();
        let lock_token = std::ffi::CString::new(lock_token).unwrap();
        let mut pool = Pool::new();
        let err = unsafe {
            (*self.0).set_path.unwrap()(
                self.1.as_mut_ptr(),
                path.as_ptr(),
                rev,
                depth.into(),
                start_empty.into(),
                lock_token.as_ptr(),
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(())
    }

    fn delete_path(&mut self, path: &str) -> Result<(), Error> {
        let path = std::ffi::CString::new(path).unwrap();
        let mut pool = Pool::new();
        let err = unsafe {
            (*self.0).delete_path.unwrap()(self.1.as_mut_ptr(), path.as_ptr(), pool.as_mut_ptr())
        };
        Error::from_raw(err)?;
        Ok(())
    }

    fn link_path(
        &mut self,
        path: &str,
        url: &str,
        rev: Revnum,
        depth: Depth,
        start_empty: bool,
        lock_token: &str,
    ) -> Result<(), Error> {
        let path = std::ffi::CString::new(path).unwrap();
        let url = std::ffi::CString::new(url).unwrap();
        let lock_token = std::ffi::CString::new(lock_token).unwrap();
        let mut pool = Pool::new();
        let err = unsafe {
            (*self.0).link_path.unwrap()(
                self.1.as_mut_ptr(),
                path.as_ptr(),
                url.as_ptr(),
                rev,
                depth.into(),
                start_empty.into(),
                lock_token.as_ptr(),
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(())
    }

    fn finish_report(&mut self) -> Result<(), Error> {
        let mut pool = Pool::new();
        let err =
            unsafe { (*self.0).finish_report.unwrap()(self.1.as_mut_ptr(), pool.as_mut_ptr()) };
        Error::from_raw(err)?;
        Ok(())
    }

    fn abort_report(&mut self) -> Result<(), Error> {
        let mut pool = Pool::new();
        let err =
            unsafe { (*self.0).abort_report.unwrap()(self.1.as_mut_ptr(), pool.as_mut_ptr()) };
        Error::from_raw(err)?;
        Ok(())
    }
}

#[allow(dead_code)]
pub struct Dirent(PooledPtr<crate::generated::svn_dirent_t>);

pub fn version() -> crate::Version {
    unsafe { crate::Version(crate::generated::svn_ra_version()) }
}

pub trait Reporter {
    fn set_path(
        &mut self,
        path: &str,
        rev: Revnum,
        depth: Depth,
        start_empty: bool,
        lock_token: &str,
    ) -> Result<(), Error>;

    fn delete_path(&mut self, path: &str) -> Result<(), Error>;

    fn link_path(
        &mut self,
        path: &str,
        url: &str,
        rev: Revnum,
        depth: Depth,
        start_empty: bool,
        lock_token: &str,
    ) -> Result<(), Error>;

    fn finish_report(&mut self) -> Result<(), Error>;

    fn abort_report(&mut self) -> Result<(), Error>;
}

pub struct Callbacks(PooledPtr<crate::generated::svn_ra_callbacks2_t>);

impl Default for Callbacks {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

impl Callbacks {
    pub fn new() -> Result<Callbacks, crate::Error> {
        Ok(Callbacks(PooledPtr::initialize(|pool| unsafe {
            let mut callbacks = std::ptr::null_mut();
            let err = crate::generated::svn_ra_create_callbacks(&mut callbacks, pool.as_mut_ptr());
            Error::from_raw(err)?;
            Ok::<_, crate::Error>(callbacks)
        })?))
    }

    fn as_mut_ptr(&mut self) -> *mut crate::generated::svn_ra_callbacks2_t {
        self.0.as_mut_ptr()
    }
}
