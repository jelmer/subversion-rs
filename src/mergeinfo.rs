use apr::pool::PooledPtr;
use crate::generated::svn_mergeinfo_t;

#[allow(dead_code)]
pub struct Mergeinfo(pub(crate) PooledPtr<svn_mergeinfo_t>);

#[derive(Debug, PartialEq)]
pub enum MergeinfoInheritance {
    Explicit,
    Inherited,
    NearestAncestor,
}

impl From<crate::generated::svn_mergeinfo_inheritance_t> for MergeinfoInheritance {
    fn from(value: crate::generated::svn_mergeinfo_inheritance_t) -> Self {
        match value {
            crate::generated::svn_mergeinfo_inheritance_t_svn_mergeinfo_nearest_ancestor => MergeinfoInheritance::NearestAncestor,
            crate::generated::svn_mergeinfo_inheritance_t_svn_mergeinfo_explicit => MergeinfoInheritance::Explicit,
            crate::generated::svn_mergeinfo_inheritance_t_svn_mergeinfo_inherited => MergeinfoInheritance::Inherited,
            _ => unreachable!()
        }
    }
}

impl From<MergeinfoInheritance> for crate::generated::svn_mergeinfo_inheritance_t {
    fn from(value: MergeinfoInheritance) -> Self {
        match value {
            MergeinfoInheritance::NearestAncestor => crate::generated::svn_mergeinfo_inheritance_t_svn_mergeinfo_nearest_ancestor,
            MergeinfoInheritance::Explicit => crate::generated::svn_mergeinfo_inheritance_t_svn_mergeinfo_explicit,
            MergeinfoInheritance::Inherited => crate::generated::svn_mergeinfo_inheritance_t_svn_mergeinfo_inherited
        }
    }
}
