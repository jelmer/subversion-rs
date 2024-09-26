pub mod auth;
pub mod client;
pub mod config;
pub mod delta;
pub mod dirent;
pub mod error;
pub mod fs;
mod generated;
pub mod io;
pub mod mergeinfo;
pub mod props;
#[cfg(feature = "ra")]
pub mod ra;
pub mod repos;
pub mod string;
pub mod time;
pub mod uri;
pub mod version;
#[cfg(feature = "wc")]
pub mod wc;
use crate::generated::{svn_opt_revision_t, svn_opt_revision_value_t};
use apr::pool::PooledPtr;
use std::str::FromStr;

pub use version::Version;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, std::hash::Hash)]
pub struct Revnum(generated::svn_revnum_t);

impl From<Revnum> for generated::svn_revnum_t {
    fn from(revnum: Revnum) -> Self {
        revnum.0
    }
}

impl apr::hash::IntoHashKey<'_> for &Revnum {
    fn into_hash_key(self) -> &'static [u8] {
        unsafe {
            std::slice::from_raw_parts(
                &self.0 as *const _ as *const u8,
                std::mem::size_of::<generated::svn_revnum_t>(),
            )
        }
    }
}

impl Revnum {
    fn from_raw(raw: generated::svn_revnum_t) -> Option<Self> {
        if raw < 0 {
            None
        } else {
            Some(Self(raw))
        }
    }
}

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
            Ok(Revision::Number(Revnum(
                rest.parse::<crate::generated::svn_revnum_t>()
                    .map_err(|e| e.to_string())?,
            )))
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
    fn extract_bound(ob: &pyo3::Bound<pyo3::PyAny>) -> pyo3::PyResult<Self> {
        use pyo3::prelude::*;
        if ob.is_instance_of::<pyo3::types::PyString>() {
            let rev = ob.extract::<String>()?;
            return Revision::from_str(&rev).map_err(|e| {
                pyo3::exceptions::PyValueError::new_err(format!("Invalid revision: {}", e))
            });
        } else if ob.is_instance_of::<pyo3::types::PyLong>() {
            let rev = ob.extract::<i64>()?;
            return Ok(Revision::Number(Revnum::from_raw(rev).unwrap()));
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
                    *uf.as_mut() = revnum.0;
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

impl From<generated::svn_depth_t> for Depth {
    fn from(depth: generated::svn_depth_t) -> Self {
        match depth {
            generated::svn_depth_t_svn_depth_unknown => Depth::Unknown,
            generated::svn_depth_t_svn_depth_exclude => Depth::Exclude,
            generated::svn_depth_t_svn_depth_empty => Depth::Empty,
            generated::svn_depth_t_svn_depth_files => Depth::Files,
            generated::svn_depth_t_svn_depth_immediates => Depth::Immediates,
            generated::svn_depth_t_svn_depth_infinity => Depth::Infinity,
            n => panic!("Unknown depth: {:?}", n),
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
    fn extract_bound(ob: &pyo3::Bound<pyo3::PyAny>) -> pyo3::PyResult<Self> {
        use pyo3::prelude::*;
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

pub struct CommitInfo(PooledPtr<generated::svn_commit_info_t>);

impl CommitInfo {
    pub fn revision(&self) -> Revnum {
        Revnum::from_raw(self.0.revision).unwrap()
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

impl Clone for CommitInfo {
    fn clone(&self) -> Self {
        unsafe {
            Self(
                PooledPtr::initialize(|pool| {
                    Ok::<_, Error>(crate::generated::svn_commit_info_dup(
                        self.0.as_ptr(),
                        pool.as_mut_ptr(),
                    ))
                })
                .unwrap(),
            )
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
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

pub struct LogEntry(apr::pool::PooledPtr<generated::svn_log_entry_t>);
unsafe impl Send for LogEntry {}

impl Clone for LogEntry {
    fn clone(&self) -> Self {
        Self(
            apr::pool::PooledPtr::initialize(|pool| {
                Ok::<_, Error>(unsafe {
                    crate::generated::svn_log_entry_dup(self.0.as_ptr(), pool.as_mut_ptr())
                })
            })
            .unwrap(),
        )
    }
}

pub type FileSize = crate::generated::svn_filesize_t;

#[derive(Debug, Clone, Copy, Default)]
pub enum NativeEOL {
    #[default]
    Standard,
    LF,
    CRLF,
    CR,
}

impl TryFrom<Option<&str>> for NativeEOL {
    type Error = crate::Error;

    fn try_from(eol: Option<&str>) -> Result<Self, Self::Error> {
        match eol {
            None => Ok(NativeEOL::Standard),
            Some("LF") => Ok(NativeEOL::LF),
            Some("CRLF") => Ok(NativeEOL::CRLF),
            Some("CR") => Ok(NativeEOL::CR),
            _ => Err(crate::Error::new(
                crate::generated::svn_errno_t_SVN_ERR_IO_UNKNOWN_EOL.into(),
                None,
                "Unknown eol marker",
            )),
        }
    }
}

impl From<NativeEOL> for Option<&str> {
    fn from(eol: NativeEOL) -> Self {
        match eol {
            NativeEOL::Standard => None,
            NativeEOL::LF => Some("LF"),
            NativeEOL::CRLF => Some("CRLF"),
            NativeEOL::CR => Some("CR"),
        }
    }
}

pub struct InheritedItem(apr::pool::PooledPtr<generated::svn_prop_inherited_item_t>);

impl InheritedItem {
    pub fn from_raw(ptr: apr::pool::PooledPtr<generated::svn_prop_inherited_item_t>) -> Self {
        Self(ptr)
    }

    pub fn path_or_url(&self) -> &str {
        unsafe {
            let path_or_url = self.0.path_or_url;
            std::ffi::CStr::from_ptr(path_or_url).to_str().unwrap()
        }
    }
}

pub struct Canonical<T>(T);

impl<T> std::ops::Deref for Canonical<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: ToString> std::fmt::Debug for Canonical<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Canonical").field(&self.to_string()).finish()
    }
}

impl<T> PartialEq for Canonical<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl<T> Eq for Canonical<T> where T: Eq {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    None,
    File,
    Dir,
    Unknown,
    Symlink,
}

impl From<generated::svn_node_kind_t> for NodeKind {
    fn from(kind: generated::svn_node_kind_t) -> Self {
        match kind {
            generated::svn_node_kind_t_svn_node_none => NodeKind::None,
            generated::svn_node_kind_t_svn_node_file => NodeKind::File,
            generated::svn_node_kind_t_svn_node_dir => NodeKind::Dir,
            generated::svn_node_kind_t_svn_node_unknown => NodeKind::Unknown,
            generated::svn_node_kind_t_svn_node_symlink => NodeKind::Symlink,
            n => panic!("Unknown node kind: {:?}", n),
        }
    }
}

impl From<NodeKind> for generated::svn_node_kind_t {
    fn from(kind: NodeKind) -> Self {
        match kind {
            NodeKind::None => generated::svn_node_kind_t_svn_node_none,
            NodeKind::File => generated::svn_node_kind_t_svn_node_file,
            NodeKind::Dir => generated::svn_node_kind_t_svn_node_dir,
            NodeKind::Unknown => generated::svn_node_kind_t_svn_node_unknown,
            NodeKind::Symlink => generated::svn_node_kind_t_svn_node_symlink,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    None,
    Unversioned,
    Normal,
    Added,
    Missing,
    Deleted,
    Replaced,
    Modified,
    Merged,
    Conflicted,
    Ignored,
    Obstructed,
    External,
    Incomplete,
}

impl From<generated::svn_wc_status_kind> for StatusKind {
    fn from(kind: generated::svn_wc_status_kind) -> Self {
        match kind {
            generated::svn_wc_status_kind_svn_wc_status_none => StatusKind::None,
            generated::svn_wc_status_kind_svn_wc_status_unversioned => StatusKind::Unversioned,
            generated::svn_wc_status_kind_svn_wc_status_normal => StatusKind::Normal,
            generated::svn_wc_status_kind_svn_wc_status_added => StatusKind::Added,
            generated::svn_wc_status_kind_svn_wc_status_missing => StatusKind::Missing,
            generated::svn_wc_status_kind_svn_wc_status_deleted => StatusKind::Deleted,
            generated::svn_wc_status_kind_svn_wc_status_replaced => StatusKind::Replaced,
            generated::svn_wc_status_kind_svn_wc_status_modified => StatusKind::Modified,
            generated::svn_wc_status_kind_svn_wc_status_merged => StatusKind::Merged,
            generated::svn_wc_status_kind_svn_wc_status_conflicted => StatusKind::Conflicted,
            generated::svn_wc_status_kind_svn_wc_status_ignored => StatusKind::Ignored,
            generated::svn_wc_status_kind_svn_wc_status_obstructed => StatusKind::Obstructed,
            generated::svn_wc_status_kind_svn_wc_status_external => StatusKind::External,
            generated::svn_wc_status_kind_svn_wc_status_incomplete => StatusKind::Incomplete,
            n => panic!("Unknown status kind: {:?}", n),
        }
    }
}

impl From<StatusKind> for generated::svn_wc_status_kind {
    fn from(kind: StatusKind) -> Self {
        match kind {
            StatusKind::None => generated::svn_wc_status_kind_svn_wc_status_none,
            StatusKind::Unversioned => generated::svn_wc_status_kind_svn_wc_status_unversioned,
            StatusKind::Normal => generated::svn_wc_status_kind_svn_wc_status_normal,
            StatusKind::Added => generated::svn_wc_status_kind_svn_wc_status_added,
            StatusKind::Missing => generated::svn_wc_status_kind_svn_wc_status_missing,
            StatusKind::Deleted => generated::svn_wc_status_kind_svn_wc_status_deleted,
            StatusKind::Replaced => generated::svn_wc_status_kind_svn_wc_status_replaced,
            StatusKind::Modified => generated::svn_wc_status_kind_svn_wc_status_modified,
            StatusKind::Merged => generated::svn_wc_status_kind_svn_wc_status_merged,
            StatusKind::Conflicted => generated::svn_wc_status_kind_svn_wc_status_conflicted,
            StatusKind::Ignored => generated::svn_wc_status_kind_svn_wc_status_ignored,
            StatusKind::Obstructed => generated::svn_wc_status_kind_svn_wc_status_obstructed,
            StatusKind::External => generated::svn_wc_status_kind_svn_wc_status_external,
            StatusKind::Incomplete => generated::svn_wc_status_kind_svn_wc_status_incomplete,
        }
    }
}

pub struct Lock(PooledPtr<generated::svn_lock_t>);

impl Lock {
    pub fn path(&self) -> &str {
        unsafe {
            let path = (*self.0).path;
            std::ffi::CStr::from_ptr(path).to_str().unwrap()
        }
    }

    pub fn token(&self) -> &str {
        unsafe {
            let token = (*self.0).token;
            std::ffi::CStr::from_ptr(token).to_str().unwrap()
        }
    }

    pub fn owner(&self) -> &str {
        unsafe {
            let owner = (*self.0).owner;
            std::ffi::CStr::from_ptr(owner).to_str().unwrap()
        }
    }

    pub fn comment(&self) -> &str {
        unsafe {
            let comment = (*self.0).comment;
            std::ffi::CStr::from_ptr(comment).to_str().unwrap()
        }
    }

    pub fn is_dav_comment(&self) -> bool {
        (*self.0).is_dav_comment == 1
    }

    pub fn creation_date(&self) -> i64 {
        (*self.0).creation_date
    }

    pub fn expiration_date(&self) -> apr::time::Time {
        apr::time::Time::from((*self.0).expiration_date)
    }
}

pub enum ChecksumKind {
    MD5,
    SHA1,
    Fnv1a32,
    Fnv1a32x4,
}

impl From<crate::generated::svn_checksum_kind_t> for ChecksumKind {
    fn from(kind: crate::generated::svn_checksum_kind_t) -> Self {
        match kind {
            crate::generated::svn_checksum_kind_t_svn_checksum_md5 => ChecksumKind::MD5,
            crate::generated::svn_checksum_kind_t_svn_checksum_sha1 => ChecksumKind::SHA1,
            crate::generated::svn_checksum_kind_t_svn_checksum_fnv1a_32 => ChecksumKind::Fnv1a32,
            crate::generated::svn_checksum_kind_t_svn_checksum_fnv1a_32x4 => {
                ChecksumKind::Fnv1a32x4
            }
            n => panic!("Unknown checksum kind: {:?}", n),
        }
    }
}

impl From<ChecksumKind> for crate::generated::svn_checksum_kind_t {
    fn from(kind: ChecksumKind) -> Self {
        match kind {
            ChecksumKind::MD5 => crate::generated::svn_checksum_kind_t_svn_checksum_md5,
            ChecksumKind::SHA1 => crate::generated::svn_checksum_kind_t_svn_checksum_sha1,
            ChecksumKind::Fnv1a32 => crate::generated::svn_checksum_kind_t_svn_checksum_fnv1a_32,
            ChecksumKind::Fnv1a32x4 => {
                crate::generated::svn_checksum_kind_t_svn_checksum_fnv1a_32x4
            }
        }
    }
}

pub struct LocationSegment(PooledPtr<generated::svn_location_segment_t>);

impl LocationSegment {
    pub fn range(&self) -> std::ops::Range<Revnum> {
        Revnum::from_raw(self.0.range_end).unwrap()..Revnum::from_raw(self.0.range_start).unwrap()
    }

    pub fn path(&self) -> &str {
        unsafe {
            let path = self.0.path;
            std::ffi::CStr::from_ptr(path).to_str().unwrap()
        }
    }
}
