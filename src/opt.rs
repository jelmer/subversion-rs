//! Command-line option processing utilities
//!
//! This module provides safe Rust wrappers around the Subversion `svn_opt_*` functions
//! for processing command-line arguments, parsing revision specifications, and handling
//! path targets.

use crate::{error::Error, Revision};

/// Parse a revision specification from a string
///
/// This function accepts various forms of revision specifiers like:
/// - Numbers: "123"
/// - Keywords: "HEAD", "BASE", "COMMITTED", "PREV"
/// - Dates: "{2023-01-01}", "{2023-01-01T12:00:00}"
///
/// Returns the parsed start and end revisions.
pub fn parse_revision(arg: &str) -> Result<(Revision, Revision), Error> {
    let pool = apr::Pool::new();
    let arg_cstr = std::ffi::CString::new(arg)?;
    let mut start_rev = subversion_sys::svn_opt_revision_t::default();
    let mut end_rev = subversion_sys::svn_opt_revision_t::default();

    unsafe {
        let result = subversion_sys::svn_opt_parse_revision(
            &mut start_rev,
            &mut end_rev,
            arg_cstr.as_ptr(),
            pool.as_mut_ptr(),
        );
        if result != 0 {
            return Err(Error::from_str("Failed to parse revision"));
        }

        Ok((Revision::from(start_rev), Revision::from(end_rev)))
    }
}

/// Parse a path@revision string into path and revision components
///
/// This function parses strings like "path@123" or "path@HEAD" into
/// separate path and revision components.
pub fn parse_path(path: &str) -> Result<(String, Revision), Error> {
    let pool = apr::Pool::new();
    let path_cstr = std::ffi::CString::new(path)?;
    let mut revision = subversion_sys::svn_opt_revision_t::default();
    let mut true_path = std::ptr::null();

    unsafe {
        let err = subversion_sys::svn_opt_parse_path(
            &mut revision,
            &mut true_path,
            path_cstr.as_ptr(),
            pool.as_mut_ptr(),
        );
        Error::from_raw(err)?;

        let path_str = if true_path.is_null() {
            path.to_string()
        } else {
            let true_path_str = std::ffi::CStr::from_ptr(true_path);
            true_path_str.to_string_lossy().into_owned()
        };

        Ok((path_str, Revision::from(revision)))
    }
}

/// Resolve revision specifications to concrete revision numbers
///
/// This function takes peg and operational revision specifications and resolves
/// them according to SVN's revision resolution rules.
pub fn resolve_revisions(
    peg_revision: &Revision,
    op_revision: &Revision,
    is_url: bool,
    notice_local_mods: bool,
) -> Result<(Revision, Revision), Error> {
    let pool = apr::Pool::new();
    let mut peg_rev = (*peg_revision).into();
    let mut op_rev = (*op_revision).into();

    unsafe {
        let err = subversion_sys::svn_opt_resolve_revisions(
            &mut peg_rev,
            &mut op_rev,
            is_url as i32,
            notice_local_mods as i32,
            pool.as_mut_ptr(),
        );
        Error::from_raw(err)?;

        Ok((Revision::from(peg_rev), Revision::from(op_rev)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Revision;

    #[test]
    fn test_parse_revision_number() {
        let (start, end) = parse_revision("123").unwrap();

        // When parsing just a number, SVN puts it in the start_revision
        assert!(matches!(start, Revision::Number(_)));
        // The end should be unspecified for a simple number
        assert!(matches!(end, Revision::Unspecified));
    }

    #[test]
    fn test_parse_revision_keywords() {
        let (start, end) = parse_revision("HEAD").unwrap();
        // When parsing keywords, SVN puts them in the start_revision
        assert!(matches!(start, Revision::Head));
        assert!(matches!(end, Revision::Unspecified));

        let (start2, end2) = parse_revision("BASE").unwrap();
        assert!(matches!(start2, Revision::Base));
        assert!(matches!(end2, Revision::Unspecified));
    }

    #[test]
    fn test_parse_path_with_revision() {
        let (path, revision) = parse_path("src/main.rs@123").unwrap();

        assert_eq!(path, "src/main.rs");
        assert!(matches!(revision, Revision::Number(_)));
    }

    #[test]
    fn test_parse_path_without_revision() {
        let (path, revision) = parse_path("src/main.rs").unwrap();

        assert_eq!(path, "src/main.rs");
        // When no revision is specified, it should remain unspecified
        assert!(matches!(revision, Revision::Unspecified));
    }
}
