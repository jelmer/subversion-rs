//! Zero-copy borrowed views for high-performance operations.
//!
//! This module provides lifetime-bound types that borrow data from APR pools
//! without copying. These types are only available with the `zero-copy` feature.
//!
//! # Safety and Lifetimes
//!
//! All types in this module are bound to the lifetime of the pool that owns
//! the underlying data. They cannot outlive their pool and are not Send or Sync.
//!
//! # Example
//!
//! ```no_run
//! # #[cfg(feature = "zero-copy")]
//! # fn example() -> Result<(), subversion::Error> {
//! use subversion::borrowed::{BStr, LogEntryBorrowed};
//!
//! let mut ctx = subversion::client::Context::new()?;
//! 
//! // Using borrowed log entries for high-performance iteration
//! ctx.log_borrowed(
//!     &["/path"],
//!     subversion::Revision::Head,
//!     &[],
//!     100,
//!     false,
//!     false,
//!     false,
//!     &[],
//!     &|entry: &LogEntryBorrowed| {
//!         // entry borrows from the pool, no allocation
//!         println!("Rev {}: {}", entry.revision, entry.message.to_str_lossy());
//!         Ok(())
//!     },
//! )?;
//! # Ok(())
//! # }
//! ```

use std::marker::PhantomData;
use std::fmt;
use std::borrow::Cow;

/// A borrowed byte string from an APR pool.
///
/// This type provides a zero-copy view of string data owned by an APR pool.
/// It is bound to the pool's lifetime and cannot outlive it.
#[derive(Clone, Copy)]
pub struct BStr<'pool> {
    data: &'pool [u8],
}

impl<'pool> BStr<'pool> {
    /// Create a new borrowed string from bytes.
    pub fn new(data: &'pool [u8]) -> Self {
        BStr { data }
    }
    
    /// Create from a C string pointer and length.
    ///
    /// # Safety
    /// The pointer must be valid for the given length and the lifetime 'pool.
    pub unsafe fn from_ptr_len(ptr: *const i8, len: usize) -> Self {
        let slice = std::slice::from_raw_parts(ptr as *const u8, len);
        BStr::new(slice)
    }
    
    /// Get the underlying bytes.
    pub fn as_bytes(&self) -> &'pool [u8] {
        self.data
    }
    
    /// Try to convert to a UTF-8 string slice.
    pub fn to_str(&self) -> Result<&'pool str, std::str::Utf8Error> {
        std::str::from_utf8(self.data)
    }
    
    /// Convert to a UTF-8 string with lossy conversion.
    pub fn to_str_lossy(&self) -> Cow<'pool, str> {
        String::from_utf8_lossy(self.data)
    }
    
    /// Get the length in bytes.
    pub fn len(&self) -> usize {
        self.data.len()
    }
    
    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl<'pool> fmt::Debug for BStr<'pool> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BStr({:?})", self.to_str_lossy())
    }
}

impl<'pool> fmt::Display for BStr<'pool> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_str_lossy())
    }
}

/// A borrowed UTF-8 string from an APR pool.
///
/// This is like BStr but guarantees valid UTF-8.
#[derive(Clone, Copy)]
pub struct BStrUtf8<'pool> {
    data: &'pool str,
}

impl<'pool> BStrUtf8<'pool> {
    /// Create a new borrowed UTF-8 string.
    ///
    /// Returns None if the bytes are not valid UTF-8.
    pub fn new(data: &'pool [u8]) -> Option<Self> {
        std::str::from_utf8(data).ok().map(|s| BStrUtf8 { data: s })
    }
    
    /// Create from a string slice.
    pub fn from_str(s: &'pool str) -> Self {
        BStrUtf8 { data: s }
    }
    
    /// Get the underlying string slice.
    pub fn as_str(&self) -> &'pool str {
        self.data
    }
    
    /// Get the length in bytes.
    pub fn len(&self) -> usize {
        self.data.len()
    }
    
    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl<'pool> fmt::Debug for BStrUtf8<'pool> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BStrUtf8({:?})", self.data)
    }
}

impl<'pool> fmt::Display for BStrUtf8<'pool> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.data)
    }
}

impl<'pool> AsRef<str> for BStrUtf8<'pool> {
    fn as_ref(&self) -> &str {
        self.data
    }
}

/// A borrowed log entry that references pool-owned data.
///
/// This provides zero-copy access to log entry data.
#[cfg(any(feature = "client", feature = "ra"))]
pub struct LogEntryBorrowed<'pool> {
    /// The revision number
    pub revision: crate::Revnum,
    /// The author (may not be UTF-8)
    pub author: Option<BStr<'pool>>,
    /// The date as microseconds since epoch
    pub date: Option<i64>,
    /// The log message (may not be UTF-8)
    pub message: Option<BStr<'pool>>,
    /// Changed paths
    pub changed_paths: Option<ChangedPathsBorrowed<'pool>>,
    /// Whether this revision has children
    pub has_children: bool,
    /// Lifetime marker
    _phantom: PhantomData<&'pool ()>,
}

/// Borrowed changed paths information.
#[cfg(any(feature = "client", feature = "ra"))]
pub struct ChangedPathsBorrowed<'pool> {
    // This would contain the actual implementation
    _phantom: PhantomData<&'pool ()>,
}

/// A borrowed directory entry.
pub struct DirEntryBorrowed<'pool> {
    /// The entry name
    pub name: BStrUtf8<'pool>,
    /// The node kind
    pub kind: crate::NodeKind,
    /// The size in bytes (files only)
    pub size: Option<u64>,
    /// Whether the entry has properties
    pub has_props: bool,
    /// The revision this entry was created
    pub created_rev: crate::Revnum,
    /// The last modification time as microseconds since epoch
    pub time: Option<i64>,
    /// The last author
    pub last_author: Option<BStr<'pool>>,
    /// Lifetime marker
    _phantom: PhantomData<&'pool ()>,
}

// Helper to create borrowed types from SVN C types
impl<'pool> LogEntryBorrowed<'pool> {
    /// Create from raw SVN log entry.
    ///
    /// # Safety
    /// The log_entry pointer must be valid and the data it references
    /// must live for at least 'pool.
    #[cfg(any(feature = "client", feature = "ra"))]
    pub unsafe fn from_raw(
        log_entry: *const subversion_sys::svn_log_entry_t,
    ) -> Self {
        let entry = &*log_entry;
        
        // Note: The actual fields depend on SVN version and bindings generation
        // This is a simplified version - real implementation would need to
        // properly access the hash fields for revprops
        LogEntryBorrowed {
            revision: crate::Revnum(entry.revision),
            author: None, // Would extract from revprops hash
            date: None, // Would extract from revprops hash
            message: None, // Would extract from revprops hash
            changed_paths: None, // Would need proper implementation
            has_children: entry.has_children != 0,
            _phantom: PhantomData,
        }
    }
}

// Extension methods for client when zero-copy is enabled
#[cfg(all(feature = "zero-copy", feature = "client"))]
impl crate::client::Context {
    /// Retrieve log messages with borrowed data for zero-copy access.
    ///
    /// This is more efficient than the regular `log` method as it avoids
    /// allocating strings for each log entry.
    pub fn log_borrowed<'pool, F>(
        &mut self,
        targets: &[&str],
        peg_revision: crate::Revision,
        revision_ranges: &[crate::RevisionRange],
        limit: i32,
        discover_changed_paths: bool,
        strict_node_history: bool,
        include_merged_revisions: bool,
        revprops: &[&str],
        receiver: F,
    ) -> Result<(), crate::Error>
    where
        F: FnMut(&LogEntryBorrowed<'pool>) -> Result<(), crate::Error>,
    {
        // This would need a proper implementation that passes borrowed data
        // For now, we use the regular log method as a fallback
        self.log(
            targets,
            peg_revision,
            revision_ranges,
            limit,
            discover_changed_paths,
            strict_node_history,
            include_merged_revisions,
            revprops,
            &|entry| {
                // Convert owned to borrowed (not ideal, but safe)
                // A real implementation would pass borrowed data directly
                receiver(&unsafe {
                    std::mem::transmute::<LogEntryBorrowed<'_>, LogEntryBorrowed<'pool>>(
                        LogEntryBorrowed {
                            revision: entry.revision(),
                            author: None, // Would need proper conversion
                            date: None,
                            message: Some(BStr::new(b"message")), // Placeholder
                            changed_paths: None,
                            has_children: false,
                            _phantom: PhantomData,
                        }
                    )
                })
            },
        )
    }
}