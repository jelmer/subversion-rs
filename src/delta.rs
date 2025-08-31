use crate::Revnum;
use apr::pool::Pool;
use std::marker::PhantomData;

// Helper structure to pass fat pointers through FFI
#[repr(C)]
pub struct EditorBaton {
    pub data_ptr: *mut std::ffi::c_void,
    pub vtable_ptr: *mut std::ffi::c_void,
}

pub fn version() -> crate::Version {
    crate::Version(unsafe { subversion_sys::svn_delta_version() })
}

pub struct WrapEditor<'pool> {
    pub(crate) editor: *const subversion_sys::svn_delta_editor_t,
    pub(crate) baton: *mut std::ffi::c_void,
    pub(crate) _pool: std::marker::PhantomData<&'pool apr::Pool>,
}
unsafe impl Send for WrapEditor<'_> {}

impl<'pool> Editor for WrapEditor<'pool> {
    fn set_target_revision(&mut self, revision: Option<Revnum>) -> Result<(), crate::Error> {
        let scratch_pool = Pool::new();
        let err = unsafe {
            ((*self.editor).set_target_revision.unwrap())(
                self.baton,
                revision.map_or(-1, |r| r.into()),
                scratch_pool.as_mut_ptr(),
            )
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }

    fn open_root<'b>(
        &'b mut self,
        base_revision: Option<Revnum>,
    ) -> Result<Box<dyn DirectoryEditor + 'b>, crate::Error> {
        let mut baton = std::ptr::null_mut();
        let pool = Pool::new();
        let err = unsafe {
            ((*self.editor).open_root.unwrap())(
                self.baton,
                base_revision.map_or(-1, |r| r.into()),
                pool.as_mut_ptr(),
                &mut baton,
            )
        };
        crate::Error::from_raw(err)?;
        Ok(Box::new(WrapDirectoryEditor {
            editor: &self.editor,
            baton,
            _pool: std::marker::PhantomData,
        }))
    }

    fn close(&mut self) -> Result<(), crate::Error> {
        let scratch_pool = Pool::new();
        let err =
            unsafe { ((*self.editor).close_edit.unwrap())(self.baton, scratch_pool.as_mut_ptr()) };
        crate::Error::from_raw(err)?;
        Ok(())
    }

    fn abort(&mut self) -> Result<(), crate::Error> {
        let scratch_pool = Pool::new();
        let err =
            unsafe { ((*self.editor).abort_edit.unwrap())(self.baton, scratch_pool.as_mut_ptr()) };
        crate::Error::from_raw(err)?;
        Ok(())
    }
}

pub struct WrapDirectoryEditor<'a, 'pool> {
    pub(crate) editor: &'a *const subversion_sys::svn_delta_editor_t,
    pub(crate) baton: *mut std::ffi::c_void,
    _pool: std::marker::PhantomData<&'pool apr::Pool>,
}

impl<'a, 'pool> DirectoryEditor for WrapDirectoryEditor<'a, 'pool> {
    fn delete_entry(&mut self, path: &str, revision: Option<Revnum>) -> Result<(), crate::Error> {
        let scratch_pool = Pool::new();
        let err = unsafe {
            ((*(*self.editor)).delete_entry.unwrap())(
                path.as_ptr() as *const i8,
                revision.map_or(-1, |r| r.into()),
                self.baton,
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
        let pool = apr::Pool::new();
        let copyfrom_path = copyfrom.map(|(p, _)| p);
        let copyfrom_rev = copyfrom.map(|(_, r)| r.0).unwrap_or(-1);
        let mut baton = std::ptr::null_mut();
        unsafe {
            let err = ((*(*self.editor)).add_directory.unwrap())(
                path.as_ptr() as *const i8,
                self.baton,
                if let Some(copyfrom_path) = copyfrom_path {
                    copyfrom_path.as_ptr() as *const i8
                } else {
                    std::ptr::null()
                },
                copyfrom_rev,
                pool.as_mut_ptr(),
                &mut baton,
            );
            crate::Error::from_raw(err)?;
        }
        Ok(Box::new(WrapDirectoryEditor {
            editor: self.editor,
            baton,
            _pool: std::marker::PhantomData,
        }))
    }

    fn open_directory<'b>(
        &'b mut self,
        path: &str,
        base_revision: Option<Revnum>,
    ) -> Result<Box<dyn DirectoryEditor + 'b>, crate::Error> {
        let pool = apr::Pool::new();
        let mut baton = std::ptr::null_mut();
        unsafe {
            let err = ((*(*self.editor)).open_directory.unwrap())(
                path.as_ptr() as *const i8,
                self.baton,
                base_revision.map_or(-1, |r| r.0),
                pool.as_mut_ptr(),
                &mut baton,
            );
            crate::Error::from_raw(err)?;
        }
        Ok(Box::new(WrapDirectoryEditor {
            editor: self.editor,
            baton,
            _pool: std::marker::PhantomData,
        }))
    }

    fn change_prop(&mut self, name: &str, value: &[u8]) -> Result<(), crate::Error> {
        let scratch_pool = apr::pool::Pool::new();
        let value: crate::string::String = value.into();
        let err = unsafe {
            ((*(*self.editor)).change_dir_prop.unwrap())(
                self.baton,
                name.as_ptr() as *const i8,
                value.as_ptr(),
                scratch_pool.as_mut_ptr(),
            )
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }

    fn close(&mut self) -> Result<(), crate::Error> {
        let scratch_pool = apr::pool::Pool::new();
        let err = unsafe {
            ((*(*self.editor)).close_directory.unwrap())(self.baton, scratch_pool.as_mut_ptr())
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }

    fn absent_directory(&mut self, path: &str) -> Result<(), crate::Error> {
        let scratch_pool = apr::pool::Pool::new();
        let err = unsafe {
            ((*(*self.editor)).absent_directory.unwrap())(
                path.as_ptr() as *const i8,
                self.baton,
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
        let pool = apr::Pool::new();
        let copyfrom_path = copyfrom.map(|(p, _)| p);
        let copyfrom_rev = copyfrom.map(|(_, r)| r.0).unwrap_or(-1);
        let mut baton = std::ptr::null_mut();
        unsafe {
            let err = ((*(*self.editor)).add_file.unwrap())(
                path.as_ptr() as *const i8,
                self.baton,
                if let Some(copyfrom_path) = copyfrom_path {
                    copyfrom_path.as_ptr() as *const i8
                } else {
                    std::ptr::null()
                },
                copyfrom_rev,
                pool.as_mut_ptr(),
                &mut baton,
            );
            crate::Error::from_raw(err)?;
        }
        Ok(Box::new(WrapFileEditor {
            editor: self.editor,
            baton,
            pool,
            _phantom: PhantomData,
        }))
    }

    fn open_file<'b>(
        &'b mut self,
        path: &str,
        base_revision: Option<Revnum>,
    ) -> Result<Box<dyn FileEditor + 'b>, crate::Error> {
        let pool = apr::Pool::new();
        let mut baton = std::ptr::null_mut();
        unsafe {
            let err = ((*(*self.editor)).open_file.unwrap())(
                path.as_ptr() as *const i8,
                self.baton,
                base_revision.map(|r| r.into()).unwrap_or(0),
                pool.as_mut_ptr(),
                &mut baton,
            );
            crate::Error::from_raw(err)?;
        }
        Ok(Box::new(WrapFileEditor {
            editor: self.editor,
            baton,
            pool,
            _phantom: PhantomData,
        }))
    }

    fn absent_file(&mut self, path: &str) -> Result<(), crate::Error> {
        let scratch_pool = apr::pool::Pool::new();
        let err = unsafe {
            ((*(*self.editor)).absent_file.unwrap())(
                path.as_ptr() as *const i8,
                self.baton,
                scratch_pool.as_mut_ptr(),
            )
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }
}

pub struct WrapFileEditor<'a> {
    editor: &'a *const subversion_sys::svn_delta_editor_t,
    baton: *mut std::ffi::c_void,
    pool: apr::Pool,
    _phantom: PhantomData<&'a ()>,
}

impl Drop for WrapFileEditor<'_> {
    fn drop(&mut self) {
        // Pool drop will clean up
    }
}

impl<'a> WrapFileEditor<'a> {
    /// Get a reference to the underlying pool
    pub fn pool(&self) -> &apr::Pool {
        &self.pool
    }
}

#[allow(dead_code)]
pub struct WrapTxdeltaWindowHandler {
    handler: *mut subversion_sys::svn_txdelta_window_handler_t,
    baton: *mut std::ffi::c_void,
    pool: apr::Pool,
    _phantom: PhantomData<*mut ()>,
}

impl Drop for WrapTxdeltaWindowHandler {
    fn drop(&mut self) {
        // Pool drop will clean up
    }
}

impl<'a> FileEditor for WrapFileEditor<'a> {
    fn apply_textdelta(
        &mut self,
        base_checksum: Option<&str>,
    ) -> Result<Box<dyn for<'b> Fn(&'b mut TxDeltaWindow) -> Result<(), crate::Error>>, crate::Error>
    {
        let pool = apr::pool::Pool::new();
        let mut handler = None;
        let mut baton = std::ptr::null_mut();
        let err = unsafe {
            ((*(*self.editor)).apply_textdelta.unwrap())(
                self.baton,
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
        let scratch_pool = apr::pool::Pool::new();
        let value: crate::string::String = value.into();
        let err = unsafe {
            ((*(*self.editor)).change_file_prop.unwrap())(
                self.baton,
                name.as_ptr() as *const i8,
                value.as_ptr(),
                scratch_pool.as_mut_ptr(),
            )
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }

    fn close(&mut self, text_checksum: Option<&str>) -> Result<(), crate::Error> {
        let pool = apr::pool::Pool::new();
        let err = unsafe {
            ((*(*self.editor)).close_file.unwrap())(
                self.baton,
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
    fn set_target_revision(&mut self, revision: Option<Revnum>) -> Result<(), crate::Error>;

    fn open_root<'a>(
        &'a mut self,
        base_revision: Option<Revnum>,
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
        subversion_sys::svn_delta_noop_window_handler(window.as_mut_ptr(), std::ptr::null_mut())
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

/// TxDelta window with RAII cleanup
pub struct TxDeltaWindow {
    ptr: *mut subversion_sys::svn_txdelta_window_t,
    pool: apr::Pool,
    _phantom: PhantomData<*mut ()>, // !Send + !Sync
}

impl Drop for TxDeltaWindow {
    fn drop(&mut self) {
        // Pool drop will clean up window
    }
}

impl Default for TxDeltaWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl TxDeltaWindow {
    /// Get a reference to the underlying pool
    pub fn pool(&self) -> &apr::Pool {
        &self.pool
    }

    /// Get the mutable raw pointer to the window (use with caution)
    pub fn as_mut_ptr(&mut self) -> *mut subversion_sys::svn_txdelta_window_t {
        self.ptr
    }
    pub fn as_ptr(&self) -> *const subversion_sys::svn_txdelta_window_t {
        self.ptr
    }

    pub fn new() -> Self {
        let pool = apr::Pool::new();
        let ptr = pool.calloc::<subversion_sys::svn_txdelta_window_t>();
        Self {
            ptr,
            pool,
            _phantom: PhantomData,
        }
    }

    pub fn sview_len(&self) -> apr_sys::apr_size_t {
        unsafe { (*self.ptr).sview_len }
    }

    pub fn tview_len(&self) -> apr_sys::apr_size_t {
        unsafe { (*self.ptr).tview_len }
    }

    pub fn sview_offset(&self) -> crate::FileSize {
        unsafe { (*self.ptr).sview_offset }
    }

    pub fn compose(a: &Self, b: &Self) -> Self {
        let pool = apr::Pool::new();
        let ptr =
            unsafe { subversion_sys::svn_txdelta_compose_windows(a.ptr, b.ptr, pool.as_mut_ptr()) };
        Self {
            ptr,
            pool,
            _phantom: PhantomData,
        }
    }

    pub fn apply_instructions(
        &mut self,
        source: &mut [u8],
        target: &mut Vec<u8>,
    ) -> Result<(), crate::Error> {
        unsafe {
            target.resize(self.tview_len(), 0);
            let mut tlen = target.len() as apr_sys::apr_size_t;
            subversion_sys::svn_txdelta_apply_instructions(
                self.ptr,
                source.as_ptr() as *mut i8,
                target.as_mut_ptr() as *mut i8,
                &mut tlen,
            );
        }
        Ok(())
    }

    pub fn dup(&self) -> Self {
        let pool = apr::Pool::new();
        let ptr = unsafe { subversion_sys::svn_txdelta_window_dup(self.ptr, pool.as_mut_ptr()) };
        Self {
            ptr,
            pool,
            _phantom: PhantomData,
        }
    }
}

extern "C" fn wrap_editor_open_root(
    edit_baton: *mut std::ffi::c_void,
    base_revision: subversion_sys::svn_revnum_t,
    _pool: *mut apr_sys::apr_pool_t,
    root_baton: *mut *mut std::ffi::c_void,
) -> *mut subversion_sys::svn_error_t {
    // Reconstruct the fat pointer from the baton
    let editor_ptr = unsafe { *(edit_baton as *mut *mut dyn Editor) };
    let editor = unsafe { &mut *editor_ptr };
    match editor.open_root(Revnum::from_raw(base_revision)) {
        Ok(root) => {
            // Leak the DirectoryEditor box - we'll reclaim it in close_directory
            unsafe {
                *root_baton = Box::into_raw(root) as *mut std::ffi::c_void;
            }
            std::ptr::null_mut()
        }
        Err(err) => unsafe { err.into_raw() },
    }
}

extern "C" fn wrap_editor_delete_entry(
    path: *const std::ffi::c_char,
    revision: subversion_sys::svn_revnum_t,
    parent_baton: *mut std::ffi::c_void,
    _pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let path = unsafe { std::ffi::CStr::from_ptr(path) };
    // The parent_baton is a Box<dyn DirectoryEditor> that was Box::into_raw'd
    let parent = unsafe { &mut **(parent_baton as *mut Box<dyn DirectoryEditor>) };
    match parent.delete_entry(path.to_str().unwrap(), Revnum::from_raw(revision)) {
        Ok(()) => std::ptr::null_mut(),
        Err(err) => unsafe { err.into_raw() },
    }
}

extern "C" fn wrap_editor_add_directory(
    path: *const std::ffi::c_char,
    parent_baton: *mut std::ffi::c_void,
    copyfrom_path: *const std::ffi::c_char,
    copyfrom_revision: subversion_sys::svn_revnum_t,
    _pool: *mut apr_sys::apr_pool_t,
    child_baton: *mut *mut std::ffi::c_void,
) -> *mut subversion_sys::svn_error_t {
    let path = unsafe { std::ffi::CStr::from_ptr(path) };
    let copyfrom_path = if copyfrom_path.is_null() {
        None
    } else {
        Some(unsafe { std::ffi::CStr::from_ptr(copyfrom_path) })
    };
    // The parent_baton is a Box<dyn DirectoryEditor> that was Box::into_raw'd
    let parent = unsafe { &mut **(parent_baton as *mut Box<dyn DirectoryEditor>) };
    let copyfrom = if let (Some(copyfrom_path), Some(copyfrom_revision)) =
        (copyfrom_path, Revnum::from_raw(copyfrom_revision))
    {
        Some((copyfrom_path.to_str().unwrap(), copyfrom_revision))
    } else {
        None
    };
    match parent.add_directory(path.to_str().unwrap(), copyfrom) {
        Ok(child) => {
            unsafe { *child_baton = Box::into_raw(child) as *mut std::ffi::c_void };
            std::ptr::null_mut()
        }
        Err(err) => unsafe { err.into_raw() },
    }
}

extern "C" fn wrap_editor_open_directory(
    path: *const std::ffi::c_char,
    parent_baton: *mut std::ffi::c_void,
    base_revision: subversion_sys::svn_revnum_t,
    _pool: *mut apr_sys::apr_pool_t,
    child_baton: *mut *mut std::ffi::c_void,
) -> *mut subversion_sys::svn_error_t {
    let path = unsafe { std::ffi::CStr::from_ptr(path) };
    // The parent_baton is a Box<dyn DirectoryEditor> that was Box::into_raw'd
    let parent = unsafe { &mut **(parent_baton as *mut Box<dyn DirectoryEditor>) };
    match parent.open_directory(path.to_str().unwrap(), Revnum::from_raw(base_revision)) {
        Ok(child) => {
            unsafe { *child_baton = Box::into_raw(child) as *mut std::ffi::c_void };
            std::ptr::null_mut()
        }
        Err(err) => unsafe { err.into_raw() },
    }
}

extern "C" fn wrap_editor_change_dir_prop(
    baton: *mut std::ffi::c_void,
    name: *const std::ffi::c_char,
    value: *const subversion_sys::svn_string_t,
    _pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let name = unsafe { std::ffi::CStr::from_ptr(name) };
    let value = unsafe { std::slice::from_raw_parts((*value).data as *const u8, (*value).len) };
    let editor = unsafe { &mut **(baton as *mut Box<dyn DirectoryEditor>) };
    match editor.change_prop(name.to_str().unwrap(), value) {
        Ok(()) => std::ptr::null_mut(),
        Err(err) => unsafe { err.into_raw() },
    }
}

extern "C" fn wrap_editor_close_directory(
    baton: *mut std::ffi::c_void,
    _pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let mut boxed_editor = unsafe { Box::from_raw(baton as *mut Box<dyn DirectoryEditor>) };
    match boxed_editor.close() {
        Ok(()) => {
            // Box is automatically dropped here, cleaning up memory
            std::ptr::null_mut()
        }
        Err(err) => unsafe { err.into_raw() },
    }
}

extern "C" fn wrap_editor_absent_directory(
    path: *const std::ffi::c_char,
    parent_baton: *mut std::ffi::c_void,
    _pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let path = unsafe { std::ffi::CStr::from_ptr(path) };
    // The parent_baton is a Box<dyn DirectoryEditor> that was Box::into_raw'd
    let parent = unsafe { &mut **(parent_baton as *mut Box<dyn DirectoryEditor>) };
    match parent.absent_directory(path.to_str().unwrap()) {
        Ok(()) => std::ptr::null_mut(),
        Err(err) => unsafe { err.into_raw() },
    }
}

extern "C" fn wrap_editor_add_file(
    path: *const std::ffi::c_char,
    parent_baton: *mut std::ffi::c_void,
    copyfrom_path: *const std::ffi::c_char,
    copyfrom_revision: subversion_sys::svn_revnum_t,
    _pool: *mut apr_sys::apr_pool_t,
    file_baton: *mut *mut std::ffi::c_void,
) -> *mut subversion_sys::svn_error_t {
    let path = unsafe { std::ffi::CStr::from_ptr(path) };
    let copyfrom_path = if copyfrom_path.is_null() {
        None
    } else {
        Some(unsafe { std::ffi::CStr::from_ptr(copyfrom_path) })
    };
    // The parent_baton is a Box<dyn DirectoryEditor> that was Box::into_raw'd
    let parent = unsafe { &mut **(parent_baton as *mut Box<dyn DirectoryEditor>) };
    let copyfrom = if let (Some(copyfrom_path), Some(copyfrom_revision)) =
        (copyfrom_path, Revnum::from_raw(copyfrom_revision))
    {
        Some((copyfrom_path.to_str().unwrap(), copyfrom_revision))
    } else {
        None
    };
    match parent.add_file(path.to_str().unwrap(), copyfrom) {
        Ok(file) => {
            unsafe { *file_baton = Box::into_raw(file) as *mut std::ffi::c_void };
            std::ptr::null_mut()
        }
        Err(err) => unsafe { err.into_raw() },
    }
}

extern "C" fn wrap_editor_open_file(
    path: *const std::ffi::c_char,
    parent_baton: *mut std::ffi::c_void,
    base_revision: subversion_sys::svn_revnum_t,
    _pool: *mut apr_sys::apr_pool_t,
    file_baton: *mut *mut std::ffi::c_void,
) -> *mut subversion_sys::svn_error_t {
    let path = unsafe { std::ffi::CStr::from_ptr(path) };
    // The parent_baton is a Box<dyn DirectoryEditor> that was Box::into_raw'd
    let parent = unsafe { &mut **(parent_baton as *mut Box<dyn DirectoryEditor>) };
    match parent.open_file(path.to_str().unwrap(), Revnum::from_raw(base_revision)) {
        Ok(file) => {
            unsafe { *file_baton = Box::into_raw(file) as *mut std::ffi::c_void };
            std::ptr::null_mut()
        }
        Err(err) => unsafe { err.into_raw() },
    }
}

extern "C" fn wrap_editor_apply_textdelta(
    file_baton: *mut std::ffi::c_void,
    base_checksum: *const std::ffi::c_char,
    _result_pool: *mut apr_sys::apr_pool_t,
    _handler: *mut subversion_sys::svn_txdelta_window_handler_t,
    _baton: *mut *mut std::ffi::c_void,
) -> *mut subversion_sys::svn_error_t {
    let base_checksum = if base_checksum.is_null() {
        None
    } else {
        Some(unsafe { std::ffi::CStr::from_ptr(base_checksum) })
    };
    // The file_baton is a Box<dyn FileEditor> that was Box::into_raw'd
    let file = unsafe { &mut **(file_baton as *mut Box<dyn FileEditor>) };
    match file.apply_textdelta(base_checksum.map(|c| c.to_str().unwrap())) {
        Ok(_apply) => {
            // TODO: Set up text delta handler and handler baton properly
            // For now, just return success
            std::ptr::null_mut()
        }
        Err(err) => unsafe { err.into_raw() },
    }
}

extern "C" fn wrap_editor_change_file_prop(
    baton: *mut std::ffi::c_void,
    name: *const std::ffi::c_char,
    value: *const subversion_sys::svn_string_t,
    _pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let name = unsafe { std::ffi::CStr::from_ptr(name) };
    let value = unsafe { std::slice::from_raw_parts((*value).data as *const u8, (*value).len) };
    // The baton is a Box<dyn FileEditor> that was Box::into_raw'd
    let editor = unsafe { &mut **(baton as *mut Box<dyn FileEditor>) };
    match editor.change_prop(name.to_str().unwrap(), value) {
        Ok(()) => std::ptr::null_mut(),
        Err(err) => unsafe { err.into_raw() },
    }
}

extern "C" fn wrap_editor_close_file(
    baton: *mut std::ffi::c_void,
    text_checksum: *const std::ffi::c_char,
    _pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let text_checksum = if text_checksum.is_null() {
        None
    } else {
        Some(unsafe { std::ffi::CStr::from_ptr(text_checksum) })
    };
    let mut boxed_editor = unsafe { Box::from_raw(baton as *mut Box<dyn FileEditor>) };
    match boxed_editor.close(text_checksum.map(|c| c.to_str().unwrap())) {
        Ok(()) => {
            // Box is automatically dropped here, cleaning up memory
            std::ptr::null_mut()
        }
        Err(err) => unsafe { err.into_raw() },
    }
}

extern "C" fn wrap_editor_absent_file(
    text_checksum: *const std::ffi::c_char,
    file_baton: *mut std::ffi::c_void,
    _pooll: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let text_checksum = if text_checksum.is_null() {
        None
    } else {
        Some(unsafe { std::ffi::CStr::from_ptr(text_checksum) })
    };
    // The file_baton is a Box<dyn FileEditor> that was Box::into_raw'd
    let file = unsafe { &mut **(file_baton as *mut Box<dyn FileEditor>) };
    match file.close(text_checksum.map(|c| c.to_str().unwrap())) {
        Ok(()) => std::ptr::null_mut(),
        Err(err) => unsafe { err.into_raw() },
    }
}

extern "C" fn wrap_editor_close_edit(
    edit_baton: *mut std::ffi::c_void,
    _pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    // Reconstruct the fat pointer from the baton
    let editor_ptr = unsafe { *(edit_baton as *mut *mut dyn Editor) };
    let editor = unsafe { &mut *editor_ptr };
    match editor.close() {
        Ok(()) => std::ptr::null_mut(),
        Err(err) => unsafe { err.into_raw() },
    }
}

extern "C" fn wrap_editor_abort_edit(
    edit_baton: *mut std::ffi::c_void,
    _pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    // Reconstruct the fat pointer from the baton
    let editor_ptr = unsafe { *(edit_baton as *mut *mut dyn Editor) };
    let editor = unsafe { &mut *editor_ptr };
    match editor.abort() {
        Ok(()) => std::ptr::null_mut(),
        Err(err) => unsafe { err.into_raw() },
    }
}

extern "C" fn wrap_editor_set_target_revision(
    edit_baton: *mut std::ffi::c_void,
    revision: subversion_sys::svn_revnum_t,
    _pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    // Reconstruct the fat pointer from the baton
    let editor_ptr = unsafe { *(edit_baton as *mut *mut dyn Editor) };
    let editor = unsafe { &mut *editor_ptr };
    match editor.set_target_revision(Revnum::from_raw(revision)) {
        Ok(()) => std::ptr::null_mut(),
        Err(err) => unsafe { err.into_raw() },
    }
}

#[no_mangle]
pub(crate) static WRAP_EDITOR: subversion_sys::svn_delta_editor_t =
    subversion_sys::svn_delta_editor_t {
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
    use super::*;

    #[test]
    fn test_version() {
        assert_eq!(super::version().major(), 1);
    }

    // Simple test editor to use in our tests
    struct TestEditor {
        operations: std::cell::RefCell<Vec<String>>,
    }

    impl Editor for TestEditor {
        fn set_target_revision(
            &mut self,
            revision: Option<crate::Revnum>,
        ) -> Result<(), crate::Error> {
            self.operations
                .borrow_mut()
                .push(format!("set_target_revision({:?})", revision.map(|r| r.0)));
            Ok(())
        }

        fn open_root<'a>(
            &'a mut self,
            base_revision: Option<crate::Revnum>,
        ) -> Result<Box<dyn DirectoryEditor + 'a>, crate::Error> {
            self.operations
                .borrow_mut()
                .push(format!("open_root({:?})", base_revision.map(|r| r.0)));
            Ok(Box::new(TestDirectoryEditor {
                operations: &self.operations,
            }))
        }

        fn close(&mut self) -> Result<(), crate::Error> {
            self.operations.borrow_mut().push("close".to_string());
            Ok(())
        }

        fn abort(&mut self) -> Result<(), crate::Error> {
            self.operations.borrow_mut().push("abort".to_string());
            Ok(())
        }
    }

    struct TestDirectoryEditor<'a> {
        operations: &'a std::cell::RefCell<Vec<String>>,
    }

    impl<'a> DirectoryEditor for TestDirectoryEditor<'a> {
        fn delete_entry(
            &mut self,
            path: &str,
            revision: Option<crate::Revnum>,
        ) -> Result<(), crate::Error> {
            self.operations.borrow_mut().push(format!(
                "delete_entry({}, {:?})",
                path,
                revision.map(|r| r.0)
            ));
            Ok(())
        }

        fn add_directory<'b>(
            &'b mut self,
            path: &str,
            copyfrom: Option<(&str, crate::Revnum)>,
        ) -> Result<Box<dyn DirectoryEditor + 'b>, crate::Error> {
            self.operations.borrow_mut().push(format!(
                "add_directory({}, {:?})",
                path,
                copyfrom.map(|(p, r)| (p, r.0))
            ));
            Ok(Box::new(TestDirectoryEditor {
                operations: self.operations,
            }))
        }

        fn open_directory<'b>(
            &'b mut self,
            path: &str,
            base_revision: Option<crate::Revnum>,
        ) -> Result<Box<dyn DirectoryEditor + 'b>, crate::Error> {
            self.operations.borrow_mut().push(format!(
                "open_directory({}, {:?})",
                path,
                base_revision.map(|r| r.0)
            ));
            Ok(Box::new(TestDirectoryEditor {
                operations: self.operations,
            }))
        }

        fn change_prop(&mut self, name: &str, value: &[u8]) -> Result<(), crate::Error> {
            self.operations
                .borrow_mut()
                .push(format!("change_prop({}, {:?})", name, value));
            Ok(())
        }

        fn close(&mut self) -> Result<(), crate::Error> {
            self.operations
                .borrow_mut()
                .push("close_directory".to_string());
            Ok(())
        }

        fn absent_directory(&mut self, path: &str) -> Result<(), crate::Error> {
            self.operations
                .borrow_mut()
                .push(format!("absent_directory({})", path));
            Ok(())
        }

        fn add_file<'b>(
            &'b mut self,
            path: &str,
            copyfrom: Option<(&str, crate::Revnum)>,
        ) -> Result<Box<dyn FileEditor + 'b>, crate::Error> {
            self.operations.borrow_mut().push(format!(
                "add_file({}, {:?})",
                path,
                copyfrom.map(|(p, r)| (p, r.0))
            ));
            Ok(Box::new(TestFileEditor {
                operations: self.operations,
            }))
        }

        fn open_file<'b>(
            &'b mut self,
            path: &str,
            base_revision: Option<crate::Revnum>,
        ) -> Result<Box<dyn FileEditor + 'b>, crate::Error> {
            self.operations.borrow_mut().push(format!(
                "open_file({}, {:?})",
                path,
                base_revision.map(|r| r.0)
            ));
            Ok(Box::new(TestFileEditor {
                operations: self.operations,
            }))
        }

        fn absent_file(&mut self, path: &str) -> Result<(), crate::Error> {
            self.operations
                .borrow_mut()
                .push(format!("absent_file({})", path));
            Ok(())
        }
    }

    struct TestFileEditor<'a> {
        operations: &'a std::cell::RefCell<Vec<String>>,
    }

    impl<'a> FileEditor for TestFileEditor<'a> {
        fn apply_textdelta(
            &mut self,
            base_checksum: Option<&str>,
        ) -> Result<
            Box<dyn for<'b> Fn(&'b mut TxDeltaWindow) -> Result<(), crate::Error>>,
            crate::Error,
        > {
            self.operations
                .borrow_mut()
                .push(format!("apply_textdelta({:?})", base_checksum));
            Ok(Box::new(|_window| Ok(())))
        }

        fn change_prop(&mut self, name: &str, value: &[u8]) -> Result<(), crate::Error> {
            self.operations
                .borrow_mut()
                .push(format!("change_file_prop({}, {:?})", name, value));
            Ok(())
        }

        fn close(&mut self, text_checksum: Option<&str>) -> Result<(), crate::Error> {
            self.operations
                .borrow_mut()
                .push(format!("close_file({:?})", text_checksum));
            Ok(())
        }
    }

    #[test]
    fn test_wrap_editor_structure() {
        // Test that WRAP_EDITOR has valid function pointers
        assert!(WRAP_EDITOR.open_root.is_some());
        assert!(WRAP_EDITOR.set_target_revision.is_some());
        assert!(WRAP_EDITOR.close_edit.is_some());
        assert!(WRAP_EDITOR.abort_edit.is_some());

        // Check that function pointers are not null
        let open_root_ptr = WRAP_EDITOR.open_root.unwrap();
        let set_target_revision_ptr = WRAP_EDITOR.set_target_revision.unwrap();
        assert_ne!(open_root_ptr as *const (), std::ptr::null());
        assert_ne!(set_target_revision_ptr as *const (), std::ptr::null());

        println!("WRAP_EDITOR.open_root: {:p}", open_root_ptr);
        println!(
            "WRAP_EDITOR.set_target_revision: {:p}",
            set_target_revision_ptr
        );
    }

    #[test]
    fn test_editor_baton_fat_pointer_handling() {
        let mut editor = TestEditor {
            operations: std::cell::RefCell::new(Vec::new()),
        };

        // Test fat pointer decomposition and reconstruction
        let fat_ptr = &mut editor as *mut dyn Editor;
        let raw_parts: (usize, usize) = unsafe { std::mem::transmute(fat_ptr) };

        println!(
            "Original fat pointer: data={:p}, vtable={:p}",
            raw_parts.0 as *const (), raw_parts.1 as *const ()
        );

        // Create EditorBaton
        let baton = EditorBaton {
            data_ptr: raw_parts.0 as *mut std::ffi::c_void,
            vtable_ptr: raw_parts.1 as *mut std::ffi::c_void,
        };

        // Reconstruct fat pointer
        let reconstructed_parts = (baton.data_ptr as usize, baton.vtable_ptr as usize);
        let reconstructed_fat_ptr: *mut dyn Editor =
            unsafe { std::mem::transmute(reconstructed_parts) };

        println!("Reconstructed fat pointer: {:p}", reconstructed_fat_ptr);

        // Test that we can safely dereference the reconstructed pointer
        let reconstructed_editor = unsafe { &mut *reconstructed_fat_ptr };

        // Call a method to verify it works
        reconstructed_editor
            .set_target_revision(Some(crate::Revnum(42)))
            .unwrap();

        let operations = editor.operations.into_inner();
        assert_eq!(operations, vec!["set_target_revision(Some(42))"]);
    }

    #[test]
    fn test_wrap_editor_function_calls() {
        let mut editor = TestEditor {
            operations: std::cell::RefCell::new(Vec::new()),
        };

        // Create EditorBaton
        let fat_ptr = &mut editor as *mut dyn Editor;
        let raw_parts: (usize, usize) = unsafe { std::mem::transmute(fat_ptr) };
        let baton = Box::new(EditorBaton {
            data_ptr: raw_parts.0 as *mut std::ffi::c_void,
            vtable_ptr: raw_parts.1 as *mut std::ffi::c_void,
        });
        let baton_ptr = Box::into_raw(baton) as *mut std::ffi::c_void;

        // Test calling set_target_revision wrapper function directly
        let pool = apr::pool::Pool::new();
        let result = unsafe { wrap_editor_set_target_revision(baton_ptr, 42, pool.as_mut_ptr()) };

        // Clean up
        unsafe {
            let _ = Box::from_raw(baton_ptr as *mut EditorBaton);
        };

        // Check that the call succeeded (null pointer means success)
        assert_eq!(result, std::ptr::null_mut());

        let operations = editor.operations.into_inner();
        assert_eq!(operations, vec!["set_target_revision(Some(42))"]);
    }
}
