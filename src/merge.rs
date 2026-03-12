use crate::dirent::AsCanonicalDirent;
use crate::uri::AsCanonicalUri;
use crate::{with_tmp_pool, Depth, Error, Revision};
use std::ffi::CString;
use std::ptr;

/// Options for merge operations
#[derive(Debug, Clone, Default)]
pub struct MergeOptions {
    /// Ignore mergeinfo during the merge
    pub ignore_mergeinfo: bool,
    /// Ignore ancestry when computing differences
    pub diff_ignore_ancestry: bool,
    /// Force deletion of locally modified or unversioned files
    pub force_delete: bool,
    /// Only record the merge, don't apply changes
    pub record_only: bool,
    /// Perform a dry run (no actual changes)
    pub dry_run: bool,
    /// Allow merge into a mixed-revision working copy
    pub allow_mixed_rev: bool,
    /// Additional diff options (e.g., "-b", "-w", "--ignore-eol-style")
    pub diff_options: Vec<String>,
}

/// Perform a three-way merge between two sources
///
/// Merge the changes between `source1` at `revision1` and `source2` at `revision2`
/// into the working copy at `target_wcpath`.
///
/// # Arguments
/// * `source1` - The first source URL or path
/// * `revision1` - The revision of the first source
/// * `source2` - The second source URL or path
/// * `revision2` - The revision of the second source
/// * `target_wcpath` - The target working copy path
/// * `depth` - How deep to recurse
/// * `options` - Merge options
/// * `ctx` - The client context
pub fn merge<S1, S2, T>(
    source1: S1,
    revision1: impl Into<Revision>,
    source2: S2,
    revision2: impl Into<Revision>,
    target_wcpath: T,
    depth: Depth,
    options: &MergeOptions,
    ctx: &mut crate::client::Context,
) -> Result<(), Error<'static>>
where
    S1: AsCanonicalUri,
    S2: AsCanonicalUri,
    T: AsCanonicalDirent,
{
    with_tmp_pool(|pool| unsafe {
        let source1_uri = source1.as_canonical_uri()?;
        let source2_uri = source2.as_canonical_uri()?;
        let target = target_wcpath.as_canonical_dirent()?;

        let source1_cstr = CString::new(source1_uri.as_str())?;
        let source2_cstr = CString::new(source2_uri.as_str())?;
        let target_cstr = CString::new(target.as_str())?;

        let revision1: subversion_sys::svn_opt_revision_t = revision1.into().into();
        let revision2: subversion_sys::svn_opt_revision_t = revision2.into().into();

        // Convert diff options to APR array
        let merge_options = if options.diff_options.is_empty() {
            ptr::null()
        } else {
            let mut arr = apr::tables::TypedArray::<*const std::ffi::c_char>::new(
                pool,
                options.diff_options.len() as i32,
            );
            for opt in &options.diff_options {
                arr.push(apr::strings::pstrdup_raw(opt, pool)? as *const _);
            }
            arr.as_ptr()
        };

        let err = subversion_sys::svn_client_merge5(
            source1_cstr.as_ptr(),
            &revision1,
            source2_cstr.as_ptr(),
            &revision2,
            target_cstr.as_ptr(),
            depth.into(),
            options.ignore_mergeinfo as i32,
            options.diff_ignore_ancestry as i32,
            options.force_delete as i32,
            options.record_only as i32,
            options.dry_run as i32,
            options.allow_mixed_rev as i32,
            merge_options,
            ctx.as_mut_ptr(),
            pool.as_mut_ptr(),
        );

        Error::from_raw(err)
    })
}

/// Perform a peg revision merge
///
/// Merge changes from a source branch identified by `source_path_or_url` at
/// `source_peg_revision` into the target working copy at `target_wcpath`.
///
/// # Arguments
/// * `source` - The source URL or path
/// * `ranges_to_merge` - Optional array of revision ranges to merge (automatic if None)
/// * `source_peg_revision` - The peg revision for the source
/// * `target_wcpath` - The target working copy path  
/// * `depth` - How deep to recurse
/// * `options` - Merge options
/// * `ctx` - The client context
pub fn merge_peg<S, T>(
    source: S,
    ranges_to_merge: Option<&[crate::RevisionRange]>,
    source_peg_revision: impl Into<Revision>,
    target_wcpath: T,
    depth: Depth,
    options: &MergeOptions,
    ctx: &mut crate::client::Context,
) -> Result<(), Error<'static>>
where
    S: AsCanonicalUri,
    T: AsCanonicalDirent,
{
    with_tmp_pool(|pool| unsafe {
        let source_uri = source.as_canonical_uri()?;
        let target = target_wcpath.as_canonical_dirent()?;

        let source_cstr = CString::new(source_uri.as_str())?;
        let target_cstr = CString::new(target.as_str())?;

        let peg_revision: subversion_sys::svn_opt_revision_t = source_peg_revision.into().into();

        // Convert revision ranges to APR array if provided
        let ranges_array =
            if let Some(ranges) = ranges_to_merge {
                let mut arr = apr::tables::TypedArray::<
                    *mut subversion_sys::svn_opt_revision_range_t,
                >::new(pool, ranges.len() as i32);
                for range in ranges {
                    let range_ptr: *mut subversion_sys::svn_opt_revision_range_t = pool.calloc();
                    (*range_ptr).start = range.start.into();
                    (*range_ptr).end = range.end.into();
                    arr.push(range_ptr);
                }
                arr.as_ptr()
            } else {
                ptr::null()
            };

        // Convert diff options to APR array
        let merge_options = if options.diff_options.is_empty() {
            ptr::null()
        } else {
            let mut arr = apr::tables::TypedArray::<*const std::ffi::c_char>::new(
                pool,
                options.diff_options.len() as i32,
            );
            for opt in &options.diff_options {
                arr.push(apr::strings::pstrdup_raw(opt, pool)? as *const _);
            }
            arr.as_ptr()
        };

        let err = subversion_sys::svn_client_merge_peg5(
            source_cstr.as_ptr(),
            ranges_array,
            &peg_revision,
            target_cstr.as_ptr(),
            depth.into(),
            options.ignore_mergeinfo as i32,
            options.diff_ignore_ancestry as i32,
            options.force_delete as i32,
            options.record_only as i32,
            options.dry_run as i32,
            options.allow_mixed_rev as i32,
            merge_options,
            ctx.as_mut_ptr(),
            pool.as_mut_ptr(),
        );

        Error::from_raw(err)
    })
}

/// Get merge information about what has been merged into a path
///
/// Returns a hash of merge source URLs to revision range lists
pub fn get_merged_mergeinfo<P>(
    path_or_url: P,
    peg_revision: impl Into<Revision>,
    ctx: &mut crate::client::Context,
) -> Result<std::collections::HashMap<String, Vec<crate::RevisionRange>>, Error<'_>>
where
    P: AsCanonicalUri,
{
    with_tmp_pool(|pool| unsafe {
        let path = path_or_url.as_canonical_uri()?;
        let path_cstr = CString::new(path.as_str())?;
        let peg_rev: subversion_sys::svn_opt_revision_t = peg_revision.into().into();

        let mut mergeinfo_ptr = ptr::null_mut();

        let err = subversion_sys::svn_client_mergeinfo_get_merged(
            &mut mergeinfo_ptr,
            path_cstr.as_ptr(),
            &peg_rev,
            ctx.as_mut_ptr(),
            pool.as_mut_ptr(),
        );

        Error::from_raw(err)?;

        if mergeinfo_ptr.is_null() {
            return Ok(std::collections::HashMap::new());
        }

        // Use MergeinfoHash to safely convert the hash
        let mergeinfo_hash = MergeinfoHash::from_ptr(mergeinfo_ptr);
        Ok(mergeinfo_hash.to_hashmap())
    })
}

/// A conflict that occurred during a merge operation
#[derive(Debug, Clone)]
pub struct MergeConflict {
    /// The path that has a conflict
    pub path: String,
    /// The type of conflict
    pub conflict_type: ConflictType,
    /// Base revision of the conflict
    pub base_revision: Option<crate::Revnum>,
    /// Their revision of the conflict
    pub their_revision: Option<crate::Revnum>,
    /// My revision of the conflict
    pub my_revision: Option<crate::Revnum>,
}

/// Type of conflict that can occur during merge
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictType {
    /// Text content conflict
    Text,
    /// Property conflict
    Property,
    /// Tree structure conflict (add/delete/move)
    Tree,
}

/// Result of a conflict resolution
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictResolution {
    /// Use the local version (mine)
    Mine,
    /// Use the incoming version (theirs)
    Theirs,
    /// Use the base version
    Base,
    /// Mark as resolved, custom resolution was applied
    Working,
    /// Postpone resolution
    Postpone,
}

/// A safe wrapper for APR hashes containing mergeinfo (path -> rangelist mappings)
///
/// This wrapper encapsulates the common pattern of working with mergeinfo hashes
/// from Subversion's C API, reducing unsafe code and providing convenient
/// conversion methods.
pub struct MergeinfoHash<'a> {
    inner: apr::hash::TypedHash<'a, apr_sys::apr_array_header_t>,
}

impl<'a> MergeinfoHash<'a> {
    /// Create a MergeinfoHash from a raw APR hash pointer
    ///
    /// # Safety
    /// The caller must ensure that:
    /// - `ptr` is a valid APR hash containing rangelist arrays
    /// - The hash and its contents remain valid for the lifetime of this wrapper
    pub unsafe fn from_ptr(ptr: *mut apr_sys::apr_hash_t) -> Self {
        Self {
            inner: apr::hash::TypedHash::<apr_sys::apr_array_header_t>::from_ptr(ptr),
        }
    }

    /// Convert the mergeinfo to a `HashMap<String, Vec<RevisionRange>>`
    pub fn to_hashmap(&self) -> std::collections::HashMap<String, Vec<crate::RevisionRange>> {
        self.inner
            .iter()
            .map(|(k, v)| {
                let key = String::from_utf8_lossy(k).into_owned();
                let ranges = unsafe { self.rangelist_to_vec(v as *const _ as *mut _) };
                (key, ranges)
            })
            .collect()
    }

    /// Convert a rangelist array to a Vec<RevisionRange>
    unsafe fn rangelist_to_vec(
        &self,
        rangelist: *mut apr_sys::apr_array_header_t,
    ) -> Vec<crate::RevisionRange> {
        if rangelist.is_null() {
            return Vec::new();
        }

        let rangelist_array =
            apr::tables::TypedArray::<*mut subversion_sys::svn_merge_range_t>::from_ptr(rangelist);

        let mut ranges = Vec::new();
        for range_ptr in rangelist_array.iter() {
            let range = &*range_ptr;
            ranges.push(crate::RevisionRange::new(
                crate::Revision::Number(crate::Revnum(range.start)),
                crate::Revision::Number(crate::Revnum(range.end)),
            ));
        }
        ranges
    }
}

/// Parse mergeinfo from a string representation
///
/// Converts a string in the format "/trunk:1-10,15,20-25" into a mergeinfo hash
pub fn parse_mergeinfo(
    mergeinfo_str: &str,
) -> Result<std::collections::HashMap<String, Vec<crate::RevisionRange>>, Error<'_>> {
    with_tmp_pool(|pool| unsafe {
        let mergeinfo_cstr = CString::new(mergeinfo_str)?;
        let mut mergeinfo_ptr = ptr::null_mut();

        let err = subversion_sys::svn_mergeinfo_parse(
            &mut mergeinfo_ptr,
            mergeinfo_cstr.as_ptr(),
            pool.as_mut_ptr(),
        );

        Error::from_raw(err)?;

        if mergeinfo_ptr.is_null() {
            return Ok(std::collections::HashMap::new());
        }

        let mergeinfo_hash = MergeinfoHash::from_ptr(mergeinfo_ptr);
        Ok(mergeinfo_hash.to_hashmap())
    })
}

/// Convert mergeinfo to a string representation
pub fn mergeinfo_to_string(
    mergeinfo: &std::collections::HashMap<String, Vec<crate::RevisionRange>>,
) -> Result<String, Error<'static>> {
    with_tmp_pool(|pool| unsafe {
        // Create an APR hash from the Rust HashMap
        let hash = apr_sys::apr_hash_make(pool.as_mut_ptr());

        for (path, ranges) in mergeinfo {
            let path_cstr = apr::strings::pstrdup_raw(path, pool)?;

            // Create rangelist array
            let mut rangelist =
                apr::tables::TypedArray::<*mut subversion_sys::svn_merge_range_t>::new(
                    pool,
                    ranges.len() as i32,
                );

            for range in ranges {
                let range_ptr: *mut subversion_sys::svn_merge_range_t = pool.calloc();
                if let (crate::Revision::Number(start), crate::Revision::Number(end)) =
                    (&range.start, &range.end)
                {
                    (*range_ptr).start = start.0;
                    (*range_ptr).end = end.0;
                    (*range_ptr).inheritable = 1;
                    rangelist.push(range_ptr);
                }
            }

            apr_sys::apr_hash_set(
                hash,
                path_cstr as *const std::ffi::c_void,
                apr_sys::APR_HASH_KEY_STRING as isize,
                rangelist.as_ptr() as *mut std::ffi::c_void,
            );
        }

        let mut output_ptr: *mut subversion_sys::svn_string_t = ptr::null_mut();

        let err = subversion_sys::svn_mergeinfo_to_string(&mut output_ptr, hash, pool.as_mut_ptr());

        Error::from_raw(err)?;

        if output_ptr.is_null() {
            Ok(String::new())
        } else {
            let svn_string = &*output_ptr;
            let slice = std::slice::from_raw_parts(svn_string.data as *const u8, svn_string.len);
            Ok(String::from_utf8_lossy(slice).into_owned())
        }
    })
}

/// Calculate the difference between two mergeinfo sets
pub fn mergeinfo_diff(
    mergeinfo1: &std::collections::HashMap<String, Vec<crate::RevisionRange>>,
    mergeinfo2: &std::collections::HashMap<String, Vec<crate::RevisionRange>>,
    consider_inheritance: bool,
) -> Result<
    (
        std::collections::HashMap<String, Vec<crate::RevisionRange>>,
        std::collections::HashMap<String, Vec<crate::RevisionRange>>,
    ),
    Error<'static>,
> {
    with_tmp_pool(|pool| unsafe {
        // Convert Rust HashMaps to APR hashes
        let hash1 = hashmap_to_mergeinfo_hash(mergeinfo1, pool)?;
        let hash2 = hashmap_to_mergeinfo_hash(mergeinfo2, pool)?;

        let mut deleted_ptr = ptr::null_mut();
        let mut added_ptr = ptr::null_mut();

        let err = subversion_sys::svn_mergeinfo_diff2(
            &mut deleted_ptr,
            &mut added_ptr,
            hash1,
            hash2,
            consider_inheritance as i32,
            pool.as_mut_ptr(),
            pool.as_mut_ptr(),
        );

        Error::from_raw(err)?;

        let deleted = MergeinfoHash::from_ptr(deleted_ptr).to_hashmap();
        let added = MergeinfoHash::from_ptr(added_ptr).to_hashmap();

        Ok((deleted, added))
    })
}

/// Merge two mergeinfo sets together
pub fn mergeinfo_merge(
    mergeinfo1: &std::collections::HashMap<String, Vec<crate::RevisionRange>>,
    mergeinfo2: &std::collections::HashMap<String, Vec<crate::RevisionRange>>,
) -> Result<std::collections::HashMap<String, Vec<crate::RevisionRange>>, Error<'static>> {
    with_tmp_pool(|pool| unsafe {
        let hash1 = hashmap_to_mergeinfo_hash(mergeinfo1, pool)?;
        let hash2 = hashmap_to_mergeinfo_hash(mergeinfo2, pool)?;

        let err = subversion_sys::svn_mergeinfo_merge2(
            hash1,
            hash2,
            pool.as_mut_ptr(),
            pool.as_mut_ptr(),
        );

        Error::from_raw(err)?;

        Ok(MergeinfoHash::from_ptr(hash1).to_hashmap())
    })
}

/// Remove a revision range from mergeinfo
pub fn mergeinfo_remove(
    mergeinfo: &mut std::collections::HashMap<String, Vec<crate::RevisionRange>>,
    eraser: &std::collections::HashMap<String, Vec<crate::RevisionRange>>,
    consider_inheritance: bool,
) -> Result<(), Error<'static>> {
    with_tmp_pool(|pool| unsafe {
        let hash = hashmap_to_mergeinfo_hash(mergeinfo, pool)?;
        let eraser_hash = hashmap_to_mergeinfo_hash(eraser, pool)?;

        let err = subversion_sys::svn_mergeinfo_remove2(
            &mut (hash as *mut _),
            eraser_hash,
            hash,
            consider_inheritance as i32,
            pool.as_mut_ptr(),
            pool.as_mut_ptr(),
        );

        Error::from_raw(err)?;

        *mergeinfo = MergeinfoHash::from_ptr(hash).to_hashmap();
        Ok(())
    })
}

/// Intersect two mergeinfo sets
pub fn mergeinfo_intersect(
    mergeinfo1: &std::collections::HashMap<String, Vec<crate::RevisionRange>>,
    mergeinfo2: &std::collections::HashMap<String, Vec<crate::RevisionRange>>,
    consider_inheritance: bool,
) -> Result<std::collections::HashMap<String, Vec<crate::RevisionRange>>, Error<'static>> {
    with_tmp_pool(|pool| unsafe {
        let hash1 = hashmap_to_mergeinfo_hash(mergeinfo1, pool)?;
        let hash2 = hashmap_to_mergeinfo_hash(mergeinfo2, pool)?;

        let mut result_ptr = ptr::null_mut();

        let err = subversion_sys::svn_mergeinfo_intersect2(
            &mut result_ptr,
            hash1,
            hash2,
            consider_inheritance as i32,
            pool.as_mut_ptr(),
            pool.as_mut_ptr(),
        );

        Error::from_raw(err)?;

        Ok(MergeinfoHash::from_ptr(result_ptr).to_hashmap())
    })
}

// Helper function to convert Rust HashMap to APR hash
unsafe fn hashmap_to_mergeinfo_hash<'p>(
    mergeinfo: &std::collections::HashMap<String, Vec<crate::RevisionRange>>,
    pool: &'p apr::Pool<'p>,
) -> Result<*mut apr_sys::apr_hash_t, Error<'static>> {
    let hash = apr_sys::apr_hash_make(pool.as_mut_ptr());

    for (path, ranges) in mergeinfo {
        let path_cstr = apr::strings::pstrdup_raw(path, pool)?;

        let mut rangelist = apr::tables::TypedArray::<*mut subversion_sys::svn_merge_range_t>::new(
            pool,
            ranges.len() as i32,
        );

        for range in ranges {
            let range_ptr: *mut subversion_sys::svn_merge_range_t = pool.calloc();
            if let (crate::Revision::Number(start), crate::Revision::Number(end)) =
                (&range.start, &range.end)
            {
                (*range_ptr).start = start.0;
                (*range_ptr).end = end.0;
                (*range_ptr).inheritable = 1;
                rangelist.push(range_ptr);
            }
        }

        apr_sys::apr_hash_set(
            hash,
            path_cstr as *const std::ffi::c_void,
            apr_sys::APR_HASH_KEY_STRING as isize,
            rangelist.as_ptr() as *mut std::ffi::c_void,
        );
    }

    Ok(hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_options_default() {
        let opts = MergeOptions::default();
        assert!(!opts.ignore_mergeinfo);
        assert!(!opts.diff_ignore_ancestry);
        assert!(!opts.force_delete);
        assert!(!opts.record_only);
        assert!(!opts.dry_run);
        assert!(!opts.allow_mixed_rev);
        assert!(opts.diff_options.is_empty());
    }

    #[test]
    fn test_merge_options_builder() {
        let opts = MergeOptions {
            dry_run: true,
            record_only: true,
            diff_options: vec!["-b".to_string(), "-w".to_string()],
            ..Default::default()
        };
        assert!(opts.dry_run);
        assert!(opts.record_only);
        assert_eq!(opts.diff_options.len(), 2);
    }

    #[test]
    fn test_parse_mergeinfo() {
        let mergeinfo = parse_mergeinfo("/trunk:1-10").unwrap();
        assert_eq!(mergeinfo.len(), 1);
        assert!(mergeinfo.contains_key("/trunk"));
        let ranges = &mergeinfo["/trunk"];
        assert_eq!(ranges.len(), 1);
    }

    #[test]
    fn test_mergeinfo_roundtrip() {
        let mergeinfo = parse_mergeinfo("/trunk:1-10").unwrap();
        let s = mergeinfo_to_string(&mergeinfo).unwrap();
        assert_eq!(s, "/trunk:1-10");
    }

    #[test]
    fn test_mergeinfo_diff() {
        let mi1 = parse_mergeinfo("/trunk:1-10").unwrap();
        let mi2 = parse_mergeinfo("/trunk:5-15").unwrap();

        let (deleted, added) = mergeinfo_diff(&mi1, &mi2, false).unwrap();
        // deleted = in mi1 but not mi2, added = in mi2 but not mi1
        assert!(deleted.contains_key("/trunk"));
        assert!(added.contains_key("/trunk"));
    }

    #[test]
    fn test_mergeinfo_merge() {
        let mi1 = parse_mergeinfo("/trunk:1-10").unwrap();
        let mi2 = parse_mergeinfo("/trunk:11-20").unwrap();

        let merged = mergeinfo_merge(&mi1, &mi2).unwrap();
        assert!(merged.contains_key("/trunk"));
        let s = mergeinfo_to_string(&merged).unwrap();
        assert_eq!(s, "/trunk:1-20");
    }

    #[test]
    fn test_mergeinfo_intersect() {
        let mi1 = parse_mergeinfo("/trunk:1-10").unwrap();
        let mi2 = parse_mergeinfo("/trunk:5-15").unwrap();

        let intersection = mergeinfo_intersect(&mi1, &mi2, false).unwrap();
        assert!(intersection.contains_key("/trunk"));
        let s = mergeinfo_to_string(&intersection).unwrap();
        assert_eq!(s, "/trunk:5-10");
    }
}
