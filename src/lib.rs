pub mod apr;
pub mod client;
mod generated;
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
                value: svn_opt_revision_value_t { number: 0 },
            },
            Revision::Number(revnum) => svn_opt_revision_t {
                kind: generated::svn_opt_revision_kind_svn_opt_revision_number,
                value: svn_opt_revision_value_t { number: revnum },
            },
            Revision::Date(date) => svn_opt_revision_t {
                kind: generated::svn_opt_revision_kind_svn_opt_revision_date,
                value: svn_opt_revision_value_t { date },
            },
            Revision::Committed => svn_opt_revision_t {
                kind: generated::svn_opt_revision_kind_svn_opt_revision_committed,
                value: svn_opt_revision_value_t { number: 0 },
            },
            Revision::Previous => svn_opt_revision_t {
                kind: generated::svn_opt_revision_kind_svn_opt_revision_previous,
                value: svn_opt_revision_value_t { number: 0 },
            },
            Revision::Base => svn_opt_revision_t {
                kind: generated::svn_opt_revision_kind_svn_opt_revision_base,
                value: svn_opt_revision_value_t { number: 0 },
            },
            Revision::Working => svn_opt_revision_t {
                kind: generated::svn_opt_revision_kind_svn_opt_revision_working,
                value: svn_opt_revision_value_t { number: 0 },
            },
            Revision::Head => svn_opt_revision_t {
                kind: generated::svn_opt_revision_kind_svn_opt_revision_head,
                value: svn_opt_revision_value_t { number: 0 },
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
