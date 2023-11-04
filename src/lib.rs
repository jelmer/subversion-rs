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
use std::str::FromStr;

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

impl FromStr for Revision {
    type Err = String;

    fn from_str(rev: &str) -> Result<Self, Self::Err> {
        if rev == "unspecified" {
            Ok(Revision::Unspecified)
        } else if rev == "committed" {
            Ok(Revision::Committed)
        } else if rev == "previous" {
            Ok(Revision::Previous)
        } else if rev == "base" {
            Ok(Revision::Base)
        } else if rev == "working" {
            Ok(Revision::Working)
        } else if rev == "head" {
            Ok(Revision::Head)
        } else if let Some(rest) = rev.strip_prefix("number:") {
            Ok(Revision::Number(
                rest.parse::<Revnum>().map_err(|e| e.to_string())?,
            ))
        } else if let Some(rest) = rev.strip_prefix("date:") {
            Ok(Revision::Date(
                rest.parse::<i64>().map_err(|e| e.to_string())?,
            ))
        } else {
            Err(format!("Invalid revision: {}", rev))
        }
    }
}

#[cfg(feature = "pyo3")]
impl pyo3::FromPyObject<'_> for Revision {
    fn extract(ob: &pyo3::PyAny) -> pyo3::PyResult<Self> {
        if ob.is_instance_of::<pyo3::types::PyString>() {
            let rev = ob.extract::<String>()?;
            return Revision::from_str(&rev).map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Invalid revision: {}", e))
            });
        } else if ob.is_instance_of::<pyo3::types::PyLong>() {
            let rev = ob.extract::<i64>()?;
            return Ok(Revision::Number(rev as Revnum));
        } else {
            Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "Invalid revision: {:?}",
                ob
            )))
        }
    }
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

impl std::str::FromStr for Depth {
    type Err = String;

    fn from_str(depth: &str) -> Result<Self, Self::Err> {
        match depth {
            "unknown" => Ok(Depth::Unknown),
            "exclude" => Ok(Depth::Exclude),
            "empty" => Ok(Depth::Empty),
            "files" => Ok(Depth::Files),
            "immediates" => Ok(Depth::Immediates),
            "infinity" => Ok(Depth::Infinity),
            _ => Err(format!("Invalid depth: {}", depth)),
        }
    }
}

#[cfg(feature = "pyo3")]
impl pyo3::FromPyObject<'_> for Depth {
    fn extract(ob: &pyo3::PyAny) -> pyo3::PyResult<Self> {
        if ob.is_instance_of::<pyo3::types::PyString>() {
            let depth = ob.extract::<String>()?;
            return Depth::from_str(&depth).map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Invalid depth: {}", e))
            });
        } else if ob.is_instance_of::<pyo3::types::PyBool>() {
            let depth = ob.extract::<bool>()?;
            return Ok(if depth { Depth::Infinity } else { Depth::Empty });
        } else {
            Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "Invalid depth: {:?}",
                ob
            )))
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

pub type FileSize = crate::generated::svn_filesize_t;
