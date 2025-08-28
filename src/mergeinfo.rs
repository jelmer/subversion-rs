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
