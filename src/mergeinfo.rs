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
}
