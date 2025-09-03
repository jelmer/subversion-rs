// Re-export apr_sys for internal use
extern crate apr_sys;

/// Helper to create temporary pools for FFI calls
pub(crate) fn with_tmp_pool<R>(f: impl FnOnce(&apr::Pool) -> R) -> R {
    let pool = apr::Pool::new();
    f(&pool)
}

/// Convert SVN error to Rust Result
pub(crate) fn svn_result(code: *mut subversion_sys::svn_error_t) -> Result<(), Error> {
    Error::from_raw(code)
}

pub mod auth;
pub mod cache;
#[cfg(feature = "client")]
pub mod client;
pub mod cmdline;
pub mod config;
#[cfg(feature = "client")]
pub mod conflict;
#[cfg(feature = "delta")]
pub mod delta;
pub mod diff;
pub mod dirent;
pub mod error;
pub mod fs;
pub mod hash;
pub mod io;
pub mod iter;
#[cfg(feature = "client")]
pub mod merge;
pub mod mergeinfo;
pub mod nls;
pub mod opt;
pub mod props;
#[cfg(feature = "ra")]
pub mod ra;
pub mod repos;
pub mod sorts;
pub mod string;
pub mod subst;
pub mod time;
pub mod uri;
pub mod utf;
pub mod version;
#[cfg(feature = "wc")]
pub mod wc;
pub mod x509;
pub mod xml;
use bitflags::bitflags;
use std::str::FromStr;
use subversion_sys::{svn_opt_revision_t, svn_opt_revision_value_t};

pub use version::Version;

// Re-export important types for API consumers
pub use repos::{LoadUUID, Notify};

bitflags! {
    pub struct DirentField: u32 {
        const Kind = subversion_sys::SVN_DIRENT_KIND;
        const Size = subversion_sys::SVN_DIRENT_SIZE;
        const HasProps = subversion_sys::SVN_DIRENT_HAS_PROPS;
        const CreatedRevision = subversion_sys::SVN_DIRENT_CREATED_REV;
        const Time = subversion_sys::SVN_DIRENT_TIME;
        const LastAuthor = subversion_sys::SVN_DIRENT_LAST_AUTHOR;
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, std::hash::Hash)]
pub struct Revnum(subversion_sys::svn_revnum_t);

impl From<Revnum> for subversion_sys::svn_revnum_t {
    fn from(revnum: Revnum) -> Self {
        revnum.0
    }
}

impl From<usize> for Revnum {
    fn from(revnum: usize) -> Self {
        Self(revnum as _)
    }
}

impl From<u32> for Revnum {
    fn from(revnum: u32) -> Self {
        Self(revnum as _)
    }
}

impl From<u64> for Revnum {
    fn from(revnum: u64) -> Self {
        Self(revnum as _)
    }
}

impl From<Revnum> for usize {
    fn from(revnum: Revnum) -> Self {
        revnum.0 as _
    }
}

impl From<Revnum> for u32 {
    fn from(revnum: Revnum) -> Self {
        revnum.0 as _
    }
}

impl From<Revnum> for u64 {
    fn from(revnum: Revnum) -> Self {
        revnum.0 as _
    }
}

impl Revnum {
    pub fn from_raw(raw: subversion_sys::svn_revnum_t) -> Option<Self> {
        if raw < 0 {
            None
        } else {
            Some(Self(raw))
        }
    }

    /// Get the revision number as u64 (for Python compatibility)
    pub fn as_u64(&self) -> u64 {
        self.0 as u64
    }

    /// Get the raw svn_revnum_t value
    pub fn as_i64(&self) -> i64 {
        self.0
    }
}

pub use error::Error;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_revnum_conversions() {
        // Test from u64
        let rev = Revnum::from(42u64);
        assert_eq!(rev.as_u64(), 42);
        assert_eq!(rev.as_i64(), 42);

        // Test from u32
        let rev = Revnum::from(100u32);
        assert_eq!(rev.as_u64(), 100);
        assert_eq!(rev.as_i64(), 100);

        // Test from usize
        let rev = Revnum::from(1000usize);
        assert_eq!(rev.as_u64(), 1000);
        assert_eq!(rev.as_i64(), 1000);
    }

    #[test]
    fn test_revnum_from_raw() {
        // Valid revision
        let rev = Revnum::from_raw(42);
        assert!(rev.is_some());
        assert_eq!(rev.unwrap().as_i64(), 42);

        // Invalid revision (negative)
        let rev = Revnum::from_raw(-1);
        assert!(rev.is_none());
    }

    #[test]
    fn test_revnum_into_conversions() {
        let rev = Revnum::from(42u64);

        // Test into usize
        let val: usize = rev.into();
        assert_eq!(val, 42);

        // Test into svn_revnum_t
        let rev = Revnum::from(100u64);
        let val: subversion_sys::svn_revnum_t = rev.into();
        assert_eq!(val, 100);
    }

    #[test]
    fn test_fs_path_change_kind_conversions() {
        // Test all variants convert properly
        assert_eq!(
            FsPathChangeKind::from(
                subversion_sys::svn_fs_path_change_kind_t_svn_fs_path_change_modify
            ),
            FsPathChangeKind::Modify
        );
        assert_eq!(
            FsPathChangeKind::from(
                subversion_sys::svn_fs_path_change_kind_t_svn_fs_path_change_add
            ),
            FsPathChangeKind::Add
        );
        assert_eq!(
            FsPathChangeKind::from(
                subversion_sys::svn_fs_path_change_kind_t_svn_fs_path_change_delete
            ),
            FsPathChangeKind::Delete
        );
        assert_eq!(
            FsPathChangeKind::from(
                subversion_sys::svn_fs_path_change_kind_t_svn_fs_path_change_replace
            ),
            FsPathChangeKind::Replace
        );
    }

    #[test]
    fn test_checksum_operations() {
        let pool = apr::Pool::new();
        let data = b"Hello, World!";

        // Test creating a checksum
        let checksum1 = checksum(ChecksumKind::SHA1, data, &pool).unwrap();
        assert_eq!(checksum1.kind(), ChecksumKind::SHA1);
        assert!(!checksum1.is_empty());
        assert_eq!(checksum1.size(), 20); // SHA1 is 20 bytes

        // Test creating another checksum with same data
        let checksum2 = Checksum::create(ChecksumKind::SHA1, data, &pool).unwrap();

        // Test matching checksums
        assert!(checksum1.matches(&checksum2));

        // Test different data produces different checksum
        let checksum3 = checksum(ChecksumKind::SHA1, b"Different data", &pool).unwrap();
        assert!(!checksum1.matches(&checksum3));

        // Test empty checksum
        let empty = Checksum::empty(ChecksumKind::SHA1, &pool).unwrap();
        assert!(empty.is_empty());
    }

    #[test]
    fn test_checksum_serialization() {
        let pool = apr::Pool::new();
        let data = b"Test data for serialization";

        // Create a checksum
        let checksum1 = checksum(ChecksumKind::MD5, data, &pool).unwrap();

        // Serialize it
        let serialized = checksum1.serialize(&pool).unwrap();
        assert!(!serialized.is_empty());

        // Deserialize it
        let checksum2 = Checksum::deserialize(&serialized, &pool).unwrap();

        // They should match
        assert!(checksum1.matches(&checksum2));
    }

    #[test]
    fn test_checksum_hex_conversion() {
        let pool = apr::Pool::new();
        let data = b"Test";

        // Create a checksum
        let checksum = checksum(ChecksumKind::MD5, data, &pool).unwrap();

        // Convert to hex
        let hex = checksum.to_hex(&pool);
        assert!(!hex.is_empty());

        // Parse from hex
        let checksum2 = Checksum::parse_hex(ChecksumKind::MD5, &hex, &pool).unwrap();

        // They should match
        assert!(checksum.matches(&checksum2));
    }

    #[test]
    fn test_checksum_context() {
        let pool = apr::Pool::new();

        // Create a context
        let mut ctx = ChecksumContext::new(ChecksumKind::SHA1, &pool).unwrap();

        // Update with data in chunks
        ctx.update(b"Hello, ").unwrap();
        ctx.update(b"World!").unwrap();

        // Finish and get checksum
        let checksum1 = ctx.finish(&pool).unwrap();

        // Compare with single-shot checksum
        let checksum2 = checksum(ChecksumKind::SHA1, b"Hello, World!", &pool).unwrap();
        assert!(checksum1.matches(&checksum2));
    }

    #[test]
    fn test_checksum_dup() {
        let pool1 = apr::Pool::new();
        let pool2 = apr::Pool::new();
        let data = b"Duplicate me";

        // Create a checksum (use SHA1 instead of SHA256)
        let checksum1 = checksum(ChecksumKind::SHA1, data, &pool1).unwrap();

        // Duplicate to another pool
        let checksum2 = checksum1.dup(&pool2).unwrap();

        // They should match
        assert!(checksum1.matches(&checksum2));
        assert_eq!(checksum1.kind(), checksum2.kind());
    }

    #[test]
    fn test_checksum_has_known_size() {
        let pool = apr::Pool::new();

        let md5 = Checksum::empty(ChecksumKind::MD5, &pool).unwrap();
        assert!(md5.has_known_size(16));
        assert!(!md5.has_known_size(20));

        let sha1 = Checksum::empty(ChecksumKind::SHA1, &pool).unwrap();
        assert!(sha1.has_known_size(20));
        assert!(!sha1.has_known_size(16));

        let fnv = Checksum::empty(ChecksumKind::Fnv1a32, &pool).unwrap();
        assert!(fnv.has_known_size(4));
        assert!(!fnv.has_known_size(16));
    }
}

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
                rest.parse::<subversion_sys::svn_revnum_t>()
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
        } else if ob.is_instance_of::<pyo3::types::PyInt>() {
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
                kind: subversion_sys::svn_opt_revision_kind_svn_opt_revision_unspecified,
                value: svn_opt_revision_value_t::default(),
            },
            Revision::Number(revnum) => {
                let mut uf = subversion_sys::__BindgenUnionField::<i64>::new();
                unsafe {
                    *uf.as_mut() = revnum.0;
                }

                svn_opt_revision_t {
                    kind: subversion_sys::svn_opt_revision_kind_svn_opt_revision_number,
                    value: svn_opt_revision_value_t {
                        number: uf,
                        ..Default::default()
                    },
                }
            }
            Revision::Date(date) => {
                let mut uf = subversion_sys::__BindgenUnionField::<i64>::new();

                unsafe {
                    *uf.as_mut() = date;
                }

                svn_opt_revision_t {
                    kind: subversion_sys::svn_opt_revision_kind_svn_opt_revision_date,
                    value: svn_opt_revision_value_t {
                        date: uf,
                        ..Default::default()
                    },
                }
            }
            Revision::Committed => svn_opt_revision_t {
                kind: subversion_sys::svn_opt_revision_kind_svn_opt_revision_committed,
                value: svn_opt_revision_value_t::default(),
            },
            Revision::Previous => svn_opt_revision_t {
                kind: subversion_sys::svn_opt_revision_kind_svn_opt_revision_previous,
                value: svn_opt_revision_value_t::default(),
            },
            Revision::Base => svn_opt_revision_t {
                kind: subversion_sys::svn_opt_revision_kind_svn_opt_revision_base,
                value: svn_opt_revision_value_t::default(),
            },
            Revision::Working => svn_opt_revision_t {
                kind: subversion_sys::svn_opt_revision_kind_svn_opt_revision_working,
                value: svn_opt_revision_value_t::default(),
            },
            Revision::Head => svn_opt_revision_t {
                kind: subversion_sys::svn_opt_revision_kind_svn_opt_revision_head,
                value: svn_opt_revision_value_t::default(),
            },
        }
    }
}

impl From<svn_opt_revision_t> for Revision {
    fn from(revision: svn_opt_revision_t) -> Self {
        match revision.kind {
            subversion_sys::svn_opt_revision_kind_svn_opt_revision_unspecified => {
                Revision::Unspecified
            }
            subversion_sys::svn_opt_revision_kind_svn_opt_revision_number => unsafe {
                Revision::Number(Revnum(*revision.value.number.as_ref()))
            },
            subversion_sys::svn_opt_revision_kind_svn_opt_revision_date => unsafe {
                Revision::Date(*revision.value.date.as_ref())
            },
            subversion_sys::svn_opt_revision_kind_svn_opt_revision_committed => Revision::Committed,
            subversion_sys::svn_opt_revision_kind_svn_opt_revision_previous => Revision::Previous,
            subversion_sys::svn_opt_revision_kind_svn_opt_revision_base => Revision::Base,
            subversion_sys::svn_opt_revision_kind_svn_opt_revision_working => Revision::Working,
            subversion_sys::svn_opt_revision_kind_svn_opt_revision_head => Revision::Head,
            _ => Revision::Unspecified,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Depth {
    #[default]
    Unknown,
    Exclude,
    Empty,
    Files,
    Immediates,
    Infinity,
}

impl From<Depth> for subversion_sys::svn_depth_t {
    fn from(depth: Depth) -> Self {
        match depth {
            Depth::Unknown => subversion_sys::svn_depth_t_svn_depth_unknown,
            Depth::Exclude => subversion_sys::svn_depth_t_svn_depth_exclude,
            Depth::Empty => subversion_sys::svn_depth_t_svn_depth_empty,
            Depth::Files => subversion_sys::svn_depth_t_svn_depth_files,
            Depth::Immediates => subversion_sys::svn_depth_t_svn_depth_immediates,
            Depth::Infinity => subversion_sys::svn_depth_t_svn_depth_infinity,
        }
    }
}

impl From<subversion_sys::svn_depth_t> for Depth {
    fn from(depth: subversion_sys::svn_depth_t) -> Self {
        match depth {
            subversion_sys::svn_depth_t_svn_depth_unknown => Depth::Unknown,
            subversion_sys::svn_depth_t_svn_depth_exclude => Depth::Exclude,
            subversion_sys::svn_depth_t_svn_depth_empty => Depth::Empty,
            subversion_sys::svn_depth_t_svn_depth_files => Depth::Files,
            subversion_sys::svn_depth_t_svn_depth_immediates => Depth::Immediates,
            subversion_sys::svn_depth_t_svn_depth_infinity => Depth::Infinity,
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

pub struct CommitInfo<'pool> {
    ptr: *const subversion_sys::svn_commit_info_t,
    _pool: std::marker::PhantomData<&'pool apr::Pool>,
}
unsafe impl Send for CommitInfo<'_> {}

impl<'pool> CommitInfo<'pool> {
    pub fn from_raw(ptr: *const subversion_sys::svn_commit_info_t) -> Self {
        Self {
            ptr,
            _pool: std::marker::PhantomData,
        }
    }

    pub fn revision(&self) -> Revnum {
        Revnum::from_raw(unsafe { (*self.ptr).revision }).unwrap()
    }

    pub fn date(&self) -> &str {
        unsafe {
            let date = (*self.ptr).date;
            std::ffi::CStr::from_ptr(date).to_str().unwrap()
        }
    }

    pub fn author(&self) -> &str {
        unsafe {
            let author = (*self.ptr).author;
            std::ffi::CStr::from_ptr(author).to_str().unwrap()
        }
    }
    pub fn post_commit_err(&self) -> Option<&str> {
        unsafe {
            let err = (*self.ptr).post_commit_err;
            if err.is_null() {
                None
            } else {
                Some(std::ffi::CStr::from_ptr(err).to_str().unwrap())
            }
        }
    }
    pub fn repos_root(&self) -> &str {
        unsafe {
            let repos_root = (*self.ptr).repos_root;
            std::ffi::CStr::from_ptr(repos_root).to_str().unwrap()
        }
    }

    pub fn dup(&self, pool: &'pool apr::Pool) -> Result<CommitInfo<'pool>, Error> {
        unsafe {
            let duplicated = subversion_sys::svn_commit_info_dup(self.ptr, pool.as_mut_ptr());
            Ok(CommitInfo::from_raw(duplicated))
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RevisionRange {
    pub start: Revision,
    pub end: Revision,
}

impl RevisionRange {
    pub fn new(start: Revision, end: Revision) -> Self {
        Self { start, end }
    }
}

impl From<&RevisionRange> for subversion_sys::svn_opt_revision_range_t {
    fn from(range: &RevisionRange) -> Self {
        Self {
            start: range.start.into(),
            end: range.end.into(),
        }
    }
}

impl RevisionRange {
    pub unsafe fn to_c(&self, pool: &apr::Pool) -> *mut subversion_sys::svn_opt_revision_range_t {
        let range = pool.calloc::<subversion_sys::svn_opt_revision_range_t>();
        *range = self.into();
        range
    }
}

pub struct LogEntry<'pool> {
    ptr: *const subversion_sys::svn_log_entry_t,
    _pool: std::marker::PhantomData<&'pool apr::Pool>,
}
unsafe impl Send for LogEntry<'_> {}

impl<'pool> LogEntry<'pool> {
    pub fn from_raw(ptr: *const subversion_sys::svn_log_entry_t) -> Self {
        Self {
            ptr,
            _pool: std::marker::PhantomData,
        }
    }

    pub fn dup(&self, pool: &'pool apr::Pool) -> Result<LogEntry<'pool>, Error> {
        unsafe {
            let duplicated = subversion_sys::svn_log_entry_dup(self.ptr, pool.as_mut_ptr());
            Ok(LogEntry::from_raw(duplicated))
        }
    }

    pub fn as_ptr(&self) -> *const subversion_sys::svn_log_entry_t {
        self.ptr
    }

    /// Get the revision number
    pub fn revision(&self) -> Option<Revnum> {
        unsafe {
            let rev = (*self.ptr).revision;
            Revnum::from_raw(rev)
        }
    }

    /// Get the log message
    pub fn message(&self) -> Option<&str> {
        self.get_revprop("svn:log")
    }

    /// Get the author
    pub fn author(&self) -> Option<&str> {
        self.get_revprop("svn:author")
    }

    /// Get the date as a string
    pub fn date(&self) -> Option<&str> {
        self.get_revprop("svn:date")
    }

    /// Get a revision property by name
    fn get_revprop(&self, prop_name: &str) -> Option<&str> {
        unsafe {
            let revprops = (*self.ptr).revprops;
            if revprops.is_null() {
                return None;
            }

            let prop_key = std::ffi::CString::new(prop_name).ok()?;
            let value_ptr = apr_sys::apr_hash_get(
                revprops,
                prop_key.as_ptr() as *const std::ffi::c_void,
                apr_sys::APR_HASH_KEY_STRING as apr_sys::apr_ssize_t,
            );

            if value_ptr.is_null() {
                return None;
            }

            let svn_string = value_ptr as *const subversion_sys::svn_string_t;
            if svn_string.is_null() || (*svn_string).data.is_null() {
                return None;
            }

            let data_slice =
                std::slice::from_raw_parts((*svn_string).data as *const u8, (*svn_string).len);
            std::str::from_utf8(data_slice).ok()
        }
    }

    /// Check if this log entry has children
    pub fn has_children(&self) -> bool {
        unsafe { (*self.ptr).has_children != 0 }
    }
}

pub type FileSize = subversion_sys::svn_filesize_t;

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
                subversion_sys::svn_errno_t_SVN_ERR_IO_UNKNOWN_EOL.into(),
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

pub struct InheritedItem<'pool> {
    ptr: *const subversion_sys::svn_prop_inherited_item_t,
    _pool: std::marker::PhantomData<&'pool apr::Pool>,
}

impl<'pool> InheritedItem<'pool> {
    pub fn from_raw(ptr: *const subversion_sys::svn_prop_inherited_item_t) -> Self {
        Self {
            ptr,
            _pool: std::marker::PhantomData,
        }
    }

    pub fn path_or_url(&self) -> &str {
        unsafe {
            let path_or_url = (*self.ptr).path_or_url;
            std::ffi::CStr::from_ptr(path_or_url).to_str().unwrap()
        }
    }
}

#[derive(Clone)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsPathChangeKind {
    Modify,
    Add,
    Delete,
    Replace,
}

impl From<subversion_sys::svn_fs_path_change_kind_t> for FsPathChangeKind {
    fn from(kind: subversion_sys::svn_fs_path_change_kind_t) -> Self {
        match kind {
            subversion_sys::svn_fs_path_change_kind_t_svn_fs_path_change_modify => {
                FsPathChangeKind::Modify
            }
            subversion_sys::svn_fs_path_change_kind_t_svn_fs_path_change_add => {
                FsPathChangeKind::Add
            }
            subversion_sys::svn_fs_path_change_kind_t_svn_fs_path_change_delete => {
                FsPathChangeKind::Delete
            }
            subversion_sys::svn_fs_path_change_kind_t_svn_fs_path_change_replace => {
                FsPathChangeKind::Replace
            }
            _ => FsPathChangeKind::Modify, // Default case
        }
    }
}

impl From<subversion_sys::svn_node_kind_t> for NodeKind {
    fn from(kind: subversion_sys::svn_node_kind_t) -> Self {
        match kind {
            subversion_sys::svn_node_kind_t_svn_node_none => NodeKind::None,
            subversion_sys::svn_node_kind_t_svn_node_file => NodeKind::File,
            subversion_sys::svn_node_kind_t_svn_node_dir => NodeKind::Dir,
            subversion_sys::svn_node_kind_t_svn_node_unknown => NodeKind::Unknown,
            subversion_sys::svn_node_kind_t_svn_node_symlink => NodeKind::Symlink,
            n => panic!("Unknown node kind: {:?}", n),
        }
    }
}

impl From<NodeKind> for subversion_sys::svn_node_kind_t {
    fn from(kind: NodeKind) -> Self {
        match kind {
            NodeKind::None => subversion_sys::svn_node_kind_t_svn_node_none,
            NodeKind::File => subversion_sys::svn_node_kind_t_svn_node_file,
            NodeKind::Dir => subversion_sys::svn_node_kind_t_svn_node_dir,
            NodeKind::Unknown => subversion_sys::svn_node_kind_t_svn_node_unknown,
            NodeKind::Symlink => subversion_sys::svn_node_kind_t_svn_node_symlink,
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

#[cfg(feature = "wc")]
impl From<subversion_sys::svn_wc_status_kind> for StatusKind {
    fn from(kind: subversion_sys::svn_wc_status_kind) -> Self {
        match kind {
            subversion_sys::svn_wc_status_kind_svn_wc_status_none => StatusKind::None,
            subversion_sys::svn_wc_status_kind_svn_wc_status_unversioned => StatusKind::Unversioned,
            subversion_sys::svn_wc_status_kind_svn_wc_status_normal => StatusKind::Normal,
            subversion_sys::svn_wc_status_kind_svn_wc_status_added => StatusKind::Added,
            subversion_sys::svn_wc_status_kind_svn_wc_status_missing => StatusKind::Missing,
            subversion_sys::svn_wc_status_kind_svn_wc_status_deleted => StatusKind::Deleted,
            subversion_sys::svn_wc_status_kind_svn_wc_status_replaced => StatusKind::Replaced,
            subversion_sys::svn_wc_status_kind_svn_wc_status_modified => StatusKind::Modified,
            subversion_sys::svn_wc_status_kind_svn_wc_status_merged => StatusKind::Merged,
            subversion_sys::svn_wc_status_kind_svn_wc_status_conflicted => StatusKind::Conflicted,
            subversion_sys::svn_wc_status_kind_svn_wc_status_ignored => StatusKind::Ignored,
            subversion_sys::svn_wc_status_kind_svn_wc_status_obstructed => StatusKind::Obstructed,
            subversion_sys::svn_wc_status_kind_svn_wc_status_external => StatusKind::External,
            subversion_sys::svn_wc_status_kind_svn_wc_status_incomplete => StatusKind::Incomplete,
            n => panic!("Unknown status kind: {:?}", n),
        }
    }
}

#[cfg(feature = "wc")]
impl From<StatusKind> for subversion_sys::svn_wc_status_kind {
    fn from(kind: StatusKind) -> Self {
        match kind {
            StatusKind::None => subversion_sys::svn_wc_status_kind_svn_wc_status_none,
            StatusKind::Unversioned => subversion_sys::svn_wc_status_kind_svn_wc_status_unversioned,
            StatusKind::Normal => subversion_sys::svn_wc_status_kind_svn_wc_status_normal,
            StatusKind::Added => subversion_sys::svn_wc_status_kind_svn_wc_status_added,
            StatusKind::Missing => subversion_sys::svn_wc_status_kind_svn_wc_status_missing,
            StatusKind::Deleted => subversion_sys::svn_wc_status_kind_svn_wc_status_deleted,
            StatusKind::Replaced => subversion_sys::svn_wc_status_kind_svn_wc_status_replaced,
            StatusKind::Modified => subversion_sys::svn_wc_status_kind_svn_wc_status_modified,
            StatusKind::Merged => subversion_sys::svn_wc_status_kind_svn_wc_status_merged,
            StatusKind::Conflicted => subversion_sys::svn_wc_status_kind_svn_wc_status_conflicted,
            StatusKind::Ignored => subversion_sys::svn_wc_status_kind_svn_wc_status_ignored,
            StatusKind::Obstructed => subversion_sys::svn_wc_status_kind_svn_wc_status_obstructed,
            StatusKind::External => subversion_sys::svn_wc_status_kind_svn_wc_status_external,
            StatusKind::Incomplete => subversion_sys::svn_wc_status_kind_svn_wc_status_incomplete,
        }
    }
}

pub struct Lock<'pool> {
    ptr: *const subversion_sys::svn_lock_t,
    _pool: std::marker::PhantomData<&'pool apr::Pool>,
}
unsafe impl Send for Lock<'_> {}

impl<'pool> Lock<'pool> {
    pub fn from_raw(ptr: *const subversion_sys::svn_lock_t) -> Self {
        Self {
            ptr,
            _pool: std::marker::PhantomData,
        }
    }

    pub fn path(&self) -> &str {
        unsafe {
            let path = (*self.ptr).path;
            std::ffi::CStr::from_ptr(path).to_str().unwrap()
        }
    }

    pub fn dup(&self, pool: &'pool apr::Pool) -> Result<Lock<'pool>, Error> {
        unsafe {
            let duplicated = subversion_sys::svn_lock_dup(self.ptr, pool.as_mut_ptr());
            Ok(Lock::from_raw(duplicated))
        }
    }

    pub fn token(&self) -> &str {
        unsafe {
            let token = (*self.ptr).token;
            std::ffi::CStr::from_ptr(token).to_str().unwrap()
        }
    }

    pub fn owner(&self) -> &str {
        unsafe {
            let owner = (*self.ptr).owner;
            std::ffi::CStr::from_ptr(owner).to_str().unwrap()
        }
    }

    pub fn comment(&self) -> &str {
        unsafe {
            let comment = (*self.ptr).comment;
            std::ffi::CStr::from_ptr(comment).to_str().unwrap()
        }
    }

    pub fn is_dav_comment(&self) -> bool {
        unsafe { (*self.ptr).is_dav_comment == 1 }
    }

    pub fn creation_date(&self) -> i64 {
        unsafe { (*self.ptr).creation_date }
    }

    pub fn expiration_date(&self) -> apr::time::Time {
        apr::time::Time::from(unsafe { (*self.ptr).expiration_date })
    }

    pub fn create(pool: &'pool apr::Pool) -> Result<Lock<'pool>, Error> {
        unsafe {
            let lock_ptr = subversion_sys::svn_lock_create(pool.as_mut_ptr());
            Ok(Lock::from_raw(lock_ptr))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChecksumKind {
    MD5,
    SHA1,
    Fnv1a32,
    Fnv1a32x4,
}

impl From<subversion_sys::svn_checksum_kind_t> for ChecksumKind {
    fn from(kind: subversion_sys::svn_checksum_kind_t) -> Self {
        match kind {
            subversion_sys::svn_checksum_kind_t_svn_checksum_md5 => ChecksumKind::MD5,
            subversion_sys::svn_checksum_kind_t_svn_checksum_sha1 => ChecksumKind::SHA1,
            subversion_sys::svn_checksum_kind_t_svn_checksum_fnv1a_32 => ChecksumKind::Fnv1a32,
            subversion_sys::svn_checksum_kind_t_svn_checksum_fnv1a_32x4 => ChecksumKind::Fnv1a32x4,
            n => panic!("Unknown checksum kind: {:?}", n),
        }
    }
}

impl From<ChecksumKind> for subversion_sys::svn_checksum_kind_t {
    fn from(kind: ChecksumKind) -> Self {
        match kind {
            ChecksumKind::MD5 => subversion_sys::svn_checksum_kind_t_svn_checksum_md5,
            ChecksumKind::SHA1 => subversion_sys::svn_checksum_kind_t_svn_checksum_sha1,
            ChecksumKind::Fnv1a32 => subversion_sys::svn_checksum_kind_t_svn_checksum_fnv1a_32,
            ChecksumKind::Fnv1a32x4 => subversion_sys::svn_checksum_kind_t_svn_checksum_fnv1a_32x4,
        }
    }
}

pub struct LocationSegment<'pool> {
    ptr: *const subversion_sys::svn_location_segment_t,
    _pool: std::marker::PhantomData<&'pool apr::Pool>,
}
unsafe impl Send for LocationSegment<'_> {}

impl<'pool> LocationSegment<'pool> {
    pub fn from_raw(ptr: *const subversion_sys::svn_location_segment_t) -> Self {
        Self {
            ptr,
            _pool: std::marker::PhantomData,
        }
    }

    pub fn dup(&self, pool: &'pool apr::Pool) -> Result<LocationSegment<'pool>, Error> {
        unsafe {
            let duplicated = subversion_sys::svn_location_segment_dup(self.ptr, pool.as_mut_ptr());
            Ok(LocationSegment::from_raw(duplicated))
        }
    }

    pub fn range(&self) -> std::ops::Range<Revnum> {
        unsafe {
            Revnum::from_raw((*self.ptr).range_end).unwrap()
                ..Revnum::from_raw((*self.ptr).range_start).unwrap()
        }
    }

    pub fn path(&self) -> &str {
        unsafe {
            let path = (*self.ptr).path;
            std::ffi::CStr::from_ptr(path).to_str().unwrap()
        }
    }
}

#[cfg(any(feature = "ra", feature = "client"))]
pub(crate) extern "C" fn wrap_commit_callback2(
    commit_info: *const subversion_sys::svn_commit_info_t,
    baton: *mut std::ffi::c_void,
    _pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    unsafe {
        let callback =
            &mut *(baton as *mut &mut dyn FnMut(&crate::CommitInfo) -> Result<(), Error>);
        let commit_info = crate::CommitInfo::from_raw(commit_info);
        match callback(&commit_info) {
            Ok(()) => std::ptr::null_mut(),
            Err(mut err) => err.as_mut_ptr(),
        }
    }
}

#[cfg(any(feature = "ra", feature = "client"))]
pub(crate) extern "C" fn wrap_log_entry_receiver(
    baton: *mut std::ffi::c_void,
    log_entry: *mut subversion_sys::svn_log_entry_t,
    _pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    eprintln!(
        "wrap_log_entry_receiver called with baton={:p}, log_entry={:p}",
        baton, log_entry
    );
    unsafe {
        // Use single dereference pattern like commit callback: &mut *(baton as *mut &mut dyn FnMut(...))
        eprintln!("  Casting baton to single-boxed callback");
        let callback = &mut *(baton as *mut &mut dyn FnMut(&LogEntry) -> Result<(), Error>);
        eprintln!("  Creating LogEntry from raw");
        let log_entry = LogEntry::from_raw(log_entry);
        eprintln!("  Calling callback");
        let ret = callback(&log_entry);
        eprintln!("  Callback returned, processing result");
        if let Err(mut err) = ret {
            err.as_mut_ptr()
        } else {
            std::ptr::null_mut()
        }
    }
}

extern "C" fn wrap_cancel_func(
    cancel_baton: *mut std::ffi::c_void,
) -> *mut subversion_sys::svn_error_t {
    let cancel_check = unsafe { &*(cancel_baton as *const Box<dyn Fn() -> Result<(), Error>>) };
    match cancel_check() {
        Ok(()) => std::ptr::null_mut(),
        Err(e) => unsafe { e.into_raw() },
    }
}

/// Conflict resolution choice for text and property conflicts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextConflictChoice {
    /// Postpone resolution for later
    Postpone,
    /// Use base version (original)
    Base,
    /// Use their version (incoming changes)
    TheirsFull,
    /// Use my version (local changes)
    MineFull,
    /// Use their version for conflicts only
    TheirsConflict,
    /// Use my version for conflicts only
    MineConflict,
    /// Use a merged version
    Merged,
    /// Undefined/unspecified
    Unspecified,
}

impl From<TextConflictChoice> for subversion_sys::svn_client_conflict_option_id_t {
    fn from(choice: TextConflictChoice) -> Self {
        match choice {
            TextConflictChoice::Unspecified => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_undefined,
            TextConflictChoice::Postpone => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_postpone,
            TextConflictChoice::Base => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_base_text,
            TextConflictChoice::TheirsFull => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_incoming_text,
            TextConflictChoice::MineFull => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_working_text,
            TextConflictChoice::TheirsConflict => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_incoming_text_where_conflicted,
            TextConflictChoice::MineConflict => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_working_text_where_conflicted,
            TextConflictChoice::Merged => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_merged_text,
        }
    }
}

/// Conflict resolution choice for tree conflicts  
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeConflictChoice {
    /// Postpone resolution for later
    Postpone,
    /// Accept current working copy state
    AcceptCurrentState,
    /// Accept incoming deletion
    AcceptIncomingDelete,
    /// Ignore incoming deletion (keep local)
    IgnoreIncomingDelete,
    /// Update moved destination
    UpdateMoveDestination,
    /// Accept incoming addition
    AcceptIncomingAdd,
    /// Ignore incoming addition
    IgnoreIncomingAdd,
}

impl From<TreeConflictChoice> for subversion_sys::svn_client_conflict_option_id_t {
    fn from(choice: TreeConflictChoice) -> Self {
        match choice {
            TreeConflictChoice::Postpone => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_postpone,
            TreeConflictChoice::AcceptCurrentState => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_accept_current_wc_state,
            TreeConflictChoice::AcceptIncomingDelete => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_incoming_delete_accept,
            TreeConflictChoice::IgnoreIncomingDelete => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_incoming_delete_ignore,
            TreeConflictChoice::UpdateMoveDestination => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_update_move_destination,
            TreeConflictChoice::AcceptIncomingAdd => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_incoming_add_ignore,
            TreeConflictChoice::IgnoreIncomingAdd => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_incoming_delete_ignore,
        }
    }
}

/// Legacy conflict choice enum for backward compatibility with WC functions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictChoice {
    Postpone,
    Base,
    TheirsFull,
    MineFull,
    TheirsConflict,
    MineConflict,
    Merged,
    Unspecified,
}

impl From<ConflictChoice> for subversion_sys::svn_wc_conflict_choice_t {
    fn from(choice: ConflictChoice) -> Self {
        match choice {
            ConflictChoice::Postpone => {
                subversion_sys::svn_wc_conflict_choice_t_svn_wc_conflict_choose_postpone
            }
            ConflictChoice::Base => {
                subversion_sys::svn_wc_conflict_choice_t_svn_wc_conflict_choose_base
            }
            ConflictChoice::TheirsFull => {
                subversion_sys::svn_wc_conflict_choice_t_svn_wc_conflict_choose_theirs_full
            }
            ConflictChoice::MineFull => {
                subversion_sys::svn_wc_conflict_choice_t_svn_wc_conflict_choose_mine_full
            }
            ConflictChoice::TheirsConflict => {
                subversion_sys::svn_wc_conflict_choice_t_svn_wc_conflict_choose_theirs_conflict
            }
            ConflictChoice::MineConflict => {
                subversion_sys::svn_wc_conflict_choice_t_svn_wc_conflict_choose_mine_conflict
            }
            ConflictChoice::Merged => {
                subversion_sys::svn_wc_conflict_choice_t_svn_wc_conflict_choose_merged
            }
            ConflictChoice::Unspecified => {
                subversion_sys::svn_wc_conflict_choice_t_svn_wc_conflict_choose_unspecified
            }
        }
    }
}

pub struct Checksum<'pool> {
    ptr: *const subversion_sys::svn_checksum_t,
    _pool: std::marker::PhantomData<&'pool apr::Pool>,
}

impl<'pool> Checksum<'pool> {
    pub fn from_raw(ptr: *const subversion_sys::svn_checksum_t) -> Self {
        Self {
            ptr,
            _pool: std::marker::PhantomData,
        }
    }

    pub fn kind(&self) -> ChecksumKind {
        ChecksumKind::from(unsafe { (*self.ptr).kind })
    }

    pub fn size(&self) -> usize {
        unsafe { subversion_sys::svn_checksum_size(self.ptr) }
    }

    pub fn is_empty(&self) -> bool {
        unsafe { subversion_sys::svn_checksum_is_empty_checksum(self.ptr as *mut _) == 1 }
    }

    pub fn digest(&self) -> &[u8] {
        unsafe {
            let digest = (*self.ptr).digest;
            std::slice::from_raw_parts(digest, self.size())
        }
    }

    pub fn parse_hex(kind: ChecksumKind, hex: &str, pool: &'pool apr::Pool) -> Result<Self, Error> {
        let mut checksum = std::ptr::null_mut();
        let kind = kind.into();
        let hex = std::ffi::CString::new(hex).unwrap();
        unsafe {
            Error::from_raw(subversion_sys::svn_checksum_parse_hex(
                &mut checksum,
                kind,
                hex.as_ptr(),
                pool.as_mut_ptr(),
            ))?;
            Ok(Self::from_raw(checksum))
        }
    }

    pub fn empty(kind: ChecksumKind, pool: &'pool apr::Pool) -> Result<Self, Error> {
        let kind = kind.into();
        unsafe {
            let checksum = subversion_sys::svn_checksum_empty_checksum(kind, pool.as_mut_ptr());
            Ok(Self::from_raw(checksum))
        }
    }

    /// Create a new checksum from data
    pub fn create(kind: ChecksumKind, data: &[u8], pool: &'pool apr::Pool) -> Result<Self, Error> {
        checksum(kind, data, pool)
    }

    /// Compare two checksums for equality
    pub fn matches(&self, other: &Checksum) -> bool {
        unsafe { subversion_sys::svn_checksum_match(self.ptr, other.ptr) != 0 }
    }

    /// Duplicate this checksum into a new pool
    pub fn dup(&self, pool: &'pool apr::Pool) -> Result<Checksum<'pool>, Error> {
        unsafe {
            let new_checksum = subversion_sys::svn_checksum_dup(self.ptr, pool.as_mut_ptr());
            if new_checksum.is_null() {
                Err(Error::from_str("Failed to duplicate checksum"))
            } else {
                Ok(Checksum::from_raw(new_checksum))
            }
        }
    }

    /// Serialize checksum to a string representation
    pub fn serialize(&self, pool: &apr::Pool) -> Result<String, Error> {
        unsafe {
            let serialized = subversion_sys::svn_checksum_serialize(
                self.ptr,
                pool.as_mut_ptr(),
                pool.as_mut_ptr(),
            );
            if serialized.is_null() {
                Err(Error::from_str("Failed to serialize checksum"))
            } else {
                let cstr = std::ffi::CStr::from_ptr(serialized);
                Ok(cstr.to_string_lossy().into_owned())
            }
        }
    }

    /// Deserialize checksum from a string representation
    pub fn deserialize(data: &str, pool: &'pool apr::Pool) -> Result<Self, Error> {
        let data_cstr = std::ffi::CString::new(data)?;
        let mut checksum = std::ptr::null();
        unsafe {
            let err = subversion_sys::svn_checksum_deserialize(
                &mut checksum,
                data_cstr.as_ptr(),
                pool.as_mut_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            if checksum.is_null() {
                Err(Error::from_str("Failed to deserialize checksum"))
            } else {
                Ok(Checksum::from_raw(checksum))
            }
        }
    }

    /// Convert checksum to hexadecimal string
    pub fn to_hex(&self, pool: &apr::Pool) -> String {
        unsafe {
            let hex = subversion_sys::svn_checksum_to_cstring(self.ptr, pool.as_mut_ptr());
            if hex.is_null() {
                String::new()
            } else {
                let cstr = std::ffi::CStr::from_ptr(hex);
                cstr.to_string_lossy().into_owned()
            }
        }
    }

    /// Convert checksum for display
    pub fn to_display(&self, pool: &apr::Pool) -> String {
        unsafe {
            let display =
                subversion_sys::svn_checksum_to_cstring_display(self.ptr, pool.as_mut_ptr());
            if display.is_null() {
                String::new()
            } else {
                let cstr = std::ffi::CStr::from_ptr(display);
                cstr.to_string_lossy().into_owned()
            }
        }
    }

    /// Check if this checksum has a known size for its type
    pub fn has_known_size(&self, size: usize) -> bool {
        let kind = self.kind();
        match kind {
            ChecksumKind::MD5 => size == 16,
            ChecksumKind::SHA1 => size == 20,
            ChecksumKind::Fnv1a32 => size == 4,
            ChecksumKind::Fnv1a32x4 => size == 16,
        }
    }
}

pub struct ChecksumContext<'pool> {
    ptr: *mut subversion_sys::svn_checksum_ctx_t,
    _pool: std::marker::PhantomData<&'pool apr::Pool>,
}

impl<'pool> ChecksumContext<'pool> {
    pub fn new(kind: ChecksumKind, pool: &'pool apr::Pool) -> Result<Self, Error> {
        let kind = kind.into();
        unsafe {
            let cc = subversion_sys::svn_checksum_ctx_create(kind, pool.as_mut_ptr());
            Ok(Self {
                ptr: cc,
                _pool: std::marker::PhantomData,
            })
        }
    }

    pub fn reset(&mut self) -> Result<(), Error> {
        let err = unsafe { subversion_sys::svn_checksum_ctx_reset(self.ptr) };
        Error::from_raw(err)?;
        Ok(())
    }

    pub fn update(&mut self, data: &[u8]) -> Result<(), Error> {
        let err = unsafe {
            subversion_sys::svn_checksum_update(
                self.ptr,
                data.as_ptr() as *const std::ffi::c_void,
                data.len(),
            )
        };
        Error::from_raw(err)?;
        Ok(())
    }

    pub fn finish(&self, result_pool: &'pool apr::Pool) -> Result<Checksum<'pool>, Error> {
        let mut checksum = std::ptr::null_mut();
        unsafe {
            Error::from_raw(subversion_sys::svn_checksum_final(
                &mut checksum,
                self.ptr,
                result_pool.as_mut_ptr(),
            ))?;
            Ok(Checksum::from_raw(checksum))
        }
    }
}

pub fn checksum<'pool>(
    kind: ChecksumKind,
    data: &[u8],
    pool: &'pool apr::Pool,
) -> Result<Checksum<'pool>, Error> {
    let mut checksum = std::ptr::null_mut();
    let kind = kind.into();
    unsafe {
        Error::from_raw(subversion_sys::svn_checksum(
            &mut checksum,
            kind,
            data.as_ptr() as *const std::ffi::c_void,
            data.len(),
            pool.as_mut_ptr(),
        ))?;
        Ok(Checksum::from_raw(checksum))
    }
}

/// Helper functions for working with svn_string_t directly
mod svn_string_helpers {
    use subversion_sys::svn_string_t;

    /// Get the data from an svn_string_t as bytes
    pub fn as_bytes(s: &svn_string_t) -> &[u8] {
        if s.data.is_null() || s.len == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(s.data as *const u8, s.len) }
        }
    }

    /// Get the data from an svn_string_t as a Vec<u8>
    pub fn to_vec(s: &svn_string_t) -> Vec<u8> {
        as_bytes(s).to_vec()
    }

    /// Create a new svn_string_t from bytes
    pub fn svn_string_ncreate(data: &[u8], pool: &apr::Pool) -> *mut svn_string_t {
        unsafe {
            subversion_sys::svn_string_ncreate(
                data.as_ptr() as *const i8,
                data.len(),
                pool.as_mut_ptr(),
            )
        }
    }
}

// Re-export the helper functions
pub use svn_string_helpers::*;
