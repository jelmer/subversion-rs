pub mod client;
pub mod config;
pub mod dirent;
pub mod error;
pub mod fs;
mod generated;
pub mod io;
pub mod mergeinfo;
pub mod ra;
pub mod repos;
pub mod time;
pub mod uri;
pub mod version;
pub mod wc;
use crate::generated::{svn_opt_revision_t, svn_opt_revision_value_t};
use apr::pool::PooledPtr;

pub use version::Version;

pub type Revnum = generated::svn_revnum_t;

pub use error::Error;

#[derive(Debug, Default, Clone, Copy)]
pub enum Revision {
    #[default]
    Unspecified,
    Number(Revnum),
    Date(i64),
    Committed,
    Previous,
    Base,
    Working,
    Head,
}

impl From<Revnum> for Revision {
    fn from(revnum: Revnum) -> Self {
        Revision::Number(revnum)
    }
}

impl From<Revision> for svn_opt_revision_t {
    fn from(revision: Revision) -> Self {
        match revision {
            Revision::Unspecified => svn_opt_revision_t {
                kind: generated::svn_opt_revision_kind_svn_opt_revision_unspecified,
                value: svn_opt_revision_value_t::default(),
            },
            Revision::Number(revnum) => {
                let mut uf = crate::generated::__BindgenUnionField::<i64>::new();
                unsafe {
                    *uf.as_mut() = revnum;
                }

                svn_opt_revision_t {
                    kind: generated::svn_opt_revision_kind_svn_opt_revision_number,
                    value: svn_opt_revision_value_t {
                        number: uf,
                        ..Default::default()
                    },
                }
            }
            Revision::Date(date) => {
                let mut uf = crate::generated::__BindgenUnionField::<i64>::new();

                unsafe {
                    *uf.as_mut() = date;
                }

                svn_opt_revision_t {
                    kind: generated::svn_opt_revision_kind_svn_opt_revision_date,
                    value: svn_opt_revision_value_t {
                        date: uf,
                        ..Default::default()
                    },
                }
            }
            Revision::Committed => svn_opt_revision_t {
                kind: generated::svn_opt_revision_kind_svn_opt_revision_committed,
                value: svn_opt_revision_value_t::default(),
            },
            Revision::Previous => svn_opt_revision_t {
                kind: generated::svn_opt_revision_kind_svn_opt_revision_previous,
                value: svn_opt_revision_value_t::default(),
            },
            Revision::Base => svn_opt_revision_t {
                kind: generated::svn_opt_revision_kind_svn_opt_revision_base,
                value: svn_opt_revision_value_t::default(),
            },
            Revision::Working => svn_opt_revision_t {
                kind: generated::svn_opt_revision_kind_svn_opt_revision_working,
                value: svn_opt_revision_value_t::default(),
            },
            Revision::Head => svn_opt_revision_t {
                kind: generated::svn_opt_revision_kind_svn_opt_revision_head,
                value: svn_opt_revision_value_t::default(),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub enum Depth {
    #[default]
    Unknown,
    Exclude,
    Empty,
    Files,
    Immediates,
    Infinity,
}

impl From<Depth> for generated::svn_depth_t {
    fn from(depth: Depth) -> Self {
        match depth {
            Depth::Unknown => generated::svn_depth_t_svn_depth_unknown,
            Depth::Exclude => generated::svn_depth_t_svn_depth_exclude,
            Depth::Empty => generated::svn_depth_t_svn_depth_empty,
            Depth::Files => generated::svn_depth_t_svn_depth_files,
            Depth::Immediates => generated::svn_depth_t_svn_depth_immediates,
            Depth::Infinity => generated::svn_depth_t_svn_depth_infinity,
        }
    }
}

pub struct CommitInfo<'pool>(PooledPtr<'pool, generated::svn_commit_info_t>);

impl<'pool> CommitInfo<'pool> {
    pub fn revision(&self) -> Revnum {
        self.0.revision
    }

    pub fn date(&self) -> &str {
        unsafe {
            let date = self.0.date;
            std::ffi::CStr::from_ptr(date).to_str().unwrap()
        }
    }

    pub fn author(&self) -> &str {
        unsafe {
            let author = self.0.author;
            std::ffi::CStr::from_ptr(author).to_str().unwrap()
        }
    }
    pub fn post_commit_err(&self) -> Option<&str> {
        unsafe {
            let err = self.0.post_commit_err;
            if err.is_null() {
                None
            } else {
                Some(std::ffi::CStr::from_ptr(err).to_str().unwrap())
            }
        }
    }
    pub fn repos_root(&self) -> &str {
        unsafe {
            let repos_root = self.0.repos_root;
            std::ffi::CStr::from_ptr(repos_root).to_str().unwrap()
        }
    }
}

pub struct RevisionRange {
    pub start: Revision,
    pub end: Revision,
}

impl From<&RevisionRange> for generated::svn_opt_revision_range_t {
    fn from(range: &RevisionRange) -> Self {
        Self {
            start: range.start.into(),
            end: range.end.into(),
        }
    }
}

impl RevisionRange {
    pub unsafe fn to_c(&self, pool: &mut apr::Pool) -> *mut generated::svn_opt_revision_range_t {
        let range = pool.calloc::<generated::svn_opt_revision_range_t>();
        *range = self.into();
        range
    }
}

pub struct LogEntry<'pool>(apr::pool::PooledPtr<'pool, generated::svn_log_entry_t>);
