#![allow(bad_style)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(dead_code)]
#![allow(clippy::upper_case_acronyms)]
#![allow(unnecessary_transmutes)]
#![allow(clippy::missing_safety_doc)]
#![allow(clippy::ptr_offset_with_cast)]
#![allow(clippy::type_complexity)]

pub use apr;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

// Private libsvn_wc APIs (declared in private/svn_wc_private.h, which is not
// installed with the public headers, so bindgen cannot pick them up). The
// symbols are exported from libsvn_wc-1.so. They are needed to acquire a
// working-copy write lock on an svn_wc_context_t, which is required to drive
// svn_wc_process_committed_queue2.
#[cfg(feature = "wc")]
extern "C" {
    pub fn svn_wc__acquire_write_lock(
        lock_root_abspath: *mut *const std::os::raw::c_char,
        wc_ctx: *mut svn_wc_context_t,
        local_abspath: *const std::os::raw::c_char,
        lock_anchor: svn_boolean_t,
        result_pool: *mut apr_pool_t,
        scratch_pool: *mut apr_pool_t,
    ) -> *mut svn_error_t;

    pub fn svn_wc__release_write_lock(
        wc_ctx: *mut svn_wc_context_t,
        local_abspath: *const std::os::raw::c_char,
        scratch_pool: *mut apr_pool_t,
    ) -> *mut svn_error_t;
}
