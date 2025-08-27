#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Entry,
    Wc,
    Regular,
}

impl From<subversion_sys::svn_prop_kind_t> for Kind {
    fn from(kind: subversion_sys::svn_prop_kind_t) -> Self {
        match kind {
            subversion_sys::svn_prop_kind_svn_prop_entry_kind => Kind::Entry,
            subversion_sys::svn_prop_kind_svn_prop_wc_kind => Kind::Wc,
            subversion_sys::svn_prop_kind_svn_prop_regular_kind => Kind::Regular,
            _ => panic!("Unknown property kind"),
        }
    }
}

pub fn kind(name: &str) -> Kind {
    let name = std::ffi::CString::new(name).unwrap();
    unsafe { subversion_sys::svn_property_kind2(name.as_ptr()) }.into()
}

pub fn is_svn_prop(name: &str) -> bool {
    let name = std::ffi::CString::new(name).unwrap();
    unsafe { subversion_sys::svn_prop_is_svn_prop(name.as_ptr()) != 0 }
}

pub fn is_boolean(name: &str) -> bool {
    let name = std::ffi::CString::new(name).unwrap();
    unsafe { subversion_sys::svn_prop_is_boolean(name.as_ptr()) != 0 }
}

pub fn is_known_svn_rev_prop(name: &str) -> bool {
    let name = std::ffi::CString::new(name).unwrap();
    unsafe { subversion_sys::svn_prop_is_known_svn_rev_prop(name.as_ptr()) != 0 }
}

pub fn is_known_svn_node_prop(name: &str) -> bool {
    let name = std::ffi::CString::new(name).unwrap();
    unsafe { subversion_sys::svn_prop_is_known_svn_node_prop(name.as_ptr()) != 0 }
}

pub fn is_known_svn_file_prop(name: &str) -> bool {
    let name = std::ffi::CString::new(name).unwrap();
    unsafe { subversion_sys::svn_prop_is_known_svn_file_prop(name.as_ptr()) != 0 }
}

pub fn is_known_svn_dir_prop(name: &str) -> bool {
    let name = std::ffi::CString::new(name).unwrap();
    unsafe { subversion_sys::svn_prop_is_known_svn_dir_prop(name.as_ptr()) != 0 }
}

pub fn needs_translation(name: &str) -> bool {
    let name = std::ffi::CString::new(name).unwrap();
    unsafe { subversion_sys::svn_prop_needs_translation(name.as_ptr()) != 0 }
}

pub fn name_is_valid(name: &str) -> bool {
    let name = std::ffi::CString::new(name).unwrap();
    unsafe { subversion_sys::svn_prop_name_is_valid(name.as_ptr()) != 0 }
}
