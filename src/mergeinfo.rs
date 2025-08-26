use apr::pool::PooledPtr;
use subversion_sys::svn_mergeinfo_t;

#[allow(dead_code)]
pub struct Mergeinfo(pub(crate) PooledPtr<svn_mergeinfo_t>);

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
