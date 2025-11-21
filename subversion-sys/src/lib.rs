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
