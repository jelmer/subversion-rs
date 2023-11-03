pub mod client;
mod generated;
pub mod io;
pub mod repos;
use crate::generated::{svn_error_t, svn_opt_revision_t, svn_opt_revision_value_t, svn_version_t};

pub struct Version(*const svn_version_t);

impl Version {
    fn equal(&self, other: &Version) -> bool {
        !matches!(unsafe { generated::svn_ver_equal(self.0, other.0) }, 0)
    }

    pub fn compatible(&self, other: &Version) -> bool {
        !matches!(unsafe { generated::svn_ver_compatible(self.0, other.0) }, 0)
    }

    pub fn major(&self) -> i32 {
        unsafe { self.0.as_ref().unwrap().major }
    }

    pub fn minor(&self) -> i32 {
        unsafe { self.0.as_ref().unwrap().minor }
    }

    pub fn patch(&self) -> i32 {
        unsafe { self.0.as_ref().unwrap().patch }
    }

    pub fn tag(&self) -> &str {
        unsafe {
            let tag = self.0.as_ref().unwrap().tag;
            std::ffi::CStr::from_ptr(tag).to_str().unwrap()
        }
    }
}

impl PartialEq for Version {
    fn eq(&self, other: &Version) -> bool {
        self.equal(other)
    }
}

impl Eq for Version {}

type Revnum = generated::svn_revnum_t;

pub struct Error(*mut svn_error_t);

impl Error {
    pub fn apr_err(&self) -> i32 {
        unsafe { self.0.as_ref().unwrap().apr_err }
    }

    pub fn line(&self) -> i64 {
        unsafe { self.0.as_ref().unwrap().line }
    }

    pub fn file(&self) -> &str {
        unsafe {
            let file = self.0.as_ref().unwrap().file;
            std::ffi::CStr::from_ptr(file).to_str().unwrap()
        }
    }

    pub fn child(&self) -> Option<Error> {
        unsafe {
            let child = self.0.as_ref().unwrap().child;
            if child.is_null() {
                None
            } else {
                Some(Error(child))
            }
        }
    }

    pub fn message(&self) -> &str {
        unsafe {
            let message = self.0.as_ref().unwrap().message;
            std::ffi::CStr::from_ptr(message).to_str().unwrap()
        }
    }
}

impl Drop for Error {
    fn drop(&mut self) {
        unsafe { generated::svn_error_clear(self.0) }
    }
}

impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(f, "{}:{}: {}", self.file(), self.line(), self.message())?;
        let mut n = self.child();
        while let Some(err) = n {
            writeln!(f, "{}:{}: {}", err.file(), err.line(), err.message())?;
            n = err.child();
        }
        Ok(())
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(f, "{}", self.message())?;
        Ok(())
    }
}

impl std::error::Error for Error {}

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

pub struct CommitInfo(*const generated::svn_commit_info_t);

impl CommitInfo {
    pub fn revision(&self) -> Revnum {
        unsafe { (*self.0).revision }
    }

    pub fn date(&self) -> &str {
        unsafe {
            let date = (*self.0).date;
            std::ffi::CStr::from_ptr(date).to_str().unwrap()
        }
    }

    pub fn author(&self) -> &str {
        unsafe {
            let author = (*self.0).author;
            std::ffi::CStr::from_ptr(author).to_str().unwrap()
        }
    }
    pub fn post_commit_err(&self) -> Option<&str> {
        unsafe {
            let err = (*self.0).post_commit_err;
            if err.is_null() {
                None
            } else {
                Some(std::ffi::CStr::from_ptr(err).to_str().unwrap())
            }
        }
    }
    pub fn repos_root(&self) -> &str {
        unsafe {
            let repos_root = (*self.0).repos_root;
            std::ffi::CStr::from_ptr(repos_root).to_str().unwrap()
        }
    }
}

pub struct RevisionRange {
    pub start: Revision,
    pub end: Revision,
}

impl RevisionRange {
    pub unsafe fn to_c(&self, pool: &mut apr::Pool) -> *mut generated::svn_opt_revision_range_t {
        let range: *mut generated::svn_opt_revision_range_t = pool.calloc();
        (*range).start = self.start.into();
        (*range).end = self.end.into();
        range
    }
}

pub struct LogEntry(*mut generated::svn_log_entry_t);
