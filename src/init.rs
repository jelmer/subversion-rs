// Initialization module for Subversion libraries
//
// Subversion libraries require explicit initialization for thread safety.
// Without proper initialization, concurrent access can cause crashes.

use crate::Error;
use std::sync::Once;

static INIT: Once = Once::new();
static mut INIT_RESULT: Result<(), String> = Ok(());

/// Initialize all Subversion libraries.
///
/// This function must be called before using any Subversion functionality.
/// It is safe to call multiple times - subsequent calls will be no-ops.
///
/// This initializes:
/// - svn_dso_initialize2() - Dynamic library loading
/// - svn_fs_initialize() - Filesystem library
/// - svn_ra_initialize() - Repository access library
///
/// # Thread Safety
///
/// This function uses `std::sync::Once` to ensure thread-safe initialization
/// even when called concurrently from multiple threads.
pub fn initialize() -> Result<(), Error> {
    INIT.call_once(|| {
        // We need a root pool that lives for the duration of the program
        // APR is already initialized by the apr crate
        // IMPORTANT: Use Box::leak() to heap-allocate the pool, not std::mem::forget()!
        // The pool must be on the heap because svn_fs_initialize and svn_ra_initialize
        // store internal pointers to data allocated within this pool. If the pool wrapper
        // is on the stack, std::mem::forget() only prevents the destructor from running,
        // but the stack memory can still be reused, causing use-after-free corruption.
        let pool = Box::new(apr::pool::Pool::new());
        let pool = Box::leak(pool);

        unsafe {
            // Initialize DSO first (required for dynamic library loading)
            // Note: svn_dso_initialize2 requires APR to be initialized first
            let err = subversion_sys::svn_dso_initialize2();
            if !err.is_null() {
                let error = Error::from_raw(err);
                INIT_RESULT = Err(format!("Failed to initialize DSO: {:?}", error));
                return;
            }

            // Initialize filesystem library
            let err = subversion_sys::svn_fs_initialize(pool.as_mut_ptr());
            if !err.is_null() {
                let error = Error::from_raw(err);
                INIT_RESULT = Err(format!("Failed to initialize FS: {:?}", error));
                return;
            }

            // Initialize repository access library
            // Note: The client library depends on RA, so we need to initialize it
            // whenever either ra or client features are enabled
            #[cfg(any(feature = "ra", feature = "client"))]
            {
                let err = subversion_sys::svn_ra_initialize(pool.as_mut_ptr());
                if !err.is_null() {
                    let error = Error::from_raw(err);
                    INIT_RESULT = Err(format!("Failed to initialize RA: {:?}", error));
                    return;
                }
            }

            // Pool is already leaked via Box::leak() - it will live for program lifetime
        }
    });

    // Check if initialization succeeded
    unsafe {
        let result_ptr = &raw const INIT_RESULT;
        match &*result_ptr {
            Ok(()) => Ok(()),
            Err(msg) => Err(Error::from_str(msg)),
        }
    }
}

/// Initialize Subversion libraries for tests.
///
/// This is automatically called by the test harness.
#[cfg(test)]
pub fn initialize_for_tests() {
    if let Err(e) = initialize() {
        panic!("Failed to initialize Subversion libraries: {:?}", e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialize_multiple_times() {
        // Should be safe to call multiple times
        for _ in 0..5 {
            assert!(initialize().is_ok());
        }
    }

    #[test]
    fn test_initialize_concurrent() {
        use std::thread;

        let handles: Vec<_> = (0..10)
            .map(|_| {
                thread::spawn(|| {
                    initialize().expect("Failed to initialize");
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }
    }
}
