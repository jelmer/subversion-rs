//! Native Language Support (NLS) initialization for Subversion.

use crate::Error;

/// Initialize NLS.
///
/// This initializes the Native Language Support system for Subversion,
/// setting up message catalogs and locale handling.
pub fn init() -> Result<(), Error<'static>> {
    let err = unsafe { subversion_sys::svn_nls_init() };
    crate::Error::from_raw(err)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nls_init() {
        init().unwrap();
    }
}
