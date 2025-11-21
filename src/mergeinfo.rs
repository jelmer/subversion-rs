use crate::{svn_result, Error, Revision, Revnum};
use std::collections::HashMap;
use std::marker::PhantomData;
use subversion_sys::svn_mergeinfo_t;

/// Mergeinfo handle with RAII cleanup
pub struct Mergeinfo {
    ptr: svn_mergeinfo_t,
    pool: apr::Pool<'static>,
    _phantom: PhantomData<*mut ()>, // !Send + !Sync
}

impl Mergeinfo {
    pub(crate) unsafe fn from_ptr_and_pool(ptr: svn_mergeinfo_t, pool: apr::Pool<'static>) -> Self {
        Self {
            ptr,
            pool,
            _phantom: PhantomData,
        }
    }

    /// Parse a mergeinfo string into a Mergeinfo struct.
    ///
    /// The input string should be in the standard mergeinfo format:
    /// `/path:rev1,rev2-rev3\n/other/path:rev4`
    pub fn parse(input: &str) -> Result<Self, Error> {
        let pool = apr::Pool::new();
        let input_cstr = std::ffi::CString::new(input)
            .map_err(|_| Error::from_str("Invalid mergeinfo string"))?;

        unsafe {
            let mut mergeinfo: svn_mergeinfo_t = std::ptr::null_mut();
            let err = subversion_sys::svn_mergeinfo_parse(
                &mut mergeinfo,
                input_cstr.as_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;
            Ok(Self::from_ptr_and_pool(mergeinfo, pool))
        }
    }

    /// Convert this mergeinfo to a string representation.
    pub fn to_string(&self) -> Result<String, Error> {
        unsafe {
            let mut output: *mut subversion_sys::svn_string_t = std::ptr::null_mut();
            let err = subversion_sys::svn_mergeinfo_to_string(
                &mut output,
                self.ptr,
                self.pool.as_mut_ptr(),
            );
            svn_result(err)?;

            if output.is_null() || (*output).data.is_null() {
                return Ok(String::new());
            }

            let data_slice = std::slice::from_raw_parts((*output).data as *const u8, (*output).len);
            Ok(std::str::from_utf8(data_slice)
                .map_err(|_| Error::from_str("Mergeinfo string is not valid UTF-8"))?
                .to_string())
        }
    }

    /// Merge another mergeinfo into this one.
    pub fn merge(&mut self, other: &Mergeinfo) -> Result<(), Error> {
        unsafe {
            let err = subversion_sys::svn_mergeinfo_merge2(
                self.ptr,
                other.ptr,
                self.pool.as_mut_ptr(),
                self.pool.as_mut_ptr(),
            );
            svn_result(err)
        }
    }

    /// Remove revisions in `eraser` from this mergeinfo.
    pub fn remove(&self, eraser: &Mergeinfo) -> Result<Mergeinfo, Error> {
        let pool = apr::Pool::new();
        unsafe {
            let mut result: svn_mergeinfo_t = std::ptr::null_mut();
            let err = subversion_sys::svn_mergeinfo_remove2(
                &mut result,
                eraser.ptr,
                self.ptr,
                1, // consider_inheritance
                pool.as_mut_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;
            Ok(Mergeinfo::from_ptr_and_pool(result, pool))
        }
    }

    /// Compute the intersection of this mergeinfo with another.
    pub fn intersect(&self, other: &Mergeinfo) -> Result<Mergeinfo, Error> {
        let pool = apr::Pool::new();
        unsafe {
            let mut result: svn_mergeinfo_t = std::ptr::null_mut();
            let err = subversion_sys::svn_mergeinfo_intersect2(
                &mut result,
                self.ptr,
                other.ptr,
                1, // consider_inheritance
                pool.as_mut_ptr(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;
            Ok(Mergeinfo::from_ptr_and_pool(result, pool))
        }
    }

    /// Compute the difference between this mergeinfo and another.
    ///
    /// Returns (deleted, added) where:
    /// - deleted: mergeinfo in `self` but not in `other`
    /// - added: mergeinfo in `other` but not in `self`
    pub fn diff(&self, other: &Mergeinfo) -> Result<(Mergeinfo, Mergeinfo), Error> {
        let pool1 = apr::Pool::new();
        let pool2 = apr::Pool::new();
        unsafe {
            let mut deleted: svn_mergeinfo_t = std::ptr::null_mut();
            let mut added: svn_mergeinfo_t = std::ptr::null_mut();
            let err = subversion_sys::svn_mergeinfo_diff2(
                &mut deleted,
                &mut added,
                self.ptr,
                other.ptr,
                1, // consider_inheritance
                pool1.as_mut_ptr(),
                pool1.as_mut_ptr(),
            );
            svn_result(err)?;
            Ok((
                Mergeinfo::from_ptr_and_pool(deleted, pool1),
                Mergeinfo::from_ptr_and_pool(added, pool2),
            ))
        }
    }

    /// Duplicate this mergeinfo.
    pub fn dup(&self) -> Mergeinfo {
        let pool = apr::Pool::new();
        unsafe {
            let result = subversion_sys::svn_mergeinfo_dup(self.ptr, pool.as_mut_ptr());
            Mergeinfo::from_ptr_and_pool(result, pool)
        }
    }

    /// Sort the mergeinfo.
    pub fn sort(&mut self) -> Result<(), Error> {
        unsafe {
            let err = subversion_sys::svn_mergeinfo_sort(self.ptr, self.pool.as_mut_ptr());
            svn_result(err)
        }
    }

    /// Get the paths in this mergeinfo as a HashMap of path -> revision ranges.
    pub fn paths(&self) -> HashMap<String, Vec<crate::RevisionRange>> {
        let mut result = HashMap::new();

        if self.ptr.is_null() {
            return result;
        }

        unsafe {
            let hash =
                apr::hash::TypedHash::<subversion_sys::svn_rangelist_t>::from_ptr(self.ptr as _);

            for (key, rangelist) in hash.iter() {
                let path = std::str::from_utf8(key)
                    .expect("mergeinfo path is not valid UTF-8")
                    .to_string();

                let mut ranges = Vec::new();
                let array = apr::tables::TypedArray::<subversion_sys::svn_merge_range_t>::from_ptr(
                    rangelist as *const _ as *mut _,
                );

                for range in array.iter() {
                    ranges.push(crate::RevisionRange {
                        start: Revision::Number(Revnum(range.start)),
                        end: Revision::Number(Revnum(range.end)),
                    });
                }

                result.insert(path, ranges);
            }
        }

        result
    }
}

impl Drop for Mergeinfo {
    fn drop(&mut self) {
        // Pool drop will clean up mergeinfo
    }
}

/// Specifies how merge information is inherited.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum MergeinfoInheritance {
    /// Merge information is explicit.
    Explicit,
    /// Merge information is inherited.
    Inherited,
    /// Merge information comes from the nearest ancestor.
    NearestAncestor,
}

impl From<subversion_sys::svn_mergeinfo_inheritance_t> for MergeinfoInheritance {
    fn from(value: subversion_sys::svn_mergeinfo_inheritance_t) -> Self {
        match value {
            subversion_sys::svn_mergeinfo_inheritance_t_svn_mergeinfo_nearest_ancestor => {
                MergeinfoInheritance::NearestAncestor
            }
            subversion_sys::svn_mergeinfo_inheritance_t_svn_mergeinfo_explicit => {
                MergeinfoInheritance::Explicit
            }
            subversion_sys::svn_mergeinfo_inheritance_t_svn_mergeinfo_inherited => {
                MergeinfoInheritance::Inherited
            }
            _ => unreachable!(),
        }
    }
}

impl From<MergeinfoInheritance> for subversion_sys::svn_mergeinfo_inheritance_t {
    fn from(value: MergeinfoInheritance) -> Self {
        match value {
            MergeinfoInheritance::NearestAncestor => {
                subversion_sys::svn_mergeinfo_inheritance_t_svn_mergeinfo_nearest_ancestor
            }
            MergeinfoInheritance::Explicit => {
                subversion_sys::svn_mergeinfo_inheritance_t_svn_mergeinfo_explicit
            }
            MergeinfoInheritance::Inherited => {
                subversion_sys::svn_mergeinfo_inheritance_t_svn_mergeinfo_inherited
            }
        }
    }
}

/// A merge range with start/end revisions and inheritability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MergeRange {
    /// Start revision (exclusive)
    pub start: Revnum,
    /// End revision (inclusive)
    pub end: Revnum,
    /// Whether this range is inheritable
    pub inheritable: bool,
}

impl MergeRange {
    /// Create a new merge range.
    pub fn new(start: Revnum, end: Revnum, inheritable: bool) -> Self {
        Self {
            start,
            end,
            inheritable,
        }
    }
}

impl From<&subversion_sys::svn_merge_range_t> for MergeRange {
    fn from(r: &subversion_sys::svn_merge_range_t) -> Self {
        Self {
            start: Revnum(r.start),
            end: Revnum(r.end),
            inheritable: r.inheritable != 0,
        }
    }
}

impl From<subversion_sys::svn_merge_range_t> for MergeRange {
    fn from(r: subversion_sys::svn_merge_range_t) -> Self {
        Self {
            start: Revnum(r.start),
            end: Revnum(r.end),
            inheritable: r.inheritable != 0,
        }
    }
}

/// A list of merge ranges.
pub struct Rangelist {
    ptr: *mut subversion_sys::svn_rangelist_t,
    pool: apr::Pool<'static>,
}

impl Rangelist {
    /// Create a new empty rangelist.
    pub fn new() -> Self {
        let pool = apr::Pool::new();
        let ptr = unsafe {
            apr_sys::apr_array_make(
                pool.as_mut_ptr(),
                0,
                std::mem::size_of::<subversion_sys::svn_merge_range_t>() as i32,
            )
        };
        Self { ptr, pool }
    }

    /// Create a rangelist from a raw pointer and pool.
    ///
    /// # Safety
    /// The pointer must be valid and the pool must own the memory.
    pub(crate) unsafe fn from_ptr_and_pool(
        ptr: *mut subversion_sys::svn_rangelist_t,
        pool: apr::Pool<'static>,
    ) -> Self {
        Self { ptr, pool }
    }

    /// Convert the rangelist to a string representation.
    pub fn to_string(&self) -> Result<String, Error> {
        unsafe {
            let mut output: *mut subversion_sys::svn_string_t = std::ptr::null_mut();
            let err = subversion_sys::svn_rangelist_to_string(
                &mut output,
                self.ptr,
                self.pool.as_mut_ptr(),
            );
            svn_result(err)?;

            if output.is_null() || (*output).data.is_null() {
                return Ok(String::new());
            }

            let data_slice = std::slice::from_raw_parts((*output).data as *const u8, (*output).len);
            Ok(std::str::from_utf8(data_slice)
                .map_err(|_| Error::from_str("Rangelist string is not valid UTF-8"))?
                .to_string())
        }
    }

    /// Compute the difference between two rangelists.
    ///
    /// Returns (deleted, added) where:
    /// - deleted: ranges in `from` but not in `to`
    /// - added: ranges in `to` but not in `from`
    pub fn diff(
        from: &Rangelist,
        to: &Rangelist,
        consider_inheritance: bool,
    ) -> Result<(Rangelist, Rangelist), Error> {
        let pool1 = apr::Pool::new();
        let pool2 = apr::Pool::new();
        unsafe {
            let mut deleted: *mut subversion_sys::svn_rangelist_t = std::ptr::null_mut();
            let mut added: *mut subversion_sys::svn_rangelist_t = std::ptr::null_mut();
            let err = subversion_sys::svn_rangelist_diff(
                &mut deleted,
                &mut added,
                from.ptr,
                to.ptr,
                consider_inheritance.into(),
                pool1.as_mut_ptr(),
            );
            svn_result(err)?;
            Ok((
                Rangelist::from_ptr_and_pool(deleted, pool1),
                Rangelist::from_ptr_and_pool(added, pool2),
            ))
        }
    }

    /// Merge changes into this rangelist.
    pub fn merge(&mut self, changes: &Rangelist) -> Result<(), Error> {
        unsafe {
            let err = subversion_sys::svn_rangelist_merge2(
                self.ptr,
                changes.ptr,
                self.pool.as_mut_ptr(),
                self.pool.as_mut_ptr(),
            );
            svn_result(err)
        }
    }

    /// Remove ranges in `eraser` from `whiteboard`.
    pub fn remove(
        eraser: &Rangelist,
        whiteboard: &Rangelist,
        consider_inheritance: bool,
    ) -> Result<Rangelist, Error> {
        let pool = apr::Pool::new();
        unsafe {
            let mut output: *mut subversion_sys::svn_rangelist_t = std::ptr::null_mut();
            let err = subversion_sys::svn_rangelist_remove(
                &mut output,
                eraser.ptr,
                whiteboard.ptr,
                consider_inheritance.into(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;
            Ok(Rangelist::from_ptr_and_pool(output, pool))
        }
    }

    /// Compute the intersection of two rangelists.
    pub fn intersect(
        rangelist1: &Rangelist,
        rangelist2: &Rangelist,
        consider_inheritance: bool,
    ) -> Result<Rangelist, Error> {
        let pool = apr::Pool::new();
        unsafe {
            let mut output: *mut subversion_sys::svn_rangelist_t = std::ptr::null_mut();
            let err = subversion_sys::svn_rangelist_intersect(
                &mut output,
                rangelist1.ptr,
                rangelist2.ptr,
                consider_inheritance.into(),
                pool.as_mut_ptr(),
            );
            svn_result(err)?;
            Ok(Rangelist::from_ptr_and_pool(output, pool))
        }
    }

    /// Reverse this rangelist in place.
    pub fn reverse(&mut self) -> Result<(), Error> {
        unsafe {
            let err = subversion_sys::svn_rangelist_reverse(self.ptr, self.pool.as_mut_ptr());
            svn_result(err)
        }
    }

    /// Duplicate this rangelist.
    pub fn dup(&self) -> Rangelist {
        let pool = apr::Pool::new();
        unsafe {
            let result = subversion_sys::svn_rangelist_dup(self.ptr, pool.as_mut_ptr());
            Rangelist::from_ptr_and_pool(result, pool)
        }
    }

    /// Get the ranges as a Vec.
    pub fn ranges(&self) -> Vec<MergeRange> {
        if self.ptr.is_null() {
            return Vec::new();
        }

        unsafe {
            let array =
                apr::tables::TypedArray::<subversion_sys::svn_merge_range_t>::from_ptr(self.ptr);
            array.iter().map(|r| MergeRange::from(r)).collect()
        }
    }

    /// Check if the rangelist is empty.
    pub fn is_empty(&self) -> bool {
        if self.ptr.is_null() {
            return true;
        }
        unsafe { (*self.ptr).nelts == 0 }
    }

    /// Get the number of ranges.
    pub fn len(&self) -> usize {
        if self.ptr.is_null() {
            return 0;
        }
        unsafe { (*self.ptr).nelts as usize }
    }
}

impl Default for Rangelist {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mergeinfo_parse() {
        let mergeinfo = Mergeinfo::parse("/trunk:1-10").unwrap();
        let s = mergeinfo.to_string().unwrap();
        assert_eq!(s, "/trunk:1-10");
    }

    #[test]
    fn test_mergeinfo_inheritance_conversion() {
        // Test conversion from SVN constants to enum
        assert_eq!(
            MergeinfoInheritance::from(
                subversion_sys::svn_mergeinfo_inheritance_t_svn_mergeinfo_explicit
            ),
            MergeinfoInheritance::Explicit
        );
        assert_eq!(
            MergeinfoInheritance::from(
                subversion_sys::svn_mergeinfo_inheritance_t_svn_mergeinfo_inherited
            ),
            MergeinfoInheritance::Inherited
        );
        assert_eq!(
            MergeinfoInheritance::from(
                subversion_sys::svn_mergeinfo_inheritance_t_svn_mergeinfo_nearest_ancestor
            ),
            MergeinfoInheritance::NearestAncestor
        );
    }

    #[test]
    fn test_mergeinfo_inheritance_to_svn() {
        // Test conversion from enum to SVN constants
        assert_eq!(
            subversion_sys::svn_mergeinfo_inheritance_t::from(MergeinfoInheritance::Explicit),
            subversion_sys::svn_mergeinfo_inheritance_t_svn_mergeinfo_explicit
        );
        assert_eq!(
            subversion_sys::svn_mergeinfo_inheritance_t::from(MergeinfoInheritance::Inherited),
            subversion_sys::svn_mergeinfo_inheritance_t_svn_mergeinfo_inherited
        );
        assert_eq!(
            subversion_sys::svn_mergeinfo_inheritance_t::from(
                MergeinfoInheritance::NearestAncestor
            ),
            subversion_sys::svn_mergeinfo_inheritance_t_svn_mergeinfo_nearest_ancestor
        );
    }

    #[test]
    fn test_mergeinfo_roundtrip_conversion() {
        // Test that converting back and forth preserves values
        let variants = vec![
            MergeinfoInheritance::Explicit,
            MergeinfoInheritance::Inherited,
            MergeinfoInheritance::NearestAncestor,
        ];

        for variant in variants {
            let svn_val = subversion_sys::svn_mergeinfo_inheritance_t::from(variant);
            let back = MergeinfoInheritance::from(svn_val);
            assert_eq!(variant, back);
        }
    }

    #[test]
    fn test_mergeinfo_drop() {
        // Test that Mergeinfo can be dropped without panic
        {
            let _mergeinfo = Mergeinfo::parse("/trunk:1-5").unwrap();
            // mergeinfo is dropped here
        }
        // No panic should occur
    }

    #[test]
    fn test_mergeinfo_dup() {
        let mergeinfo = Mergeinfo::parse("/trunk:1-10").unwrap();
        let dup = mergeinfo.dup();
        assert_eq!(dup.to_string().unwrap(), "/trunk:1-10");
    }

    #[test]
    fn test_rangelist_new() {
        let rangelist = Rangelist::new();
        assert!(rangelist.is_empty());
        assert_eq!(rangelist.len(), 0);
    }

    #[test]
    fn test_merge_range_from() {
        let range = MergeRange {
            start: Revnum(1),
            end: Revnum(10),
            inheritable: true,
        };
        assert_eq!(range.start.0, 1);
        assert_eq!(range.end.0, 10);
        assert!(range.inheritable);
    }

    #[test]
    fn test_rangelist_dup() {
        let rangelist = Rangelist::new();
        let dup = rangelist.dup();
        assert!(dup.is_empty());
    }

    #[test]
    fn test_rangelist_to_string_empty() {
        let rangelist = Rangelist::new();
        let s = rangelist.to_string().unwrap();
        assert_eq!(s, "");
    }

    #[test]
    fn test_rangelist_merge() {
        let mut r1 = Rangelist::new();
        let r2 = Rangelist::new();
        r1.merge(&r2).unwrap();
        assert!(r1.is_empty());
    }

    #[test]
    fn test_rangelist_intersect() {
        let r1 = Rangelist::new();
        let r2 = Rangelist::new();
        let intersected = Rangelist::intersect(&r1, &r2, true).unwrap();
        assert!(intersected.is_empty());
    }

    #[test]
    fn test_rangelist_reverse() {
        let mut rangelist = Rangelist::new();
        rangelist.reverse().unwrap();
        assert!(rangelist.is_empty());
    }

    #[test]
    fn test_rangelist_ranges_empty() {
        let rangelist = Rangelist::new();
        let ranges = rangelist.ranges();
        assert!(ranges.is_empty());
    }
}
