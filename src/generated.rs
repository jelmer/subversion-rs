#![allow(bad_style)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(dead_code)]
#![allow(clippy::upper_case_acronyms)]
include!(concat!(env!("OUT_DIR"), "/subversion.rs"));

use apr::apr_byte_t;
use apr::apr_file_t;
use apr::apr_finfo_t;
use apr::apr_getopt_t;

use apr::apr_int64_t;
use apr::apr_off_t;
use apr::apr_pool_t;
use apr::apr_size_t;
use apr::apr_status_t;
use apr::apr_time_t;

use apr::apr_int32_t;
use apr::apr_uint32_t;

use apr::apr_fileperms_t;
use apr::apr_proc_t;
use apr::apr_uint64_t;

use apr::apr_dir_t;

use apr::hash::apr_hash_t;
use apr::tables::apr_array_header_t;

use apr::apr_getopt_option_t;

use apr::apr_exit_why_e;
use apr::apr_seek_where_t;
