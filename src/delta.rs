use crate::Revnum;
use apr::pool::Pool;

pub fn version() -> crate::Version {
    crate::Version(unsafe { crate::generated::svn_delta_version() })
}

pub struct WrapEditor(
    pub(crate) *const crate::generated::svn_delta_editor_t,
    pub(crate) apr::pool::PooledPtr<std::ffi::c_void>,
);

impl Editor for WrapEditor {
    fn set_target_revision(&mut self, revision: Revnum) -> Result<(), crate::Error> {
        let mut scratch_pool = Pool::new();
        let err = unsafe {
            ((*self.0).set_target_revision.unwrap())(
                self.1.as_mut_ptr(),
                revision.into(),
                scratch_pool.as_mut_ptr(),
            )
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }

    fn open_root<'b>(
        &'b mut self,
        base_revision: Revnum,
    ) -> Result<Box<dyn DirectoryEditor + 'b>, crate::Error> {
        let editor = apr::pool::PooledPtr::initialize(|p| unsafe {
            let mut baton = std::ptr::null_mut();
            let err = ((*self.0).open_root.unwrap())(
                self.1.as_mut_ptr(),
                base_revision.into(),
                p.as_mut_ptr(),
                &mut baton,
            );
            crate::Error::from_raw(err)?;
            Ok::<_, crate::Error>(baton)
        })?;
        Ok(Box::new(WrapDirectoryEditor(&self.0, editor)))
    }

    fn close(&mut self) -> Result<(), crate::Error> {
        let mut scratch_pool = Pool::new();
        let err = unsafe {
            ((*self.0).close_edit.unwrap())(self.1.as_mut_ptr(), scratch_pool.as_mut_ptr())
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }

    fn abort(&mut self) -> Result<(), crate::Error> {
        let mut scratch_pool = Pool::new();
        let err = unsafe {
            ((*self.0).abort_edit.unwrap())(self.1.as_mut_ptr(), scratch_pool.as_mut_ptr())
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }
}

pub struct WrapDirectoryEditor<'a>(
    pub(crate) &'a *const crate::generated::svn_delta_editor_t,
    pub(crate) apr::pool::PooledPtr<std::ffi::c_void>,
);

impl<'a> DirectoryEditor for WrapDirectoryEditor<'a> {
    fn delete_entry(&mut self, path: &str, revision: Option<Revnum>) -> Result<(), crate::Error> {
        let mut scratch_pool = self.1.pool().subpool();
        let err = unsafe {
            ((*(*self.0)).delete_entry.unwrap())(
                path.as_ptr() as *const i8,
                revision.map_or(-1, |r| r.0),
                self.1.as_mut_ptr(),
                scratch_pool.as_mut_ptr(),
            )
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }

    fn add_directory<'b>(
        &'b mut self,
        path: &str,
        copyfrom: Option<(&str, Revnum)>,
    ) -> Result<Box<dyn DirectoryEditor + 'b>, crate::Error> {
        let editor = apr::pool::PooledPtr::initialize(|p| unsafe {
            let copyfrom_path = copyfrom.map(|(p, _)| p);
            let copyfrom_rev = copyfrom.map(|(_, r)| r.0).unwrap_or(-1);
            let mut baton = std::ptr::null_mut();
            let err = ((*(*self.0)).add_directory.unwrap())(
                path.as_ptr() as *const i8,
                self.1.as_mut_ptr(),
                if let Some(copyfrom_path) = copyfrom_path {
                    copyfrom_path.as_ptr() as *const i8
                } else {
                    std::ptr::null()
                },
                copyfrom_rev.into(),
                p.as_mut_ptr(),
                &mut baton,
            );
            crate::Error::from_raw(err)?;
            Ok::<_, crate::Error>(baton)
        })?;
        Ok(Box::new(WrapDirectoryEditor(self.0, editor)))
    }

    fn open_directory<'b>(
        &'b mut self,
        path: &str,
        base_revision: Option<Revnum>,
    ) -> Result<Box<dyn DirectoryEditor + 'b>, crate::Error> {
        let editor = apr::pool::PooledPtr::initialize(|p| unsafe {
            let mut baton = std::ptr::null_mut();
            let err = ((*(*self.0)).open_directory.unwrap())(
                path.as_ptr() as *const i8,
                self.1.as_mut_ptr(),
                base_revision.map_or(-1, |r| r.0),
                p.as_mut_ptr(),
                &mut baton,
            );
            crate::Error::from_raw(err)?;
            Ok::<_, crate::Error>(baton)
        })?;
        Ok(Box::new(WrapDirectoryEditor(self.0, editor)))
    }

    fn change_prop(&mut self, name: &str, value: &[u8]) -> Result<(), crate::Error> {
        let mut scratch_pool = self.1.pool().subpool();
        let value: crate::string::String = value.into();
        let err = unsafe {
            ((*(*self.0)).change_dir_prop.unwrap())(
                self.1.as_mut_ptr(),
                name.as_ptr() as *const i8,
                value.as_ptr(),
                scratch_pool.as_mut_ptr(),
            )
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }

    fn close(&mut self) -> Result<(), crate::Error> {
        let mut scratch_pool = self.1.pool().subpool();
        let err = unsafe {
            ((*(*self.0)).close_directory.unwrap())(self.1.as_mut_ptr(), scratch_pool.as_mut_ptr())
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }

    fn absent_directory(&mut self, path: &str) -> Result<(), crate::Error> {
        let mut scratch_pool = self.1.pool().subpool();
        let err = unsafe {
            ((*(*self.0)).absent_directory.unwrap())(
                path.as_ptr() as *const i8,
                self.1.as_mut_ptr(),
                scratch_pool.as_mut_ptr(),
            )
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }

    fn add_file<'b>(
        &'b mut self,
        path: &str,
        copyfrom: Option<(&str, Revnum)>,
    ) -> Result<Box<dyn FileEditor + 'b>, crate::Error> {
        let editor = apr::pool::PooledPtr::initialize(|p| unsafe {
            let copyfrom_path = copyfrom.map(|(p, _)| p);
            let copyfrom_rev = copyfrom.map(|(_, r)| r.0).unwrap_or(-1);
            let mut baton = std::ptr::null_mut();
            let err = ((*(*self.0)).add_file.unwrap())(
                path.as_ptr() as *const i8,
                self.1.as_mut_ptr(),
                if let Some(copyfrom_path) = copyfrom_path {
                    copyfrom_path.as_ptr() as *const i8
                } else {
                    std::ptr::null()
                },
                copyfrom_rev.into(),
                p.as_mut_ptr(),
                &mut baton,
            );
            crate::Error::from_raw(err)?;
            Ok::<_, crate::Error>(baton)
        })?;
        Ok(Box::new(WrapFileEditor(self.0, editor)))
    }

    fn open_file<'b>(
        &'b mut self,
        path: &str,
        base_revision: Option<Revnum>,
    ) -> Result<Box<dyn FileEditor + 'b>, crate::Error> {
        let editor = apr::pool::PooledPtr::initialize(|p| unsafe {
            let mut baton = std::ptr::null_mut();
            let err = ((*(*self.0)).open_file.unwrap())(
                path.as_ptr() as *const i8,
                self.1.as_mut_ptr(),
                base_revision.map(|r| r.into()).unwrap_or(0),
                p.as_mut_ptr(),
                &mut baton,
            );
            crate::Error::from_raw(err)?;
            Ok::<_, crate::Error>(baton)
        })?;
        Ok(Box::new(WrapFileEditor(self.0, editor)))
    }

    fn absent_file(&mut self, path: &str) -> Result<(), crate::Error> {
        let mut scratch_pool = self.1.pool().subpool();
        let err = unsafe {
            ((*(*self.0)).absent_file.unwrap())(
                path.as_ptr() as *const i8,
                self.1.as_mut_ptr(),
                scratch_pool.as_mut_ptr(),
            )
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }
}

pub struct WrapFileEditor<'a>(
    &'a *const crate::generated::svn_delta_editor_t,
    apr::pool::PooledPtr<std::ffi::c_void>,
);

#[allow(dead_code)]
pub struct WrapTxdeltaWindowHandler(
    apr::pool::PooledPtr<crate::generated::svn_txdelta_window_handler_t>,
    apr::pool::PooledPtr<std::ffi::c_void>,
);

impl<'a> FileEditor for WrapFileEditor<'a> {
    fn apply_textdelta(
        &mut self,
        base_checksum: Option<&str>,
    ) -> Result<Box<dyn for<'b> Fn(&'b mut TxDeltaWindow) -> Result<(), crate::Error>>, crate::Error>
    {
        let mut pool = self.1.pool().subpool();
        let mut handler = None;
        let mut baton = std::ptr::null_mut();
        let err = unsafe {
            ((*(*self.0)).apply_textdelta.unwrap())(
                self.1.as_mut_ptr(),
                if let Some(base_checksum) = base_checksum {
                    base_checksum.as_ptr() as *const i8
                } else {
                    std::ptr::null()
                },
                pool.as_mut_ptr(),
                &mut handler,
                &mut baton,
            )
        };
        crate::Error::from_raw(err)?;
        let apply = move |window: &mut TxDeltaWindow| -> Result<(), crate::Error> {
            let err = unsafe { (handler.unwrap())(window.as_mut_ptr(), baton) };
            crate::Error::from_raw(err)?;
            Ok(())
        };
        Ok(Box::new(apply))
    }

    fn change_prop(&mut self, name: &str, value: &[u8]) -> Result<(), crate::Error> {
        let mut scratch_pool = self.1.pool().subpool();
        let value: crate::string::String = value.into();
        let err = unsafe {
            ((*(*self.0)).change_file_prop.unwrap())(
                self.1.as_mut_ptr(),
                name.as_ptr() as *const i8,
                value.as_ptr(),
                scratch_pool.as_mut_ptr(),
            )
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }

    fn close(&mut self, text_checksum: Option<&str>) -> Result<(), crate::Error> {
        let mut pool = self.1.pool().subpool();
        let err = unsafe {
            ((*(*self.0)).close_file.unwrap())(
                self.1.as_mut_ptr(),
                if let Some(text_checksum) = text_checksum {
                    text_checksum.as_ptr() as *const i8
                } else {
                    std::ptr::null()
                },
                pool.as_mut_ptr(),
            )
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }
}

pub trait Editor {
    fn set_target_revision(&mut self, revision: Revnum) -> Result<(), crate::Error>;

    fn open_root<'a>(
        &'a mut self,
        base_revision: Revnum,
    ) -> Result<Box<dyn DirectoryEditor + 'a>, crate::Error>;

    fn close(&mut self) -> Result<(), crate::Error>;

    fn abort(&mut self) -> Result<(), crate::Error>;
}

pub trait DirectoryEditor {
    fn delete_entry(&mut self, path: &str, revision: Option<Revnum>) -> Result<(), crate::Error>;

    fn add_directory<'a>(
        &'a mut self,
        path: &str,
        copyfrom: Option<(&str, Revnum)>,
    ) -> Result<Box<dyn DirectoryEditor + 'a>, crate::Error>;

    fn open_directory<'a>(
        &'a mut self,
        path: &str,
        base_revision: Option<Revnum>,
    ) -> Result<Box<dyn DirectoryEditor + 'a>, crate::Error>;

    fn change_prop(&mut self, name: &str, value: &[u8]) -> Result<(), crate::Error>;

    fn close(&mut self) -> Result<(), crate::Error>;

    fn absent_directory(&mut self, path: &str) -> Result<(), crate::Error>;

    fn add_file<'a>(
        &'a mut self,
        path: &str,
        copyfrom: Option<(&str, Revnum)>,
    ) -> Result<Box<dyn FileEditor + 'a>, crate::Error>;

    fn open_file<'a>(
        &'a mut self,
        path: &str,
        base_revision: Option<Revnum>,
    ) -> Result<Box<dyn FileEditor + 'a>, crate::Error>;

    fn absent_file(&mut self, path: &str) -> Result<(), crate::Error>;
}

pub fn noop_window_handler(window: &mut TxDeltaWindow) -> Result<(), crate::Error> {
    let err = unsafe {
        crate::generated::svn_delta_noop_window_handler(window.as_mut_ptr(), std::ptr::null_mut())
    };
    crate::Error::from_raw(err)?;
    Ok(())
}

pub trait FileEditor {
    fn apply_textdelta(
        &mut self,
        base_checksum: Option<&str>,
    ) -> Result<Box<dyn for<'a> Fn(&'a mut TxDeltaWindow) -> Result<(), crate::Error>>, crate::Error>;

    // TODO: fn apply_textdelta_stream(&mut self, base_checksum: Option<&str>) -> Result<&dyn TextDelta, crate::Error>;

    fn change_prop(&mut self, name: &str, value: &[u8]) -> Result<(), crate::Error>;

    fn close(&mut self, text_checksum: Option<&str>) -> Result<(), crate::Error>;
}

pub struct TxDeltaWindow(apr::pool::PooledPtr<crate::generated::svn_txdelta_window_t>);

impl TxDeltaWindow {
    pub fn as_ptr(&self) -> *const crate::generated::svn_txdelta_window_t {
        self.0.as_ptr()
    }

    pub fn as_mut_ptr(&mut self) -> *mut crate::generated::svn_txdelta_window_t {
        self.0.as_mut_ptr()
    }

    pub fn new() -> Self {
        Self(apr::pool::PooledPtr::initialize(|p| Ok::<_, crate::Error>(p.calloc())).unwrap())
    }

    pub fn sview_len(&self) -> apr::apr_size_t {
        self.0.sview_len
    }

    pub fn tview_len(&self) -> apr::apr_size_t {
        self.0.tview_len
    }

    pub fn sview_offset(&self) -> crate::FileSize {
        self.0.sview_offset
    }

    pub fn compose(a: &Self, b: &Self) -> Self {
        Self(
            apr::pool::PooledPtr::initialize(|pool| unsafe {
                Ok::<_, crate::Error>(crate::generated::svn_txdelta_compose_windows(
                    a.0.as_ptr(),
                    b.0.as_ptr(),
                    pool.as_mut_ptr(),
                ))
            })
            .unwrap(),
        )
    }

    pub fn apply_instructions(
        &mut self,
        source: &mut [u8],
        target: &mut Vec<u8>,
    ) -> Result<(), crate::Error> {
        unsafe {
            target.resize(self.tview_len(), 0);
            let mut tlen = target.len() as apr::apr_size_t;
            crate::generated::svn_txdelta_apply_instructions(
                self.0.as_mut_ptr(),
                source.as_ptr() as *mut i8,
                target.as_mut_ptr() as *mut i8,
                &mut tlen,
            );
        }
        Ok(())
    }
}

extern "C" fn wrap_editor_open_root(
    edit_baton: *mut std::ffi::c_void,
    base_revision: crate::generated::svn_revnum_t,
    _pool: *mut apr::apr_pool_t,
    root_baton: *mut *mut std::ffi::c_void,
) -> *mut crate::generated::svn_error_t {
    let editor: &mut dyn Editor = unsafe { *(edit_baton as *mut &mut dyn Editor) };
    match editor.open_root(Revnum::from_raw(base_revision).unwrap()) {
        Ok(mut root) => {
            unsafe {
                *root_baton = root.as_mut() as *mut dyn DirectoryEditor as *mut std::ffi::c_void
            };
            std::ptr::null_mut()
        }
        Err(err) => unsafe { err.into_raw() },
    }
}

extern "C" fn wrap_editor_delete_entry(
    path: *const std::ffi::c_char,
    revision: crate::generated::svn_revnum_t,
    parent_baton: *mut std::ffi::c_void,
    _pool: *mut apr::apr_pool_t,
) -> *mut crate::generated::svn_error_t {
    let path = unsafe { std::ffi::CStr::from_ptr(path) };
    let parent: &mut dyn DirectoryEditor =
        unsafe { *(parent_baton as *mut &mut dyn DirectoryEditor) };
    match parent.delete_entry(path.to_str().unwrap(), Revnum::from_raw(revision)) {
        Ok(()) => std::ptr::null_mut(),
        Err(err) => unsafe { err.into_raw() },
    }
}

extern "C" fn wrap_editor_add_directory(
    path: *const std::ffi::c_char,
    parent_baton: *mut std::ffi::c_void,
    copyfrom_path: *const std::ffi::c_char,
    copyfrom_revision: crate::generated::svn_revnum_t,
    _pool: *mut apr::apr_pool_t,
    child_baton: *mut *mut std::ffi::c_void,
) -> *mut crate::generated::svn_error_t {
    let path = unsafe { std::ffi::CStr::from_ptr(path) };
    let copyfrom_path = if copyfrom_path.is_null() {
        None
    } else {
        Some(unsafe { std::ffi::CStr::from_ptr(copyfrom_path) })
    };
    let parent: &mut dyn DirectoryEditor =
        unsafe { *(parent_baton as *mut &mut dyn DirectoryEditor) };
    let copyfrom = if let (Some(copyfrom_path), Some(copyfrom_revision)) =
        (copyfrom_path, Revnum::from_raw(copyfrom_revision))
    {
        Some((copyfrom_path.to_str().unwrap(), copyfrom_revision))
    } else {
        None
    };
    match parent.add_directory(path.to_str().unwrap(), copyfrom) {
        Ok(mut child) => {
            unsafe {
                *child_baton = child.as_mut() as *mut dyn DirectoryEditor as *mut std::ffi::c_void
            };
            std::ptr::null_mut()
        }
        Err(err) => unsafe { err.into_raw() },
    }
}

extern "C" fn wrap_editor_open_directory(
    path: *const std::ffi::c_char,
    parent_baton: *mut std::ffi::c_void,
    base_revision: crate::generated::svn_revnum_t,
    _pool: *mut apr::apr_pool_t,
    child_baton: *mut *mut std::ffi::c_void,
) -> *mut crate::generated::svn_error_t {
    let path = unsafe { std::ffi::CStr::from_ptr(path) };
    let parent: &mut dyn DirectoryEditor =
        unsafe { *(parent_baton as *mut &mut dyn DirectoryEditor) };
    match parent.open_directory(path.to_str().unwrap(), Revnum::from_raw(base_revision)) {
        Ok(mut child) => {
            unsafe {
                *child_baton = child.as_mut() as *mut dyn DirectoryEditor as *mut std::ffi::c_void
            };
            std::ptr::null_mut()
        }
        Err(err) => unsafe { err.into_raw() },
    }
}

extern "C" fn wrap_editor_change_dir_prop(
    baton: *mut std::ffi::c_void,
    name: *const std::ffi::c_char,
    value: *const crate::generated::svn_string_t,
    _pool: *mut apr::apr_pool_t,
) -> *mut crate::generated::svn_error_t {
    let name = unsafe { std::ffi::CStr::from_ptr(name) };
    let value =
        unsafe { std::slice::from_raw_parts((*value).data as *const u8, (*value).len as usize) };
    let editor: &mut dyn DirectoryEditor = unsafe { *(baton as *mut &mut dyn DirectoryEditor) };
    match editor.change_prop(name.to_str().unwrap(), value) {
        Ok(()) => std::ptr::null_mut(),
        Err(err) => unsafe { err.into_raw() },
    }
}

extern "C" fn wrap_editor_close_directory(
    baton: *mut std::ffi::c_void,
    _pool: *mut apr::apr_pool_t,
) -> *mut crate::generated::svn_error_t {
    let editor: &mut dyn DirectoryEditor = unsafe { *(baton as *mut &mut dyn DirectoryEditor) };
    match editor.close() {
        Ok(()) => std::ptr::null_mut(),
        Err(err) => unsafe { err.into_raw() },
    }
}

extern "C" fn wrap_editor_absent_directory(
    path: *const std::ffi::c_char,
    parent_baton: *mut std::ffi::c_void,
    _pool: *mut apr::apr_pool_t,
) -> *mut crate::generated::svn_error_t {
    let path = unsafe { std::ffi::CStr::from_ptr(path) };
    let parent: &mut dyn DirectoryEditor =
        unsafe { *(parent_baton as *mut &mut dyn DirectoryEditor) };
    match parent.absent_directory(path.to_str().unwrap()) {
        Ok(()) => std::ptr::null_mut(),
        Err(err) => unsafe { err.into_raw() },
    }
}

extern "C" fn wrap_editor_add_file(
    path: *const std::ffi::c_char,
    parent_baton: *mut std::ffi::c_void,
    copyfrom_path: *const std::ffi::c_char,
    copyfrom_revision: crate::generated::svn_revnum_t,
    _pool: *mut apr::apr_pool_t,
    file_baton: *mut *mut std::ffi::c_void,
) -> *mut crate::generated::svn_error_t {
    let path = unsafe { std::ffi::CStr::from_ptr(path) };
    let copyfrom_path = if copyfrom_path.is_null() {
        None
    } else {
        Some(unsafe { std::ffi::CStr::from_ptr(copyfrom_path) })
    };
    let parent: &mut dyn DirectoryEditor =
        unsafe { *(parent_baton as *mut &mut dyn DirectoryEditor) };
    let copyfrom = if let (Some(copyfrom_path), Some(copyfrom_revision)) =
        (copyfrom_path, Revnum::from_raw(copyfrom_revision))
    {
        Some((copyfrom_path.to_str().unwrap(), copyfrom_revision))
    } else {
        None
    };
    match parent.add_file(path.to_str().unwrap(), copyfrom) {
        Ok(mut file) => {
            unsafe { *file_baton = file.as_mut() as *mut dyn FileEditor as *mut std::ffi::c_void };
            std::ptr::null_mut()
        }
        Err(err) => unsafe { err.into_raw() },
    }
}

extern "C" fn wrap_editor_open_file(
    path: *const std::ffi::c_char,
    parent_baton: *mut std::ffi::c_void,
    base_revision: crate::generated::svn_revnum_t,
    _pool: *mut apr::apr_pool_t,
    file_baton: *mut *mut std::ffi::c_void,
) -> *mut crate::generated::svn_error_t {
    let path = unsafe { std::ffi::CStr::from_ptr(path) };
    let parent: &mut dyn DirectoryEditor =
        unsafe { *(parent_baton as *mut &mut dyn DirectoryEditor) };
    match parent.open_file(path.to_str().unwrap(), Revnum::from_raw(base_revision)) {
        Ok(mut file) => {
            unsafe { *file_baton = file.as_mut() as *mut dyn FileEditor as *mut std::ffi::c_void };
            std::ptr::null_mut()
        }
        Err(err) => unsafe { err.into_raw() },
    }
}

extern "C" fn wrap_editor_apply_textdelta(
    file_baton: *mut std::ffi::c_void,
    base_checksum: *const std::ffi::c_char,
    _result_pool: *mut apr::apr_pool_t,
    _handler: *mut crate::generated::svn_txdelta_window_handler_t,
    _baton: *mut *mut std::ffi::c_void,
) -> *mut crate::generated::svn_error_t {
    let base_checksum = if base_checksum.is_null() {
        None
    } else {
        Some(unsafe { std::ffi::CStr::from_ptr(base_checksum) })
    };
    let file: &mut dyn FileEditor = unsafe { *(file_baton as *mut &mut dyn FileEditor) };
    match file.apply_textdelta(base_checksum.map(|c| c.to_str().unwrap())) {
        Ok(_apply) => {
            todo!();
        }
        Err(err) => unsafe { err.into_raw() },
    }
}

extern "C" fn wrap_editor_change_file_prop(
    baton: *mut std::ffi::c_void,
    name: *const std::ffi::c_char,
    value: *const crate::generated::svn_string_t,
    _pool: *mut apr::apr_pool_t,
) -> *mut crate::generated::svn_error_t {
    let name = unsafe { std::ffi::CStr::from_ptr(name) };
    let value =
        unsafe { std::slice::from_raw_parts((*value).data as *const u8, (*value).len as usize) };
    let editor: &mut dyn FileEditor = unsafe { *(baton as *mut &mut dyn FileEditor) };
    match editor.change_prop(name.to_str().unwrap(), value) {
        Ok(()) => std::ptr::null_mut(),
        Err(err) => unsafe { err.into_raw() },
    }
}

extern "C" fn wrap_editor_close_file(
    baton: *mut std::ffi::c_void,
    text_checksum: *const std::ffi::c_char,
    _pool: *mut apr::apr_pool_t,
) -> *mut crate::generated::svn_error_t {
    let text_checksum = if text_checksum.is_null() {
        None
    } else {
        Some(unsafe { std::ffi::CStr::from_ptr(text_checksum) })
    };
    let editor: &mut dyn FileEditor = unsafe { *(baton as *mut &mut dyn FileEditor) };
    match editor.close(text_checksum.map(|c| c.to_str().unwrap())) {
        Ok(()) => std::ptr::null_mut(),
        Err(err) => unsafe { err.into_raw() },
    }
}

extern "C" fn wrap_editor_absent_file(
    text_checksum: *const std::ffi::c_char,
    file_baton: *mut std::ffi::c_void,
    _pooll: *mut apr::apr_pool_t,
) -> *mut crate::generated::svn_error_t {
    let text_checksum = if text_checksum.is_null() {
        None
    } else {
        Some(unsafe { std::ffi::CStr::from_ptr(text_checksum) })
    };
    let file: &mut dyn FileEditor = unsafe { *(file_baton as *mut &mut dyn FileEditor) };
    match file.close(text_checksum.map(|c| c.to_str().unwrap())) {
        Ok(()) => std::ptr::null_mut(),
        Err(err) => unsafe { err.into_raw() },
    }
}

extern "C" fn wrap_editor_close_edit(
    edit_baton: *mut std::ffi::c_void,
    _pool: *mut apr::apr_pool_t,
) -> *mut crate::generated::svn_error_t {
    let editor: &mut dyn Editor = unsafe { *(edit_baton as *mut &mut dyn Editor) };
    match editor.close() {
        Ok(()) => std::ptr::null_mut(),
        Err(err) => unsafe { err.into_raw() },
    }
}

extern "C" fn wrap_editor_abort_edit(
    edit_baton: *mut std::ffi::c_void,
    _pool: *mut apr::apr_pool_t,
) -> *mut crate::generated::svn_error_t {
    let editor: &mut dyn Editor = unsafe { *(edit_baton as *mut &mut dyn Editor) };
    match editor.abort() {
        Ok(()) => std::ptr::null_mut(),
        Err(err) => unsafe { err.into_raw() },
    }
}

extern "C" fn wrap_editor_set_target_revision(
    edit_baton: *mut std::ffi::c_void,
    revision: crate::generated::svn_revnum_t,
    _pool: *mut apr::apr_pool_t,
) -> *mut crate::generated::svn_error_t {
    let editor: &mut dyn Editor = unsafe { *(edit_baton as *mut &mut dyn Editor) };
    match editor.set_target_revision(Revnum::from_raw(revision).unwrap()) {
        Ok(()) => std::ptr::null_mut(),
        Err(err) => unsafe { err.into_raw() },
    }
}

#[no_mangle]
pub(crate) static WRAP_EDITOR: crate::generated::svn_delta_editor_t =
    crate::generated::svn_delta_editor_t {
        open_root: Some(wrap_editor_open_root),
        delete_entry: Some(wrap_editor_delete_entry),
        add_directory: Some(wrap_editor_add_directory),
        open_directory: Some(wrap_editor_open_directory),
        change_dir_prop: Some(wrap_editor_change_dir_prop),
        close_directory: Some(wrap_editor_close_directory),
        absent_directory: Some(wrap_editor_absent_directory),
        add_file: Some(wrap_editor_add_file),
        open_file: Some(wrap_editor_open_file),
        apply_textdelta: Some(wrap_editor_apply_textdelta),
        change_file_prop: Some(wrap_editor_change_file_prop),
        close_file: Some(wrap_editor_close_file),
        absent_file: Some(wrap_editor_absent_file),
        close_edit: Some(wrap_editor_close_edit),
        abort_edit: Some(wrap_editor_abort_edit),
        set_target_revision: Some(wrap_editor_set_target_revision),
        apply_textdelta_stream: None, // TODO
    };

#[cfg(test)]
mod tests {
    #[test]
    fn test_version() {
        assert_eq!(super::version().major(), 1);
    }
}
