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
) -> Result<(), Error>
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
) -> Result<(), Error>
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
) -> Result<std::collections::HashMap<String, Vec<crate::RevisionRange>>, Error>
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

        // Convert the apr_hash_t to a Rust HashMap
        // mergeinfo is a hash from path (char*) to rangelist (apr_array_header_t*)
        let mut result = std::collections::HashMap::new();

        // We need to iterate the hash directly using APR's hash iteration
        let iter_pool = apr::pool::Pool::new();
        let hi = unsafe { apr_sys::apr_hash_first(iter_pool.as_mut_ptr(), mergeinfo_ptr) };
        let mut iter = hi;

        while !iter.is_null() {
            let mut key_ptr: *const std::ffi::c_void = ptr::null();
            let mut val_ptr: *mut std::ffi::c_void = ptr::null_mut();

            unsafe {
                apr_sys::apr_hash_this(iter, &mut key_ptr, ptr::null_mut(), &mut val_ptr);

                let source_cstr = std::ffi::CStr::from_ptr(key_ptr as *const std::ffi::c_char);
                let source_str = source_cstr.to_str()?.to_string();

                // Convert svn_rangelist_t (apr_array_header_t of svn_merge_range_t*)
                let rangelist_array = unsafe {
                    apr::tables::TypedArray::<*mut subversion_sys::svn_merge_range_t>::from_ptr(
                        val_ptr as *mut apr_sys::apr_array_header_t,
                    )
                };

                let mut ranges = Vec::new();
                for range_ptr in rangelist_array.iter() {
                    let range = &*range_ptr;
                    ranges.push(crate::RevisionRange::new(
                        crate::Revision::Number(crate::Revnum(range.start)),
                        crate::Revision::Number(crate::Revnum(range.end)),
                    ));
                }

                result.insert(source_str, ranges);

                iter = apr_sys::apr_hash_next(iter);
            }
        }

        Ok(result)
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

// TODO: Implement conflict resolution callbacks when we have better callback infrastructure

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
}
