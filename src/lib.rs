//! Rust bindings for the Subversion version control system.
//!
//! This crate provides idiomatic Rust bindings for the Subversion C libraries,
//! enabling Rust applications to interact with Subversion repositories and working copies.
//!
//! # Overview
//!
//! The `subversion` crate offers comprehensive access to Subversion's functionality through
//! several modules, each corresponding to a major component of the Subversion API:
//!
//! - **`client`** - High-level client operations (checkout, commit, update, diff, merge, etc.)
//! - **`wc`** - Working copy management and status operations
//! - **`ra`** - Repository access layer for network operations
//! - **`repos`** - Repository administration (create, load, dump, verify)
//! - **`fs`** - Filesystem layer for direct repository access
//! - **`delta`** - Editor interface for efficient tree transformations
//!
//! # Features
//!
//! Enable specific functionality via Cargo features:
//!
//! - `client` - Client operations
//! - `wc` - Working copy management
//! - `ra` - Repository access layer
//! - `delta` - Delta/editor operations
//! - `repos` - Repository administration
//! - `url` - URL parsing utilities
//!
//! Default features: `["ra", "wc", "client", "delta", "repos"]`
//!
//! # Error Handling
//!
//! All operations return a [`Result<T, Error>`](Error) where [`Error`] wraps Subversion's
//! error chain. Errors can be inspected for detailed information:
//!
//! ```no_run
//! use subversion::client::Context;
//!
//! let mut ctx = Context::new().unwrap();
//! match ctx.checkout("https://svn.example.com/repo", "/tmp/wc", None, true) {
//!     Ok(_) => println!("Checkout succeeded"),
//!     Err(e) => {
//!         eprintln!("Error: {}", e.full_message());
//!         eprintln!("At: {:?}", e.location());
//!     }
//! }
//! ```
//!
//! # Thread Safety
//!
//! The Subversion libraries are not thread-safe. Each thread should have its own
//! [`client::Context`] or other Subversion objects.

#![deny(missing_docs)]

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

/// Authentication and credential management.
pub mod auth;
/// Cache configuration and management.
#[cfg(feature = "cache")]
pub mod cache;
/// Subversion client operations.
#[cfg(feature = "client")]
pub mod client;
/// Command-line utilities and helpers.
#[cfg(feature = "cmdline")]
pub mod cmdline;
/// Configuration file handling.
pub mod config;
/// Conflict resolution for working copy operations.
#[cfg(feature = "client")]
pub mod conflict;
/// Delta editor for tree modifications.
#[cfg(feature = "delta")]
pub mod delta;
/// Diff generation and processing.
pub mod diff;
/// Directory entry operations.
pub mod dirent;
/// Error handling types and utilities.
pub mod error;
/// Filesystem backend for repositories.
pub mod fs;
/// Hash table utilities.
pub mod hash;
/// Initialization utilities.
pub mod init;
/// Input/output stream handling.
pub mod io;
/// Iterator utilities for Subversion data structures.
pub mod iter;
/// Merge operations for branches.
#[cfg(feature = "client")]
pub mod merge;
/// Merge tracking information.
pub mod mergeinfo;
/// Native language support and internationalization.
#[cfg(feature = "nls")]
pub mod nls;
/// Option parsing and command-line argument handling.
pub mod opt;
/// Property management for versioned items.
pub mod props;
/// Repository access layer for remote operations.
#[cfg(feature = "ra")]
pub mod ra;
/// Repository administration and management.
#[cfg(feature = "repos")]
pub mod repos;
/// Sorting utilities for Subversion data.
#[cfg(feature = "sorts")]
pub mod sorts;
/// String manipulation utilities.
pub mod string;
/// Keyword and EOL substitution.
pub mod subst;
/// Time and date utilities.
pub mod time;
/// URI manipulation and validation.
pub mod uri;
/// UTF-8 string validation and conversion.
#[cfg(feature = "utf")]
pub mod utf;
/// Version information and compatibility checking.
pub mod version;
/// Working copy management and operations.
#[cfg(feature = "wc")]
pub mod wc;
/// X.509 certificate handling.
#[cfg(feature = "x509")]
pub mod x509;
/// XML parsing and generation utilities.
#[cfg(feature = "xml")]
pub mod xml;
use bitflags::bitflags;
use std::str::FromStr;
use subversion_sys::{svn_opt_revision_t, svn_opt_revision_value_t};

pub use version::Version;

// Re-export important types for API consumers
#[cfg(feature = "repos")]
pub use repos::{LoadUUID, Notify};

bitflags! {
    /// Flags indicating which fields are present in a directory entry.
    pub struct DirentField: u32 {
        /// Node kind field is present.
        const Kind = subversion_sys::SVN_DIRENT_KIND;
        /// File size field is present.
        const Size = subversion_sys::SVN_DIRENT_SIZE;
        /// Has properties field is present.
        const HasProps = subversion_sys::SVN_DIRENT_HAS_PROPS;
        /// Created revision field is present.
        const CreatedRevision = subversion_sys::SVN_DIRENT_CREATED_REV;
        /// Modification time field is present.
        const Time = subversion_sys::SVN_DIRENT_TIME;
        /// Last author field is present.
        const LastAuthor = subversion_sys::SVN_DIRENT_LAST_AUTHOR;
    }
}

/// A Subversion revision number.
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
    /// Creates a Revnum from a raw svn_revnum_t value.
    /// Returns None if the value is negative (invalid revision).
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

/// A revision specification.
#[derive(Debug, Default, Clone, Copy)]
pub enum Revision {
    /// No revision specified.
    #[default]
    Unspecified,
    /// A specific revision number.
    Number(Revnum),
    /// A revision at a specific date/time.
    Date(i64),
    /// The last committed revision.
    Committed,
    /// The revision before the last committed revision.
    Previous,
    /// The base revision of the working copy.
    Base,
    /// The working copy revision.
    Working,
    /// The latest revision in the repository.
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

/// The depth of a Subversion operation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Depth {
    /// Depth not specified or unknown.
    #[default]
    Unknown,
    /// Exclude the item.
    Exclude,
    /// Just the item itself.
    Empty,
    /// The item and its file children.
    Files,
    /// The item and its immediate children.
    Immediates,
    /// The item and all descendants.
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

/// Information about a committed revision.
pub struct CommitInfo<'pool> {
    ptr: *const subversion_sys::svn_commit_info_t,
    _pool: std::marker::PhantomData<&'pool apr::Pool<'static>>,
}
unsafe impl Send for CommitInfo<'_> {}

impl<'pool> CommitInfo<'pool> {
    /// Creates a CommitInfo from a raw pointer.
    pub fn from_raw(ptr: *const subversion_sys::svn_commit_info_t) -> Self {
        Self {
            ptr,
            _pool: std::marker::PhantomData,
        }
    }

    /// Returns the revision number of the commit.
    pub fn revision(&self) -> Revnum {
        Revnum::from_raw(unsafe { (*self.ptr).revision }).unwrap()
    }

    /// Returns the date of the commit.
    pub fn date(&self) -> &str {
        unsafe {
            let date = (*self.ptr).date;
            std::ffi::CStr::from_ptr(date).to_str().unwrap()
        }
    }

    /// Returns the author of the commit.
    pub fn author(&self) -> &str {
        unsafe {
            let author = (*self.ptr).author;
            std::ffi::CStr::from_ptr(author).to_str().unwrap()
        }
    }
    /// Returns any post-commit error message.
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
    /// Returns the repository root URL.
    pub fn repos_root(&self) -> &str {
        unsafe {
            let repos_root = (*self.ptr).repos_root;
            std::ffi::CStr::from_ptr(repos_root).to_str().unwrap()
        }
    }

    /// Duplicates the commit info in the given pool.
    pub fn dup(&self, pool: &'pool apr::Pool<'pool>) -> Result<CommitInfo<'pool>, Error> {
        unsafe {
            let duplicated = subversion_sys::svn_commit_info_dup(self.ptr, pool.as_mut_ptr());
            Ok(CommitInfo::from_raw(duplicated))
        }
    }
}

/// A range of revisions.
#[derive(Debug, Clone, Copy, Default)]
pub struct RevisionRange {
    /// Starting revision of the range.
    pub start: Revision,
    /// Ending revision of the range.
    pub end: Revision,
}

impl RevisionRange {
    /// Creates a new revision range.
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
    /// Converts to a C revision range structure.
    pub unsafe fn to_c(&self, pool: &apr::Pool) -> *mut subversion_sys::svn_opt_revision_range_t {
        let range = pool.calloc::<subversion_sys::svn_opt_revision_range_t>();
        *range = self.into();
        range
    }
}

/// A log entry from the repository history.
pub struct LogEntry<'pool> {
    ptr: *const subversion_sys::svn_log_entry_t,
    _pool: std::marker::PhantomData<&'pool apr::Pool<'static>>,
}
unsafe impl Send for LogEntry<'_> {}

impl<'pool> LogEntry<'pool> {
    /// Creates a LogEntry from a raw pointer.
    pub fn from_raw(ptr: *const subversion_sys::svn_log_entry_t) -> Self {
        Self {
            ptr,
            _pool: std::marker::PhantomData,
        }
    }

    /// Duplicates the log entry in the given pool.
    pub fn dup(&self, pool: &'pool apr::Pool<'pool>) -> Result<LogEntry<'pool>, Error> {
        unsafe {
            let duplicated = subversion_sys::svn_log_entry_dup(self.ptr, pool.as_mut_ptr());
            Ok(LogEntry::from_raw(duplicated))
        }
    }

    /// Returns the raw pointer to the log entry.
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

            // Use TypedHash directly to get a reference with the correct lifetime
            let hash = apr::hash::TypedHash::<subversion_sys::svn_string_t>::from_ptr(revprops);
            let svn_string = hash.get_ref(prop_name)?;

            if svn_string.data.is_null() {
                return None;
            }

            let data_slice =
                std::slice::from_raw_parts(svn_string.data as *const u8, svn_string.len);
            std::str::from_utf8(data_slice).ok()
        }
    }

    /// Check if this log entry has children
    pub fn has_children(&self) -> bool {
        unsafe { (*self.ptr).has_children != 0 }
    }

    /// Whether this revision should be interpreted as non-inheritable
    ///
    /// Only set when this log entry is returned by the mergeinfo APIs.
    pub fn non_inheritable(&self) -> bool {
        unsafe { (*self.ptr).non_inheritable != 0 }
    }

    /// Whether this revision is a merged revision resulting from a reverse merge
    pub fn subtractive_merge(&self) -> bool {
        unsafe { (*self.ptr).subtractive_merge != 0 }
    }

    /// Get all revision properties as a HashMap
    pub fn revprops(&self) -> std::collections::HashMap<String, Vec<u8>> {
        unsafe {
            let revprops = (*self.ptr).revprops;
            if revprops.is_null() {
                return std::collections::HashMap::new();
            }

            let hash = apr::hash::TypedHash::<subversion_sys::svn_string_t>::from_ptr(revprops);
            let mut result = std::collections::HashMap::new();
            for (key, svn_string) in hash.iter() {
                if !svn_string.data.is_null() {
                    let data_slice =
                        std::slice::from_raw_parts(svn_string.data as *const u8, svn_string.len);
                    let key_str = std::str::from_utf8(key)
                        .expect("revprop key is not valid UTF-8")
                        .to_string();
                    result.insert(key_str, data_slice.to_vec());
                }
            }
            result
        }
    }

    /// Get the changed paths for this log entry
    ///
    /// Returns None if changed paths were not requested in the log operation.
    pub fn changed_paths(&self) -> Option<std::collections::HashMap<String, LogChangedPath>> {
        unsafe {
            let changed_paths2 = (*self.ptr).changed_paths2;
            if changed_paths2.is_null() {
                return None;
            }

            let hash = apr::hash::TypedHash::<subversion_sys::svn_log_changed_path2_t>::from_ptr(
                changed_paths2,
            );
            let mut result = std::collections::HashMap::new();
            for (key, changed_path) in hash.iter() {
                let key_str = std::str::from_utf8(key)
                    .expect("changed path key is not valid UTF-8")
                    .to_string();
                result.insert(key_str, LogChangedPath::from_raw(changed_path));
            }
            Some(result)
        }
    }
}

/// A changed path entry from a log entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogChangedPath {
    /// The action: 'A'dd, 'D'elete, 'R'eplace, 'M'odify
    pub action: char,
    /// Source path of copy (if any)
    pub copyfrom_path: Option<String>,
    /// Source revision of copy (if any)
    pub copyfrom_rev: Option<Revnum>,
    /// The type of the node
    pub node_kind: NodeKind,
    /// Whether text was modified (None if unknown)
    pub text_modified: Option<bool>,
    /// Whether properties were modified (None if unknown)
    pub props_modified: Option<bool>,
}

impl LogChangedPath {
    fn from_raw(raw: &subversion_sys::svn_log_changed_path2_t) -> Self {
        let copyfrom_path = if raw.copyfrom_path.is_null() {
            None
        } else {
            unsafe {
                Some(
                    std::ffi::CStr::from_ptr(raw.copyfrom_path)
                        .to_string_lossy()
                        .into_owned(),
                )
            }
        };

        let copyfrom_rev = Revnum::from_raw(raw.copyfrom_rev);

        fn tristate_to_option(ts: subversion_sys::svn_tristate_t) -> Option<bool> {
            match ts {
                subversion_sys::svn_tristate_t_svn_tristate_false => Some(false),
                subversion_sys::svn_tristate_t_svn_tristate_true => Some(true),
                _ => None, // svn_tristate_unknown
            }
        }

        Self {
            action: raw.action as u8 as char,
            copyfrom_path,
            copyfrom_rev,
            node_kind: raw.node_kind.into(),
            text_modified: tristate_to_option(raw.text_modified),
            props_modified: tristate_to_option(raw.props_modified),
        }
    }
}

/// File size type.
pub type FileSize = subversion_sys::svn_filesize_t;

/// Native end-of-line style.
#[derive(Debug, Clone, Copy, Default)]
pub enum NativeEOL {
    /// Use the standard EOL for the platform.
    #[default]
    Standard,
    /// Unix-style line feed.
    LF,
    /// Windows-style carriage return + line feed.
    CRLF,
    /// Mac-style carriage return.
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

/// An inherited property item.
pub struct InheritedItem<'pool> {
    ptr: *const subversion_sys::svn_prop_inherited_item_t,
    _pool: std::marker::PhantomData<&'pool apr::Pool<'static>>,
}

impl<'pool> InheritedItem<'pool> {
    /// Creates an InheritedItem from a raw pointer.
    pub fn from_raw(ptr: *const subversion_sys::svn_prop_inherited_item_t) -> Self {
        Self {
            ptr,
            _pool: std::marker::PhantomData,
        }
    }

    /// Returns the path or URL of the item.
    pub fn path_or_url(&self) -> &str {
        unsafe {
            let path_or_url = (*self.ptr).path_or_url;
            std::ffi::CStr::from_ptr(path_or_url).to_str().unwrap()
        }
    }
}

/// A canonicalized path or URL.
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

/// The kind of a node in the repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    /// Not a node.
    None,
    /// Regular file.
    File,
    /// Directory.
    Dir,
    /// Unknown node kind.
    Unknown,
    /// Symbolic link.
    Symlink,
}

/// The kind of change made to a path in the filesystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FsPathChangeKind {
    /// Path was modified.
    Modify,
    /// Path was added.
    Add,
    /// Path was deleted.
    Delete,
    /// Path was replaced.
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

/// The status of a working copy item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    /// No status.
    None,
    /// Not under version control.
    Unversioned,
    /// Normal status.
    Normal,
    /// Scheduled for addition.
    Added,
    /// Missing from working copy.
    Missing,
    /// Scheduled for deletion.
    Deleted,
    /// Replaced in the working copy.
    Replaced,
    /// Modified in the working copy.
    Modified,
    /// Merged from another branch.
    Merged,
    /// Has conflicts.
    Conflicted,
    /// Ignored by version control.
    Ignored,
    /// Obstructed by an unversioned item.
    Obstructed,
    /// External definition.
    External,
    /// Incomplete status.
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

/// A lock on a path in the repository.
///
/// The lock data is allocated in a pool. The lifetime parameter ensures
/// the lock doesn't outlive the object it was obtained from (e.g., Session or Fs).
pub struct Lock<'a> {
    ptr: *const subversion_sys::svn_lock_t,
    _pool: apr::PoolHandle<'static>,
    _lifetime: std::marker::PhantomData<&'a ()>,
}

unsafe impl Send for Lock<'_> {}

impl<'a> Lock<'a> {
    /// Creates a Lock from a raw pointer with a pool.
    ///
    /// The lock data must be allocated in the provided pool.
    pub fn from_raw(
        ptr: *const subversion_sys::svn_lock_t,
        pool: apr::PoolHandle<'static>,
    ) -> Self {
        Self {
            ptr,
            _pool: pool,
            _lifetime: std::marker::PhantomData,
        }
    }

    /// Returns the path that is locked.
    pub fn path(&self) -> &str {
        unsafe {
            let path = (*self.ptr).path;
            std::ffi::CStr::from_ptr(path).to_str().unwrap()
        }
    }

    /// Duplicates the lock in a new pool.
    pub fn dup(&self) -> Result<Lock<'static>, Error> {
        let pool = apr::Pool::new();
        unsafe {
            let duplicated = subversion_sys::svn_lock_dup(self.ptr, pool.as_mut_ptr());
            let pool_handle = apr::PoolHandle::owned(pool);
            Ok(Lock::from_raw(duplicated, pool_handle))
        }
    }

    /// Returns the lock token.
    pub fn token(&self) -> &str {
        unsafe {
            let token = (*self.ptr).token;
            std::ffi::CStr::from_ptr(token).to_str().unwrap()
        }
    }

    /// Returns the owner of the lock.
    pub fn owner(&self) -> &str {
        unsafe {
            let owner = (*self.ptr).owner;
            std::ffi::CStr::from_ptr(owner).to_str().unwrap()
        }
    }

    /// Returns the lock comment.
    pub fn comment(&self) -> &str {
        unsafe {
            let comment = (*self.ptr).comment;
            std::ffi::CStr::from_ptr(comment).to_str().unwrap()
        }
    }

    /// Returns true if the comment is a DAV comment.
    pub fn is_dav_comment(&self) -> bool {
        unsafe { (*self.ptr).is_dav_comment == 1 }
    }

    /// Returns the creation date of the lock.
    pub fn creation_date(&self) -> i64 {
        unsafe { (*self.ptr).creation_date }
    }

    /// Returns the expiration date of the lock.
    pub fn expiration_date(&self) -> apr::time::Time {
        apr::time::Time::from(unsafe { (*self.ptr).expiration_date })
    }

    /// Creates a new lock in a new pool.
    pub fn create() -> Result<Lock<'static>, Error> {
        let pool = apr::Pool::new();
        unsafe {
            let lock_ptr = subversion_sys::svn_lock_create(pool.as_mut_ptr());
            let pool_handle = apr::PoolHandle::owned(pool);
            Ok(Lock::from_raw(lock_ptr, pool_handle))
        }
    }
}

/// The kind of checksum algorithm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChecksumKind {
    /// MD5 checksum.
    MD5,
    /// SHA1 checksum.
    SHA1,
    /// FNV-1a 32-bit checksum.
    Fnv1a32,
    /// FNV-1a 32-bit x4 checksum.
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

/// A segment of a location in the repository history.
pub struct LocationSegment<'pool> {
    ptr: *const subversion_sys::svn_location_segment_t,
    _pool: std::marker::PhantomData<&'pool apr::Pool<'static>>,
}
unsafe impl Send for LocationSegment<'_> {}

impl<'pool> LocationSegment<'pool> {
    /// Creates a LocationSegment from a raw pointer.
    pub fn from_raw(ptr: *const subversion_sys::svn_location_segment_t) -> Self {
        Self {
            ptr,
            _pool: std::marker::PhantomData,
        }
    }

    /// Duplicates the location segment in the given pool.
    pub fn dup(&self, pool: &'pool apr::Pool<'pool>) -> Result<LocationSegment<'pool>, Error> {
        unsafe {
            let duplicated = subversion_sys::svn_location_segment_dup(self.ptr, pool.as_mut_ptr());
            Ok(LocationSegment::from_raw(duplicated))
        }
    }

    /// Returns the revision range for this segment.
    pub fn range(&self) -> std::ops::Range<Revnum> {
        unsafe {
            Revnum::from_raw((*self.ptr).range_end).unwrap()
                ..Revnum::from_raw((*self.ptr).range_start).unwrap()
        }
    }

    /// Returns the path for this segment.
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
            Err(err) => err.into_raw(),
        }
    }
}

#[cfg(any(feature = "ra", feature = "client"))]
pub(crate) extern "C" fn wrap_log_entry_receiver(
    baton: *mut std::ffi::c_void,
    log_entry: *mut subversion_sys::svn_log_entry_t,
    _pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    unsafe {
        // Use single dereference pattern like commit callback
        let callback = &mut *(baton as *mut &mut dyn FnMut(&LogEntry) -> Result<(), Error>);
        let log_entry = LogEntry::from_raw(log_entry);
        let ret = callback(&log_entry);
        if let Err(err) = ret {
            err.into_raw()
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
#[cfg(feature = "client")]
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

#[cfg(feature = "client")]
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
#[cfg(feature = "client")]
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

#[cfg(feature = "client")]
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

/// Client conflict option ID that maps directly to svn_client_conflict_option_id_t
#[cfg(feature = "client")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientConflictOptionId {
    /// Undefined or uninitialized option
    Undefined,
    /// Postpone the conflict resolution
    Postpone,
    /// Use the base text (common ancestor)
    BaseText,
    /// Use the incoming text from the merge
    IncomingText,
    /// Use the working text (local changes)
    WorkingText,
    /// Use incoming text only where there are conflicts
    IncomingTextWhereConflicted,
    /// Use working text only where there are conflicts
    WorkingTextWhereConflicted,
    /// Use the merged text
    MergedText,
    /// Unspecified option
    Unspecified,
    /// Accept the current working copy state
    AcceptCurrentWcState,
    /// Update the move destination
    UpdateMoveDestination,
    /// Update any children that were moved away
    UpdateAnyMovedAwayChildren,
    /// Ignore the incoming addition
    IncomingAddIgnore,
    /// Merge the incoming added file text
    IncomingAddedFileTextMerge,
    /// Replace and merge the incoming added file
    IncomingAddedFileReplaceAndMerge,
    /// Merge the incoming added directory
    IncomingAddedDirMerge,
    /// Replace the incoming added directory
    IncomingAddedDirReplace,
    /// Replace and merge the incoming added directory
    IncomingAddedDirReplaceAndMerge,
    /// Ignore the incoming deletion
    IncomingDeleteIgnore,
    /// Accept the incoming deletion
    IncomingDeleteAccept,
}

#[cfg(feature = "client")]
impl From<ClientConflictOptionId> for subversion_sys::svn_client_conflict_option_id_t {
    fn from(choice: ClientConflictOptionId) -> Self {
        match choice {
            ClientConflictOptionId::Undefined => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_undefined,
            ClientConflictOptionId::Postpone => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_postpone,
            ClientConflictOptionId::BaseText => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_base_text,
            ClientConflictOptionId::IncomingText => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_incoming_text,
            ClientConflictOptionId::WorkingText => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_working_text,
            ClientConflictOptionId::IncomingTextWhereConflicted => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_incoming_text_where_conflicted,
            ClientConflictOptionId::WorkingTextWhereConflicted => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_working_text_where_conflicted,
            ClientConflictOptionId::MergedText => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_merged_text,
            ClientConflictOptionId::Unspecified => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_unspecified,
            ClientConflictOptionId::AcceptCurrentWcState => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_accept_current_wc_state,
            ClientConflictOptionId::UpdateMoveDestination => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_update_move_destination,
            ClientConflictOptionId::UpdateAnyMovedAwayChildren => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_update_any_moved_away_children,
            ClientConflictOptionId::IncomingAddIgnore => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_incoming_add_ignore,
            ClientConflictOptionId::IncomingAddedFileTextMerge => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_incoming_added_file_text_merge,
            ClientConflictOptionId::IncomingAddedFileReplaceAndMerge => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_incoming_added_file_replace_and_merge,
            ClientConflictOptionId::IncomingAddedDirMerge => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_incoming_added_dir_merge,
            ClientConflictOptionId::IncomingAddedDirReplace => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_incoming_added_dir_replace,
            ClientConflictOptionId::IncomingAddedDirReplaceAndMerge => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_incoming_added_dir_replace_and_merge,
            ClientConflictOptionId::IncomingDeleteIgnore => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_incoming_delete_ignore,
            ClientConflictOptionId::IncomingDeleteAccept => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_incoming_delete_accept,
        }
    }
}

#[cfg(feature = "client")]
impl From<subversion_sys::svn_client_conflict_option_id_t> for ClientConflictOptionId {
    fn from(id: subversion_sys::svn_client_conflict_option_id_t) -> Self {
        match id {
            subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_undefined => ClientConflictOptionId::Undefined,
            subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_postpone => ClientConflictOptionId::Postpone,
            subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_base_text => ClientConflictOptionId::BaseText,
            subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_incoming_text => ClientConflictOptionId::IncomingText,
            subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_working_text => ClientConflictOptionId::WorkingText,
            subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_incoming_text_where_conflicted => ClientConflictOptionId::IncomingTextWhereConflicted,
            subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_working_text_where_conflicted => ClientConflictOptionId::WorkingTextWhereConflicted,
            subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_merged_text => ClientConflictOptionId::MergedText,
            subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_unspecified => ClientConflictOptionId::Unspecified,
            subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_accept_current_wc_state => ClientConflictOptionId::AcceptCurrentWcState,
            subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_update_move_destination => ClientConflictOptionId::UpdateMoveDestination,
            subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_update_any_moved_away_children => ClientConflictOptionId::UpdateAnyMovedAwayChildren,
            subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_incoming_add_ignore => ClientConflictOptionId::IncomingAddIgnore,
            subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_incoming_added_file_text_merge => ClientConflictOptionId::IncomingAddedFileTextMerge,
            subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_incoming_added_file_replace_and_merge => ClientConflictOptionId::IncomingAddedFileReplaceAndMerge,
            subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_incoming_added_dir_merge => ClientConflictOptionId::IncomingAddedDirMerge,
            subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_incoming_added_dir_replace => ClientConflictOptionId::IncomingAddedDirReplace,
            subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_incoming_added_dir_replace_and_merge => ClientConflictOptionId::IncomingAddedDirReplaceAndMerge,
            subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_incoming_delete_ignore => ClientConflictOptionId::IncomingDeleteIgnore,
            subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_incoming_delete_accept => ClientConflictOptionId::IncomingDeleteAccept,
            _ => ClientConflictOptionId::Undefined,
        }
    }
}

/// Legacy conflict choice enum for backward compatibility with WC functions
#[cfg(feature = "wc")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictChoice {
    /// Postpone resolution for later.
    Postpone,
    /// Use base version (original).
    Base,
    /// Use their version (incoming changes).
    TheirsFull,
    /// Use my version (local changes).
    MineFull,
    /// Use their version for conflicts only.
    TheirsConflict,
    /// Use my version for conflicts only.
    MineConflict,
    /// Use a merged version.
    Merged,
    /// Undefined/unspecified.
    Unspecified,
}

#[cfg(feature = "wc")]
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

/// A checksum value.
pub struct Checksum<'pool> {
    ptr: *const subversion_sys::svn_checksum_t,
    _pool: std::marker::PhantomData<&'pool apr::Pool<'static>>,
}

impl<'pool> Checksum<'pool> {
    /// Creates a Checksum from a raw pointer.
    pub fn from_raw(ptr: *const subversion_sys::svn_checksum_t) -> Self {
        Self {
            ptr,
            _pool: std::marker::PhantomData,
        }
    }

    /// Returns the kind of checksum.
    pub fn kind(&self) -> ChecksumKind {
        ChecksumKind::from(unsafe { (*self.ptr).kind })
    }

    /// Returns the size of the checksum in bytes.
    pub fn size(&self) -> usize {
        unsafe { subversion_sys::svn_checksum_size(self.ptr) }
    }

    /// Returns true if the checksum is empty.
    pub fn is_empty(&self) -> bool {
        unsafe { subversion_sys::svn_checksum_is_empty_checksum(self.ptr as *mut _) == 1 }
    }

    /// Returns the digest bytes.
    pub fn digest(&self) -> &[u8] {
        unsafe {
            let digest = (*self.ptr).digest;
            std::slice::from_raw_parts(digest, self.size())
        }
    }

    /// Parses a checksum from a hexadecimal string.
    pub fn parse_hex(
        kind: ChecksumKind,
        hex: &str,
        pool: &'pool apr::Pool<'pool>,
    ) -> Result<Self, Error> {
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

    /// Creates an empty checksum of the specified kind.
    pub fn empty(kind: ChecksumKind, pool: &'pool apr::Pool<'pool>) -> Result<Self, Error> {
        let kind = kind.into();
        unsafe {
            let checksum = subversion_sys::svn_checksum_empty_checksum(kind, pool.as_mut_ptr());
            Ok(Self::from_raw(checksum))
        }
    }

    /// Create a new checksum from data
    pub fn create(
        kind: ChecksumKind,
        data: &[u8],
        pool: &'pool apr::Pool<'pool>,
    ) -> Result<Self, Error> {
        checksum(kind, data, pool)
    }

    /// Compare two checksums for equality
    pub fn matches(&self, other: &Checksum) -> bool {
        unsafe { subversion_sys::svn_checksum_match(self.ptr, other.ptr) != 0 }
    }

    /// Duplicate this checksum into a new pool
    pub fn dup(&self, pool: &'pool apr::Pool<'pool>) -> Result<Checksum<'pool>, Error> {
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
    pub fn deserialize(data: &str, pool: &'pool apr::Pool<'pool>) -> Result<Self, Error> {
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

/// A context for computing checksums incrementally.
pub struct ChecksumContext<'pool> {
    ptr: *mut subversion_sys::svn_checksum_ctx_t,
    _pool: std::marker::PhantomData<&'pool apr::Pool<'static>>,
}

impl<'pool> ChecksumContext<'pool> {
    /// Creates a new checksum context.
    pub fn new(kind: ChecksumKind, pool: &'pool apr::Pool<'pool>) -> Result<Self, Error> {
        let kind = kind.into();
        unsafe {
            let cc = subversion_sys::svn_checksum_ctx_create(kind, pool.as_mut_ptr());
            Ok(Self {
                ptr: cc,
                _pool: std::marker::PhantomData,
            })
        }
    }

    /// Resets the checksum context.
    pub fn reset(&mut self) -> Result<(), Error> {
        let err = unsafe { subversion_sys::svn_checksum_ctx_reset(self.ptr) };
        Error::from_raw(err)?;
        Ok(())
    }

    /// Updates the checksum with more data.
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

    /// Finishes the checksum computation and returns the result.
    pub fn finish(&self, result_pool: &'pool apr::Pool<'pool>) -> Result<Checksum<'pool>, Error> {
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

/// Computes a checksum for the given data.
pub fn checksum<'pool>(
    kind: ChecksumKind,
    data: &[u8],
    pool: &'pool apr::Pool<'pool>,
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

    /// Get the data from an svn_string_t as a `Vec<u8>`
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
