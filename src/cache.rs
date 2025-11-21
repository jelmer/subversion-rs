//! Cache configuration for Subversion operations
//!
//! This module provides utilities for configuring Subversion's internal caching
//! mechanisms, which can significantly improve performance for certain operations.

/// Cache configuration settings
///
/// This corresponds to SVN's `svn_cache_config_t` structure.
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Total cache size in bytes. This is a soft limit.
    /// May be 0, resulting in default caching code being used.
    pub cache_size: u64,
    /// Maximum number of files kept open
    pub file_handle_count: usize,
    /// Is this application guaranteed to be single-threaded?
    pub single_threaded: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        // Get default from SVN
        let svn_config = unsafe { subversion_sys::svn_cache_config_get() };
        if svn_config.is_null() {
            Self {
                cache_size: 0,
                file_handle_count: 0,
                single_threaded: false,
            }
        } else {
            unsafe {
                Self {
                    cache_size: (*svn_config).cache_size,
                    file_handle_count: (*svn_config).file_handle_count as usize,
                    single_threaded: (*svn_config).single_threaded != 0,
                }
            }
        }
    }
}

/// Sets the global cache configuration.
pub fn set_cache_config(config: &CacheConfig) {
    let svn_config = subversion_sys::svn_cache_config_t {
        cache_size: config.cache_size,
        file_handle_count: config.file_handle_count as apr_sys::apr_size_t,
        single_threaded: if config.single_threaded { 1 } else { 0 },
    };
    unsafe {
        subversion_sys::svn_cache_config_set(&svn_config);
    }
}

/// Get current cache configuration
///
/// Returns the current cache configuration settings from SVN.
pub fn get_cache_config() -> CacheConfig {
    let svn_config = unsafe { subversion_sys::svn_cache_config_get() };
    if svn_config.is_null() {
        CacheConfig::default()
    } else {
        unsafe {
            CacheConfig {
                cache_size: (*svn_config).cache_size,
                file_handle_count: (*svn_config).file_handle_count as usize,
                single_threaded: (*svn_config).single_threaded != 0,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_config_default() {
        let config = CacheConfig::default();
        // Default values come from SVN, just check we can create it
        let _ = config.cache_size;
        let _ = config.file_handle_count;
        let _ = config.single_threaded;
    }

    #[test]
    fn test_cache_config_custom() {
        let config = CacheConfig {
            cache_size: 32 * 1024 * 1024,
            file_handle_count: 100,
            single_threaded: true,
        };

        assert_eq!(config.cache_size, 32 * 1024 * 1024);
        assert_eq!(config.file_handle_count, 100);
        assert!(config.single_threaded);
    }

    #[test]
    fn test_set_and_get_cache_config() {
        let config = CacheConfig {
            cache_size: 16 * 1024 * 1024,
            file_handle_count: 50,
            single_threaded: false,
        };

        set_cache_config(&config);
        let retrieved = get_cache_config();

        assert_eq!(retrieved.cache_size, config.cache_size);
        assert_eq!(retrieved.file_handle_count, config.file_handle_count);
        assert_eq!(retrieved.single_threaded, config.single_threaded);
    }
}
