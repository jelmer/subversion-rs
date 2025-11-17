use crate::{Error, Revnum};
use std::ffi::{CStr, CString};
use std::os::raw::c_void;

/// Type of conflict that occurred
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictKind {
    /// Text or content conflict
    Text,
    /// Property conflict
    Property,
    /// Tree conflict (structural changes)
    Tree,
}

impl From<subversion_sys::svn_wc_conflict_kind_t> for ConflictKind {
    fn from(kind: subversion_sys::svn_wc_conflict_kind_t) -> Self {
        match kind {
            0 => ConflictKind::Text,     // svn_wc_conflict_kind_text
            1 => ConflictKind::Property, // svn_wc_conflict_kind_property
            2 => ConflictKind::Tree,     // svn_wc_conflict_kind_tree
            _ => ConflictKind::Text,     // Default fallback
        }
    }
}

/// The action that caused a conflict
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictAction {
    /// Edit action on the item.
    Edit,
    /// Add action on the item.
    Add,
    /// Delete action on the item.
    Delete,
    /// Replace action on the item.
    Replace,
}

impl From<subversion_sys::svn_wc_conflict_action_t> for ConflictAction {
    fn from(action: subversion_sys::svn_wc_conflict_action_t) -> Self {
        match action {
            0 => ConflictAction::Edit,    // svn_wc_conflict_action_edit
            1 => ConflictAction::Add,     // svn_wc_conflict_action_add
            2 => ConflictAction::Delete,  // svn_wc_conflict_action_delete
            3 => ConflictAction::Replace, // svn_wc_conflict_action_replace
            _ => ConflictAction::Edit,
        }
    }
}

/// The reason for a conflict
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictReason {
    /// Item was edited.
    Edited,
    /// Item was obstructed.
    Obstructed,
    /// Item was deleted.
    Deleted,
    /// Item is missing.
    Missing,
    /// Item is unversioned.
    Unversioned,
    /// Item was added.
    Added,
    /// Item was replaced.
    Replaced,
    /// Item was moved away.
    MovedAway,
    /// Item was moved here.
    MovedHere,
}

impl From<subversion_sys::svn_wc_conflict_reason_t> for ConflictReason {
    fn from(reason: subversion_sys::svn_wc_conflict_reason_t) -> Self {
        match reason {
            0 => ConflictReason::Edited,      // svn_wc_conflict_reason_edited
            1 => ConflictReason::Obstructed,  // svn_wc_conflict_reason_obstructed
            2 => ConflictReason::Deleted,     // svn_wc_conflict_reason_deleted
            3 => ConflictReason::Missing,     // svn_wc_conflict_reason_missing
            4 => ConflictReason::Unversioned, // svn_wc_conflict_reason_unversioned
            5 => ConflictReason::Added,       // svn_wc_conflict_reason_added
            6 => ConflictReason::Replaced,    // svn_wc_conflict_reason_replaced
            7 => ConflictReason::MovedAway,   // svn_wc_conflict_reason_moved_away
            8 => ConflictReason::MovedHere,   // svn_wc_conflict_reason_moved_here
            _ => ConflictReason::Edited,
        }
    }
}

/// Information about one side of a conflict
#[derive(Debug, Clone)]
pub struct ConflictVersion {
    /// Repository root URL
    pub repos_url: String,
    /// Peg revision
    pub peg_revision: Revnum,
    /// Path in repository
    pub path_in_repos: String,
    /// Node kind
    pub node_kind: crate::NodeKind,
}

impl ConflictVersion {
    unsafe fn from_raw(ptr: *const subversion_sys::svn_wc_conflict_version_t) -> Option<Self> {
        if ptr.is_null() {
            return None;
        }

        let version = &*ptr;
        Some(Self {
            repos_url: CStr::from_ptr(version.repos_url)
                .to_string_lossy()
                .into_owned(),
            peg_revision: Revnum(version.peg_rev),
            path_in_repos: CStr::from_ptr(version.path_in_repos)
                .to_string_lossy()
                .into_owned(),
            node_kind: version.node_kind.into(),
        })
    }
}

/// Description of a conflict that occurred
#[derive(Debug, Clone)]
pub struct ConflictDescription {
    /// The local path that is in conflict
    pub local_abspath: String,
    /// The node type involved in this conflict
    pub node_kind: crate::NodeKind,
    /// What sort of conflict
    pub kind: ConflictKind,
    /// Property name (if property conflict)
    pub property_name: Option<String>,
    /// Whether the file is binary
    pub is_binary: bool,
    /// MIME type of the file
    pub mime_type: Option<String>,
    /// The action that caused the conflict
    pub action: ConflictAction,
    /// The reason for the conflict
    pub reason: ConflictReason,
    /// Base file (common ancestor)
    pub base_file: Option<String>,
    /// Their file (repository version)
    pub their_file: Option<String>,
    /// My file (local version)
    pub my_file: Option<String>,
    /// Merged file (if already merged)
    pub merged_file: Option<String>,
    /// Left version of the conflict
    pub src_left_version: Option<ConflictVersion>,
    /// Right version of the conflict
    pub src_right_version: Option<ConflictVersion>,
}

impl ConflictDescription {
    /// Create from raw SVN conflict description
    pub unsafe fn from_raw(desc: *const subversion_sys::svn_wc_conflict_description2_t) -> Self {
        let d = &*desc;

        Self {
            local_abspath: CStr::from_ptr(d.local_abspath)
                .to_string_lossy()
                .into_owned(),
            node_kind: d.node_kind.into(),
            kind: d.kind.into(),
            property_name: if d.property_name.is_null() {
                None
            } else {
                Some(
                    CStr::from_ptr(d.property_name)
                        .to_string_lossy()
                        .into_owned(),
                )
            },
            is_binary: d.is_binary != 0,
            mime_type: if d.mime_type.is_null() {
                None
            } else {
                Some(CStr::from_ptr(d.mime_type).to_string_lossy().into_owned())
            },
            action: d.action.into(),
            reason: d.reason.into(),
            base_file: if d.base_abspath.is_null() {
                None
            } else {
                Some(
                    CStr::from_ptr(d.base_abspath)
                        .to_string_lossy()
                        .into_owned(),
                )
            },
            their_file: if d.their_abspath.is_null() {
                None
            } else {
                Some(
                    CStr::from_ptr(d.their_abspath)
                        .to_string_lossy()
                        .into_owned(),
                )
            },
            my_file: if d.my_abspath.is_null() {
                None
            } else {
                Some(CStr::from_ptr(d.my_abspath).to_string_lossy().into_owned())
            },
            merged_file: if d.merged_file.is_null() {
                None
            } else {
                Some(CStr::from_ptr(d.merged_file).to_string_lossy().into_owned())
            },
            src_left_version: ConflictVersion::from_raw(d.src_left_version),
            src_right_version: ConflictVersion::from_raw(d.src_right_version),
        }
    }
}

/// Resolution choice for a conflict
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictChoice {
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
    /// Undefined (for internal use)
    Undefined,
}

impl ConflictChoice {
    /// Converts the conflict choice to its raw SVN representation.
    pub fn to_raw(&self) -> subversion_sys::svn_wc_conflict_choice_t {
        match self {
            ConflictChoice::Undefined => -1, // svn_wc_conflict_choose_undefined
            ConflictChoice::Postpone => 0,   // svn_wc_conflict_choose_postpone
            ConflictChoice::Base => 1,       // svn_wc_conflict_choose_base
            ConflictChoice::TheirsFull => 2, // svn_wc_conflict_choose_theirs_full
            ConflictChoice::MineFull => 3,   // svn_wc_conflict_choose_mine_full
            ConflictChoice::TheirsConflict => 4, // svn_wc_conflict_choose_theirs_conflict
            ConflictChoice::MineConflict => 5, // svn_wc_conflict_choose_mine_conflict
            ConflictChoice::Merged => 6,     // svn_wc_conflict_choose_merged
        }
    }

    /// Convert to client conflict option ID for text conflicts
    pub fn to_text_option_id(&self) -> subversion_sys::svn_client_conflict_option_id_t {
        match self {
            ConflictChoice::Undefined => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_undefined,
            ConflictChoice::Postpone => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_postpone,
            ConflictChoice::Base => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_base_text,
            ConflictChoice::TheirsFull => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_incoming_text,
            ConflictChoice::MineFull => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_working_text,
            ConflictChoice::TheirsConflict => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_incoming_text_where_conflicted,
            ConflictChoice::MineConflict => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_working_text_where_conflicted,
            ConflictChoice::Merged => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_merged_text,
        }
    }

    /// Convert to client conflict option ID for tree conflicts
    pub fn to_tree_option_id(&self) -> subversion_sys::svn_client_conflict_option_id_t {
        match self {
            ConflictChoice::Postpone => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_postpone,
            ConflictChoice::Base => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_accept_current_wc_state,
            ConflictChoice::TheirsFull => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_incoming_delete_accept,
            ConflictChoice::MineFull => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_incoming_delete_ignore,
            // For unsupported tree conflict choices, default to postpone
            _ => subversion_sys::svn_client_conflict_option_id_t_svn_client_conflict_option_postpone,
        }
    }
}

/// Result of conflict resolution
#[derive(Debug, Clone)]
pub struct ConflictResult {
    /// The choice made to resolve the conflict
    pub choice: ConflictChoice,
    /// Path to merged file (if choice is Merged)
    pub merged_file: Option<String>,
    /// Whether to save the resolution to the working copy
    pub save_merged: bool,
}

impl Default for ConflictResult {
    fn default() -> Self {
        Self {
            choice: ConflictChoice::Postpone,
            merged_file: None,
            save_merged: false,
        }
    }
}

/// Trait for implementing conflict resolution
pub trait ConflictResolver: Send + Sync {
    /// Resolve a conflict
    ///
    /// This method is called when a conflict is encountered during a merge
    /// or update operation. The implementation should examine the conflict
    /// description and return a resolution choice.
    fn resolve(&mut self, conflict: &ConflictDescription) -> Result<ConflictResult, Error>;
}

/// A simple conflict resolver that always makes the same choice
pub struct SimpleConflictResolver {
    choice: ConflictChoice,
}

impl SimpleConflictResolver {
    /// Create a resolver that always chooses the same resolution
    pub fn new(choice: ConflictChoice) -> Self {
        Self { choice }
    }

    /// Create a resolver that always postpones
    pub fn postpone() -> Self {
        Self::new(ConflictChoice::Postpone)
    }

    /// Create a resolver that always chooses theirs
    pub fn theirs() -> Self {
        Self::new(ConflictChoice::TheirsFull)
    }

    /// Create a resolver that always chooses mine
    pub fn mine() -> Self {
        Self::new(ConflictChoice::MineFull)
    }
}

impl ConflictResolver for SimpleConflictResolver {
    fn resolve(&mut self, _conflict: &ConflictDescription) -> Result<ConflictResult, Error> {
        Ok(ConflictResult {
            choice: self.choice,
            merged_file: None,
            save_merged: false,
        })
    }
}

/// Interactive conflict resolver that prompts the user
pub struct InteractiveConflictResolver;

impl ConflictResolver for InteractiveConflictResolver {
    fn resolve(&mut self, conflict: &ConflictDescription) -> Result<ConflictResult, Error> {
        use std::io::{self, Write};

        println!("\n=== Conflict in {} ===", conflict.local_abspath);
        println!("Kind: {:?}", conflict.kind);
        println!(
            "Action: {:?} vs Reason: {:?}",
            conflict.action, conflict.reason
        );

        if let Some(ref prop) = conflict.property_name {
            println!("Property: {}", prop);
        }

        println!("\nOptions:");
        println!("  (p) Postpone");
        println!("  (b) Use base/original");
        println!("  (t) Use theirs (incoming)");
        println!("  (m) Use mine (local)");
        println!("  (tc) Use theirs for conflict regions only");
        println!("  (mc) Use mine for conflict regions only");

        loop {
            print!("Choice: ");
            io::stdout().flush().unwrap();

            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();

            let choice = match input.trim() {
                "p" => ConflictChoice::Postpone,
                "b" => ConflictChoice::Base,
                "t" => ConflictChoice::TheirsFull,
                "m" => ConflictChoice::MineFull,
                "tc" => ConflictChoice::TheirsConflict,
                "mc" => ConflictChoice::MineConflict,
                _ => {
                    println!("Invalid choice, please try again");
                    continue;
                }
            };

            return Ok(ConflictResult {
                choice,
                merged_file: None,
                save_merged: false,
            });
        }
    }
}

/// Storage for conflict resolver in client context
pub(crate) struct ConflictResolverBaton {
    pub resolver: Box<dyn ConflictResolver>,
}

/// C callback function for conflict resolution
pub(crate) unsafe extern "C" fn conflict_resolver_callback(
    result: *mut *mut subversion_sys::svn_wc_conflict_result_t,
    description: *const subversion_sys::svn_wc_conflict_description2_t,
    baton: *mut c_void,
    result_pool: *mut apr_sys::apr_pool_t,
    _scratch_pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    if baton.is_null() {
        return std::ptr::null_mut();
    }

    let resolver_baton = &mut *(baton as *mut ConflictResolverBaton);
    let conflict_desc = ConflictDescription::from_raw(description);

    match resolver_baton.resolver.resolve(&conflict_desc) {
        Ok(resolution) => {
            // Allocate result structure in the result pool (owned by SVN)
            let pool = unsafe { apr::PoolHandle::from_borrowed_raw(result_pool) };
            let conflict_result: *mut subversion_sys::svn_wc_conflict_result_t = pool.calloc();

            (*conflict_result).choice = resolution.choice.to_raw();

            if let Some(ref merged_path) = resolution.merged_file {
                let merged_cstr = CString::new(merged_path.as_str()).unwrap();
                (*conflict_result).merged_file =
                    apr::strings::pstrdup_raw(merged_cstr.to_str().unwrap(), &pool).unwrap()
                        as *const _;
            } else {
                (*conflict_result).merged_file = std::ptr::null();
            }

            (*conflict_result).save_merged = if resolution.save_merged { 1 } else { 0 };

            *result = conflict_result;
            std::ptr::null_mut()
        }
        Err(e) => e.into_raw(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_resolver() {
        let mut resolver = SimpleConflictResolver::theirs();
        let conflict = ConflictDescription {
            local_abspath: "/path/to/file".to_string(),
            node_kind: crate::NodeKind::File,
            kind: ConflictKind::Text,
            property_name: None,
            is_binary: false,
            mime_type: None,
            action: ConflictAction::Edit,
            reason: ConflictReason::Edited,
            base_file: None,
            their_file: None,
            my_file: None,
            merged_file: None,
            src_left_version: None,
            src_right_version: None,
        };

        let result = resolver.resolve(&conflict).unwrap();
        assert_eq!(result.choice, ConflictChoice::TheirsFull);
    }

    #[test]
    fn test_conflict_choice_conversion() {
        assert_eq!(
            ConflictChoice::Postpone.to_raw(),
            0 // svn_wc_conflict_choose_postpone
        );
        assert_eq!(
            ConflictChoice::TheirsFull.to_raw(),
            2 // svn_wc_conflict_choose_theirs_full
        );
    }
}
