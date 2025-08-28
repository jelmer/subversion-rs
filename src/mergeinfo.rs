use std::marker::PhantomData;
use subversion_sys::svn_mergeinfo_t;

/// Mergeinfo handle with RAII cleanup
#[allow(dead_code)]
pub struct Mergeinfo {
    ptr: *mut svn_mergeinfo_t,
    pool: apr::Pool,
    _phantom: PhantomData<*mut ()>, // !Send + !Sync
}

impl Mergeinfo {
    pub(crate) unsafe fn from_ptr_and_pool(ptr: *mut svn_mergeinfo_t, pool: apr::Pool) -> Self {
        Self {
            ptr,
            pool,
            _phantom: PhantomData,
        }
    }

    pub fn as_ptr(&self) -> *const svn_mergeinfo_t {
        self.ptr
    }

    pub fn as_mut_ptr(&mut self) -> *mut svn_mergeinfo_t {
        self.ptr
    }
}

impl Drop for Mergeinfo {
    fn drop(&mut self) {
        // Pool drop will clean up mergeinfo
    }
}

#[derive(Debug, PartialEq)]
pub enum MergeinfoInheritance {
    Explicit,
    Inherited,
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
    fn test_mergeinfo_creation() {
        // Test creating Mergeinfo from raw pointer and pool
        let pool = apr::Pool::new();
        let ptr = pool.calloc::<svn_mergeinfo_t>();

        let mergeinfo = unsafe { Mergeinfo::from_ptr_and_pool(ptr, pool) };
        assert_eq!(mergeinfo.as_ptr(), ptr);
        assert!(!mergeinfo.as_ptr().is_null());
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
            let svn_val = subversion_sys::svn_mergeinfo_inheritance_t::from(variant.clone());
            let back = MergeinfoInheritance::from(svn_val);
            assert_eq!(variant, back);
        }
    }

    #[test]
    fn test_mergeinfo_drop() {
        // Test that Mergeinfo can be dropped without panic
        let pool = apr::Pool::new();
        let ptr = pool.calloc::<svn_mergeinfo_t>();

        {
            let _mergeinfo = unsafe { Mergeinfo::from_ptr_and_pool(ptr, pool.clone()) };
            // mergeinfo is dropped here
        }

        // No panic should occur
    }

    #[test]
    fn test_mergeinfo_not_send_not_sync() {
        // Verify that Mergeinfo is !Send and !Sync due to PhantomData<*mut ()>
        fn assert_not_send<T>()
        where
            T: ?Sized,
        {
            // This function body is empty - the check happens at compile time
        }

        fn assert_not_sync<T>()
        where
            T: ?Sized,
        {
            // This function body is empty - the check happens at compile time
        }

        // These will compile only if Mergeinfo is !Send and !Sync
        assert_not_send::<Mergeinfo>();
        assert_not_sync::<Mergeinfo>();
    }

    #[test]
    fn test_mergeinfo_as_mut_ptr() {
        let pool = apr::Pool::new();
        let ptr = pool.calloc::<svn_mergeinfo_t>();

        let mut mergeinfo = unsafe { Mergeinfo::from_ptr_and_pool(ptr, pool) };
        let mut_ptr = mergeinfo.as_mut_ptr();
        assert_eq!(mut_ptr, ptr);
    }
}
