//! Command-line utilities for Subversion applications
//!
//! This module provides utilities for command-line applications using Subversion,
//! including initialization.

/// Initialize command-line application
///
/// This function initializes the Subversion command-line environment:
/// - Initializes APR
/// - Sets up locale and encoding
/// - Configures stderr for error output
///
/// Returns the exit code that should be used if initialization fails.
/// A return value of 0 indicates success.
pub fn init(program_name: &str) -> Result<(), i32> {
    let program_name_cstr = std::ffi::CString::new(program_name).map_err(|_| 1)?;

    let result = unsafe {
        subversion_sys::svn_cmdline_init(program_name_cstr.as_ptr(), std::ptr::null_mut())
    };

    if result == 0 {
        Ok(())
    } else {
        Err(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        init("test-program").unwrap();
    }
}
