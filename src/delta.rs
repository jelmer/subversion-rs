//! Editor interface for efficient tree transformations.
//!
//! This module provides the [`Editor`](crate::delta::Editor) trait and related types for describing changes to
//! directory trees using Subversion's delta editor interface. This is the core mechanism
//! for transmitting changes between client and server.
//!
//! # Overview
//!
//! The delta/editor interface is Subversion's way of describing tree changes efficiently.
//! Instead of transmitting entire trees, operations send a series of editor calls that
//! describe what changed. This is used for updates, commits, diffs, and merges.
//!
//! ## Key Concepts
//!
//! - **[`Editor`](crate::delta::Editor)**: Main trait for receiving tree change events
//! - **[`DirectoryEditor`](crate::delta::DirectoryEditor)**: Trait for handling directory-level changes
//! - **[`FileEditor`](crate::delta::FileEditor)**: Trait for handling file-level changes
//! - **Text deltas**: Efficient binary diffs for file contents
//! - **Property deltas**: Changes to versioned properties
//!
//! # Example
//!
//! ```no_run
//! use subversion::delta::Editor;
//!
//! struct MyEditor;
//!
//! impl Editor for MyEditor {
//!     type Dir = ();
//!     type File = ();
//!
//!     fn open_root(&mut self, base_revision: i64) -> Result<Self::Dir, subversion::Error> {
//!         println!("Opening root at revision {}", base_revision);
//!         Ok(())
//!     }
//!
//!     // Implement other required methods...
//! #   fn set_target_revision(&mut self, target_revision: i64) -> Result<(), subversion::Error> { Ok(()) }
//! #   fn close_directory(&mut self, _dir: Self::Dir) -> Result<(), subversion::Error> { Ok(()) }
//! #   fn close_edit(&mut self) -> Result<(), subversion::Error> { Ok(()) }
//! #   fn abort_edit(&mut self) -> Result<(), subversion::Error> { Ok(()) }
//! }
//! ```

use crate::Revnum;
use apr::pool::Pool;
use std::marker::PhantomData;

/// Returns the version of the delta library.
pub fn version() -> crate::Version {
    crate::Version(unsafe { subversion_sys::svn_delta_version() })
}

/// Creates a default/no-op delta editor.
///
/// This editor does nothing but can be used for testing or as a placeholder.
pub fn default_editor(pool: Pool<'static>) -> WrapEditor<'static> {
    let editor = unsafe { subversion_sys::svn_delta_default_editor(pool.as_mut_ptr()) };
    // The default editor uses null baton
    let baton = std::ptr::null_mut();
    WrapEditor {
        editor,
        baton,
        _pool: apr::PoolHandle::owned(pool),
        callback_batons: Vec::new(), // No callbacks for default editor
    }
}

/// Wrap an editor with depth filtering
///
/// Returns an editor that filters operations based on depth, only forwarding calls
/// that operate within the requested depth range.
///
/// # Arguments
///
/// * `wrapped_editor` - The editor to wrap
/// * `requested_depth` - Depth to filter to (infinity, empty, files, immediates, or unknown)
/// * `has_target` - Whether the edit drive uses a target
/// * `pool` - Pool for allocations
///
/// Returns the filtered editor, or the original editor if filtering is unnecessary.
///
/// Wraps `svn_delta_depth_filter_editor`.
pub fn depth_filter_editor(
    wrapped_editor: &WrapEditor,
    requested_depth: crate::Depth,
    has_target: bool,
    pool: Pool<'static>,
) -> WrapEditor<'static> {
    let mut editor_ptr: *const subversion_sys::svn_delta_editor_t = std::ptr::null();
    let mut edit_baton: *mut std::ffi::c_void = std::ptr::null_mut();

    unsafe {
        subversion_sys::svn_delta_depth_filter_editor(
            &mut editor_ptr,
            &mut edit_baton,
            wrapped_editor.editor,
            wrapped_editor.baton,
            requested_depth.into(),
            has_target as i32,
            pool.as_mut_ptr(),
        );
    }

    WrapEditor {
        editor: editor_ptr,
        baton: edit_baton,
        _pool: apr::PoolHandle::owned(pool),
        callback_batons: Vec::new(),
    }
}

/// Type-erased dropper function for callback batons
pub type DropperFn = unsafe fn(*mut std::ffi::c_void);

/// Wrapper for a Subversion delta editor.
pub struct WrapEditor<'pool> {
    pub(crate) editor: *const subversion_sys::svn_delta_editor_t,
    pub(crate) baton: *mut std::ffi::c_void,
    pub(crate) _pool: apr::PoolHandle<'pool>,
    // Callback batons with their dropper functions
    pub(crate) callback_batons: Vec<(*mut std::ffi::c_void, DropperFn)>,
}
unsafe impl Send for WrapEditor<'_> {}

impl Drop for WrapEditor<'_> {
    fn drop(&mut self) {
        // Clean up callback batons using their type-erased droppers
        for (baton, dropper) in &self.callback_batons {
            if !baton.is_null() {
                unsafe {
                    dropper(*baton);
                }
            }
        }
        self.callback_batons.clear();
    }
}

impl<'pool> Editor for WrapEditor<'pool> {
    type RootEditor = WrapDirectoryEditor<'pool>;

    fn set_target_revision(&mut self, revision: Option<Revnum>) -> Result<(), crate::Error<'_>> {
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

    fn open_root(
        &mut self,
        base_revision: Option<Revnum>,
    ) -> Result<WrapDirectoryEditor<'pool>, crate::Error<'_>> {
        let mut baton = std::ptr::null_mut();
        // Use the editor's pool directly - the directory baton should live as long as the editor
        // Don't create a separate pool that gets destroyed when the directory editor is dropped
        let err = unsafe {
            ((*self.editor).open_root.unwrap())(
                self.baton,
                base_revision.map_or(-1, |r| r.into()),
                self._pool.as_mut_ptr(),
                &mut baton,
            )
        };
        crate::Error::from_raw(err)?;
        Ok(WrapDirectoryEditor {
            editor: self.editor,
            baton,
            _pool: unsafe { apr::PoolHandle::from_borrowed_raw(self._pool.as_mut_ptr()) },
        })
    }

    fn close(&mut self) -> Result<(), crate::Error<'_>> {
        let scratch_pool = Pool::new();
        let err =
            unsafe { ((*self.editor).close_edit.unwrap())(self.baton, scratch_pool.as_mut_ptr()) };
        crate::Error::from_raw(err)?;
        Ok(())
    }

    fn abort(&mut self) -> Result<(), crate::Error<'_>> {
        let scratch_pool = Pool::new();
        let err =
            unsafe { ((*self.editor).abort_edit.unwrap())(self.baton, scratch_pool.as_mut_ptr()) };
        crate::Error::from_raw(err)?;
        Ok(())
    }
}

impl<'pool> WrapEditor<'pool> {
    /// Get the raw editor and baton pointers for FFI operations.
    ///
    /// # Safety
    ///
    /// This method is intentionally crate-private. The returned pointers must not
    /// outlive the WrapEditor, and the baton is tied to the editor's pool lifetime.
    /// Exposing this publicly would allow use-after-free and pool lifetime bugs.
    ///
    /// This is used for low-level operations that need direct access to the C structures.
    #[cfg(any(feature = "ra", feature = "repos"))]
    pub(crate) fn as_raw_parts(
        &self,
    ) -> (
        *const subversion_sys::svn_delta_editor_t,
        *mut std::ffi::c_void,
    ) {
        (self.editor, self.baton)
    }
}

/// Wrapper for a Subversion directory editor.
pub struct WrapDirectoryEditor<'pool> {
    pub(crate) editor: *const subversion_sys::svn_delta_editor_t,
    pub(crate) baton: *mut std::ffi::c_void,
    pub(crate) _pool: apr::PoolHandle<'pool>,
}

impl<'pool> WrapDirectoryEditor<'pool> {
    /// Get the raw editor and baton pointers for FFI operations.
    ///
    /// # Safety
    ///
    /// This method is intentionally crate-private. The returned pointers must not
    /// outlive the WrapDirectoryEditor, and the baton is tied to the pool lifetime.
    /// Exposing this publicly would allow use-after-free and pool lifetime bugs.
    ///
    /// This is used for low-level operations like `svn_wc_transmit_prop_deltas2`
    /// that need direct access to the C structures.
    #[cfg(feature = "wc")]
    pub(crate) fn as_raw_parts(
        &self,
    ) -> (
        *const subversion_sys::svn_delta_editor_t,
        *mut std::ffi::c_void,
    ) {
        (self.editor, self.baton)
    }
}

impl<'pool> DirectoryEditor for WrapDirectoryEditor<'pool> {
    type SubDirectory = WrapDirectoryEditor<'pool>;
    type File = WrapFileEditor<'pool>;

    fn delete_entry(
        &mut self,
        path: &str,
        revision: Option<Revnum>,
    ) -> Result<(), crate::Error<'_>> {
        let path_cstr = std::ffi::CString::new(path)?;
        let scratch_pool = Pool::new();
        let err = unsafe {
            ((*self.editor).delete_entry.unwrap())(
                path_cstr.as_ptr(),
                revision.map_or(-1, |r| r.into()),
                self.baton,
                scratch_pool.as_mut_ptr(),
            )
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }

    fn add_directory(
        &mut self,
        path: &str,
        copyfrom: Option<(&str, Revnum)>,
    ) -> Result<WrapDirectoryEditor<'pool>, crate::Error<'_>> {
        let path_cstr = std::ffi::CString::new(path)?;
        let copyfrom_path = copyfrom
            .map(|(p, _)| std::ffi::CString::new(p))
            .transpose()?;
        let copyfrom_rev = copyfrom.map(|(_, r)| r.0).unwrap_or(-1);
        let mut baton = std::ptr::null_mut();
        unsafe {
            let err = ((*self.editor).add_directory.unwrap())(
                path_cstr.as_ptr(),
                self.baton,
                if let Some(ref copyfrom_path) = copyfrom_path {
                    copyfrom_path.as_ptr()
                } else {
                    std::ptr::null()
                },
                copyfrom_rev,
                self._pool.as_mut_ptr(),
                &mut baton,
            );
            crate::Error::from_raw(err)?;
        }
        Ok(WrapDirectoryEditor {
            editor: self.editor,
            baton,
            _pool: unsafe { apr::PoolHandle::from_borrowed_raw(self._pool.as_mut_ptr()) },
        })
    }

    fn open_directory(
        &mut self,
        path: &str,
        base_revision: Option<Revnum>,
    ) -> Result<WrapDirectoryEditor<'pool>, crate::Error<'_>> {
        let path_cstr = std::ffi::CString::new(path)?;
        let mut baton = std::ptr::null_mut();
        unsafe {
            let err = ((*self.editor).open_directory.unwrap())(
                path_cstr.as_ptr(),
                self.baton,
                base_revision.map_or(-1, |r| r.0),
                self._pool.as_mut_ptr(),
                &mut baton,
            );
            crate::Error::from_raw(err)?;
        }
        Ok(WrapDirectoryEditor {
            editor: self.editor,
            baton,
            _pool: unsafe { apr::PoolHandle::from_borrowed_raw(self._pool.as_mut_ptr()) },
        })
    }

    fn change_prop(&mut self, name: &str, value: Option<&[u8]>) -> Result<(), crate::Error<'_>> {
        let scratch_pool = apr::pool::Pool::new();
        let name_cstr = std::ffi::CString::new(name)?;
        let value_ptr = if let Some(v) = value {
            let value = crate::string::BStr::from_bytes(v, &scratch_pool);
            value.as_ptr()
        } else {
            std::ptr::null()
        };
        let err = unsafe {
            ((*self.editor).change_dir_prop.unwrap())(
                self.baton,
                name_cstr.as_ptr(),
                value_ptr,
                scratch_pool.as_mut_ptr(),
            )
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }

    fn close(&mut self) -> Result<(), crate::Error<'_>> {
        let scratch_pool = apr::pool::Pool::new();
        let err = unsafe {
            ((*self.editor).close_directory.unwrap())(self.baton, scratch_pool.as_mut_ptr())
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }

    fn absent_directory(&mut self, path: &str) -> Result<(), crate::Error<'_>> {
        let scratch_pool = apr::pool::Pool::new();
        let path_cstr = std::ffi::CString::new(path)?;
        let err = unsafe {
            ((*self.editor).absent_directory.unwrap())(
                path_cstr.as_ptr(),
                self.baton,
                scratch_pool.as_mut_ptr(),
            )
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }

    fn add_file(
        &mut self,
        path: &str,
        copyfrom: Option<(&str, Revnum)>,
    ) -> Result<WrapFileEditor<'pool>, crate::Error<'_>> {
        let path_cstr = std::ffi::CString::new(path)?;
        let copyfrom_path = copyfrom
            .map(|(p, _)| std::ffi::CString::new(p))
            .transpose()?;
        let copyfrom_rev = copyfrom.map(|(_, r)| r.0).unwrap_or(-1);
        let mut baton = std::ptr::null_mut();
        unsafe {
            let err = ((*self.editor).add_file.unwrap())(
                path_cstr.as_ptr(),
                self.baton,
                if let Some(ref copyfrom_path) = copyfrom_path {
                    copyfrom_path.as_ptr()
                } else {
                    std::ptr::null()
                },
                copyfrom_rev,
                self._pool.as_mut_ptr(),
                &mut baton,
            );
            crate::Error::from_raw(err)?;
        }
        Ok(WrapFileEditor {
            editor: self.editor,
            baton,
            _pool: unsafe { apr::PoolHandle::from_borrowed_raw(self._pool.as_mut_ptr()) },
        })
    }

    fn open_file(
        &mut self,
        path: &str,
        base_revision: Option<Revnum>,
    ) -> Result<WrapFileEditor<'pool>, crate::Error<'_>> {
        let path_cstr = std::ffi::CString::new(path)?;
        let mut baton = std::ptr::null_mut();
        unsafe {
            let err = ((*self.editor).open_file.unwrap())(
                path_cstr.as_ptr(),
                self.baton,
                base_revision.map_or(-1, |r| r.into()),
                self._pool.as_mut_ptr(),
                &mut baton,
            );
            crate::Error::from_raw(err)?;
        }
        Ok(WrapFileEditor {
            editor: self.editor,
            baton,
            _pool: unsafe { apr::PoolHandle::from_borrowed_raw(self._pool.as_mut_ptr()) },
        })
    }

    fn absent_file(&mut self, path: &str) -> Result<(), crate::Error<'_>> {
        let scratch_pool = apr::pool::Pool::new();
        let path_cstr = std::ffi::CString::new(path)?;
        let err = unsafe {
            ((*self.editor).absent_file.unwrap())(
                path_cstr.as_ptr(),
                self.baton,
                scratch_pool.as_mut_ptr(),
            )
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }
}

/// Wrapper for a Subversion file editor.
pub struct WrapFileEditor<'pool> {
    editor: *const subversion_sys::svn_delta_editor_t,
    baton: *mut std::ffi::c_void,
    _pool: apr::PoolHandle<'pool>,
}

impl<'pool> WrapFileEditor<'pool> {
    /// Get the raw editor and baton pointers for FFI operations.
    ///
    /// # Safety
    ///
    /// This method is intentionally crate-private. The returned pointers must not
    /// outlive the WrapFileEditor, and the baton is tied to the pool lifetime.
    /// Exposing this publicly would allow use-after-free and pool lifetime bugs.
    ///
    /// This is used for low-level operations like `svn_wc_transmit_text_deltas3`
    /// that need direct access to the C structures.
    #[cfg(feature = "wc")]
    pub(crate) fn as_raw_parts(
        &self,
    ) -> (
        *const subversion_sys::svn_delta_editor_t,
        *mut std::ffi::c_void,
    ) {
        (self.editor, self.baton)
    }

    /// Apply a text delta and return the window handler wrapper.
    ///
    /// This is an alternative to the `FileEditor::apply_textdelta()` trait method
    /// that returns the handler wrapper directly instead of a closure, allowing
    /// more flexible use from language bindings.
    pub fn apply_textdelta_raw(
        &mut self,
        base_checksum: Option<&str>,
    ) -> Result<WrapTxdeltaWindowHandler, crate::Error<'static>> {
        let pool = apr::pool::Pool::new();
        let base_checksum_cstr = base_checksum.map(std::ffi::CString::new).transpose()?;
        let mut handler: subversion_sys::svn_txdelta_window_handler_t = None;
        let mut baton = std::ptr::null_mut();
        let err = unsafe {
            ((*self.editor).apply_textdelta.unwrap())(
                self.baton,
                if let Some(ref base_checksum_cstr) = base_checksum_cstr {
                    base_checksum_cstr.as_ptr()
                } else {
                    std::ptr::null()
                },
                pool.as_mut_ptr(),
                &mut handler,
                &mut baton,
            )
        };
        crate::Error::from_raw(err)?;
        Ok(WrapTxdeltaWindowHandler::from_raw(handler, baton, pool))
    }
}

/// Wrapper for a text delta window handler.
pub struct WrapTxdeltaWindowHandler {
    handler: subversion_sys::svn_txdelta_window_handler_t,
    baton: *mut std::ffi::c_void,
    _pool: apr::Pool<'static>,
    _phantom: PhantomData<*mut ()>,
}

impl Drop for WrapTxdeltaWindowHandler {
    fn drop(&mut self) {
        // Pool drop will clean up
    }
}

impl WrapTxdeltaWindowHandler {
    /// Create from raw pointer and baton
    pub(crate) fn from_raw(
        handler: subversion_sys::svn_txdelta_window_handler_t,
        baton: *mut std::ffi::c_void,
        pool: apr::Pool<'static>,
    ) -> Self {
        Self {
            handler,
            baton,
            _pool: pool,
            _phantom: PhantomData,
        }
    }

    /// Call the handler with a delta window
    pub fn call(&self, window: &mut TxDeltaWindow) -> Result<(), crate::Error<'static>> {
        let err = unsafe { (self.handler.unwrap())(window.as_mut_ptr(), self.baton) };
        crate::Error::from_raw(err)?;
        Ok(())
    }

    /// Call the handler with None to signal completion
    pub fn finish(&self) -> Result<(), crate::Error<'static>> {
        let err = unsafe { (self.handler.unwrap())(std::ptr::null_mut(), self.baton) };
        crate::Error::from_raw(err)?;
        Ok(())
    }
}

impl<'pool> FileEditor for WrapFileEditor<'pool> {
    fn apply_textdelta(
        &mut self,
        base_checksum: Option<&str>,
    ) -> Result<
        Box<dyn for<'b> Fn(&'b mut TxDeltaWindow) -> Result<(), crate::Error<'static>>>,
        crate::Error<'static>,
    > {
        let pool = apr::pool::Pool::new();
        let base_checksum_cstr = base_checksum.map(std::ffi::CString::new).transpose()?;
        let mut handler = None;
        let mut baton = std::ptr::null_mut();
        let err = unsafe {
            ((*self.editor).apply_textdelta.unwrap())(
                self.baton,
                if let Some(ref base_checksum_cstr) = base_checksum_cstr {
                    base_checksum_cstr.as_ptr()
                } else {
                    std::ptr::null()
                },
                pool.as_mut_ptr(),
                &mut handler,
                &mut baton,
            )
        };
        crate::Error::from_raw(err)?;
        let apply = move |window: &mut TxDeltaWindow| -> Result<(), crate::Error<'static>> {
            let err = unsafe { (handler.unwrap())(window.as_mut_ptr(), baton) };
            crate::Error::from_raw(err)?;
            Ok(())
        };
        Ok(Box::new(apply))
    }

    fn change_prop(
        &mut self,
        name: &str,
        value: Option<&[u8]>,
    ) -> Result<(), crate::Error<'static>> {
        let scratch_pool = apr::pool::Pool::new();
        let name_cstr = std::ffi::CString::new(name)?;
        let value_ptr = if let Some(v) = value {
            let value = crate::string::BStr::from_bytes(v, &scratch_pool);
            value.as_ptr()
        } else {
            std::ptr::null()
        };
        let err = unsafe {
            ((*self.editor).change_file_prop.unwrap())(
                self.baton,
                name_cstr.as_ptr(),
                value_ptr,
                scratch_pool.as_mut_ptr(),
            )
        };
        crate::Error::from_raw(err)?;
        Ok(())
    }

    fn close(&mut self, text_checksum: Option<&str>) -> Result<(), crate::Error<'static>> {
        let pool = apr::pool::Pool::new();
        let text_checksum_cstr = text_checksum.map(std::ffi::CString::new).transpose()?;
        let err = unsafe {
            ((*self.editor).close_file.unwrap())(
                self.baton,
                if let Some(ref text_checksum_cstr) = text_checksum_cstr {
                    text_checksum_cstr.as_ptr()
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

/// Trait for delta editor operations.
pub trait Editor {
    /// The type of directory editor returned when opening the root directory.
    type RootEditor: DirectoryEditor;

    /// Sets the target revision for the edit.
    fn set_target_revision(&mut self, revision: Option<Revnum>) -> Result<(), crate::Error<'_>>;

    /// Opens the root directory.
    fn open_root(
        &mut self,
        base_revision: Option<Revnum>,
    ) -> Result<Self::RootEditor, crate::Error<'_>>;

    /// Closes the editor.
    fn close(&mut self) -> Result<(), crate::Error<'_>>;

    /// Aborts the edit operation.
    fn abort(&mut self) -> Result<(), crate::Error<'_>>;
}

/// Trait for directory editor operations.
pub trait DirectoryEditor {
    /// The type of directory editor returned when opening or adding subdirectories.
    type SubDirectory: DirectoryEditor;
    /// The type of file editor returned when opening or adding files.
    type File: FileEditor;

    /// Deletes an entry from the directory.
    fn delete_entry(
        &mut self,
        path: &str,
        revision: Option<Revnum>,
    ) -> Result<(), crate::Error<'_>>;

    /// Adds a new directory.
    fn add_directory(
        &mut self,
        path: &str,
        copyfrom: Option<(&str, Revnum)>,
    ) -> Result<Self::SubDirectory, crate::Error<'_>>;

    /// Opens an existing directory.
    fn open_directory(
        &mut self,
        path: &str,
        base_revision: Option<Revnum>,
    ) -> Result<Self::SubDirectory, crate::Error<'_>>;

    /// Changes a property on the directory.
    /// Pass `None` for value to delete the property.
    fn change_prop(&mut self, name: &str, value: Option<&[u8]>) -> Result<(), crate::Error<'_>>;

    /// Closes the directory.
    fn close(&mut self) -> Result<(), crate::Error<'_>>;

    /// Marks a directory as absent.
    fn absent_directory(&mut self, path: &str) -> Result<(), crate::Error<'_>>;

    /// Adds a new file to the directory.
    fn add_file(
        &mut self,
        path: &str,
        copyfrom: Option<(&str, Revnum)>,
    ) -> Result<Self::File, crate::Error<'_>>;

    /// Opens an existing file for editing.
    fn open_file(
        &mut self,
        path: &str,
        base_revision: Option<Revnum>,
    ) -> Result<Self::File, crate::Error<'_>>;

    /// Marks a file as absent.
    fn absent_file(&mut self, path: &str) -> Result<(), crate::Error<'_>>;
}

/// No-op handler for text delta windows.
pub fn noop_window_handler(window: &mut TxDeltaWindow) -> Result<(), crate::Error<'_>> {
    let err = unsafe {
        subversion_sys::svn_delta_noop_window_handler(window.as_mut_ptr(), std::ptr::null_mut())
    };
    crate::Error::from_raw(err)?;
    Ok(())
}

/// Trait for file editor operations.
pub trait FileEditor {
    /// Applies a text delta to the file.
    fn apply_textdelta(
        &mut self,
        base_checksum: Option<&str>,
    ) -> Result<
        Box<dyn for<'a> Fn(&'a mut TxDeltaWindow) -> Result<(), crate::Error<'static>>>,
        crate::Error<'static>,
    >;

    /// Applies a text delta stream to the file.
    ///
    /// The `open_stream` callback should create and return a TxDeltaStream that
    /// provides the delta data. This is an alternative to `apply_textdelta` that
    /// can be more efficient for streaming operations.
    ///
    /// Default implementation returns an error indicating it's not supported.
    fn apply_textdelta_stream(
        &mut self,
        _base_checksum: Option<&str>,
        _open_stream: Box<dyn FnOnce() -> Result<TxDeltaStream, crate::Error<'static>>>,
    ) -> Result<(), crate::Error<'static>> {
        Err(crate::Error::from_message(
            "apply_textdelta_stream not supported by this editor",
        ))
    }

    /// Changes a property on the file.
    /// Pass `None` for value to delete the property.
    fn change_prop(
        &mut self,
        name: &str,
        value: Option<&[u8]>,
    ) -> Result<(), crate::Error<'static>>;

    /// Closes the file editor.
    fn close(&mut self, text_checksum: Option<&str>) -> Result<(), crate::Error<'static>>;
}

/// TxDelta window with RAII cleanup
pub struct TxDeltaWindow {
    ptr: *mut subversion_sys::svn_txdelta_window_t,
    pool: apr::Pool<'static>,
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
    pub fn pool(&self) -> &apr::Pool<'_> {
        &self.pool
    }

    /// Get the mutable raw pointer to the window (use with caution)
    pub fn as_mut_ptr(&mut self) -> *mut subversion_sys::svn_txdelta_window_t {
        self.ptr
    }
    /// Get the raw pointer to the window.
    pub fn as_ptr(&self) -> *const subversion_sys::svn_txdelta_window_t {
        self.ptr
    }

    /// Creates a new TxDelta window.
    pub fn new() -> Self {
        let pool = apr::Pool::new();
        let ptr = pool.calloc::<subversion_sys::svn_txdelta_window_t>();
        Self {
            ptr,
            pool,
            _phantom: PhantomData,
        }
    }

    /// Creates a TxDelta window from constituent parts.
    ///
    /// # Arguments
    /// * `sview_offset` - Source view offset
    /// * `sview_len` - Source view length
    /// * `tview_len` - Target view length
    /// * `src_ops` - Number of source operations (usually ignored)
    /// * `ops` - Delta operations as (action_code, offset, length) tuples
    /// * `new_data` - New data buffer
    pub fn from_parts(
        sview_offset: u64,
        sview_len: u64,
        tview_len: u64,
        src_ops: i32,
        ops: &[(i32, u64, u64)],
        new_data: &[u8],
    ) -> Self {
        let pool = apr::Pool::new();

        unsafe {
            let window_ptr = pool.calloc::<subversion_sys::svn_txdelta_window_t>();

            let ops_ptr = if ops.is_empty() {
                std::ptr::null_mut()
            } else {
                apr_sys::apr_palloc(
                    pool.as_mut_ptr(),
                    std::mem::size_of::<subversion_sys::svn_txdelta_op_t>() * ops.len(),
                ) as *mut subversion_sys::svn_txdelta_op_t
            };

            // Fill in ops
            for (i, &(action_code, offset, length)) in ops.iter().enumerate() {
                (*ops_ptr.add(i)).action_code = action_code as std::os::raw::c_uint;
                (*ops_ptr.add(i)).offset = offset as apr_sys::apr_size_t;
                (*ops_ptr.add(i)).length = length as apr_sys::apr_size_t;
            }

            // Allocate and copy new_data if present
            let new_data_ptr = if new_data.is_empty() {
                std::ptr::null()
            } else {
                let svn_string = apr_sys::apr_palloc(
                    pool.as_mut_ptr(),
                    std::mem::size_of::<subversion_sys::svn_string_t>(),
                ) as *mut subversion_sys::svn_string_t;

                let data_buf = apr_sys::apr_palloc(pool.as_mut_ptr(), new_data.len())
                    as *mut std::os::raw::c_char;
                std::ptr::copy_nonoverlapping(
                    new_data.as_ptr(),
                    data_buf as *mut u8,
                    new_data.len(),
                );

                (*svn_string).data = data_buf;
                (*svn_string).len = new_data.len();
                svn_string as *const subversion_sys::svn_string_t
            };

            // Fill in window struct
            (*window_ptr).sview_offset = sview_offset as subversion_sys::svn_filesize_t;
            (*window_ptr).sview_len = sview_len as apr_sys::apr_size_t;
            (*window_ptr).tview_len = tview_len as apr_sys::apr_size_t;
            (*window_ptr).num_ops = ops.len() as std::os::raw::c_int;
            (*window_ptr).src_ops = src_ops as std::os::raw::c_int;
            (*window_ptr).ops = ops_ptr;
            (*window_ptr).new_data = new_data_ptr;

            Self {
                ptr: window_ptr,
                pool,
                _phantom: PhantomData,
            }
        }
    }

    /// Gets the source view length.
    pub fn sview_len(&self) -> apr_sys::apr_size_t {
        unsafe { (*self.ptr).sview_len }
    }

    /// Gets the target view length.
    pub fn tview_len(&self) -> apr_sys::apr_size_t {
        unsafe { (*self.ptr).tview_len }
    }

    /// Gets the source view offset.
    pub fn sview_offset(&self) -> crate::FileSize {
        unsafe { (*self.ptr).sview_offset }
    }

    /// Gets the number of source operations.
    pub fn src_ops(&self) -> i32 {
        unsafe { (*self.ptr).src_ops as i32 }
    }

    /// Gets the delta operations as (action_code, offset, length) tuples.
    pub fn ops(&self) -> Vec<(i32, u64, u64)> {
        unsafe {
            let num = (*self.ptr).num_ops as usize;
            let ops_ptr = (*self.ptr).ops;
            if ops_ptr.is_null() || num == 0 {
                return Vec::new();
            }
            (0..num)
                .map(|i| {
                    let op = &*ops_ptr.add(i);
                    (op.action_code as i32, op.offset as u64, op.length as u64)
                })
                .collect()
        }
    }

    /// Gets the new data buffer.
    pub fn new_data(&self) -> &[u8] {
        unsafe {
            let nd = (*self.ptr).new_data;
            if nd.is_null() || (*nd).data.is_null() {
                &[]
            } else {
                std::slice::from_raw_parts((*nd).data as *const u8, (*nd).len)
            }
        }
    }

    /// Composes two TxDelta windows.
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

    /// Applies the window instructions to transform source to target.
    pub fn apply_instructions(
        &mut self,
        source: &mut [u8],
        target: &mut Vec<u8>,
    ) -> Result<(), crate::Error<'_>> {
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

    /// Creates a duplicate of the window.
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
// ============================================================================
// Rust Editor Support: Monomorphized C callbacks for custom Rust editors
// ============================================================================

/// C callback for Editor::set_target_revision - monomorphized for concrete type E
unsafe extern "C" fn rust_editor_set_target_revision<E: Editor>(
    edit_baton: *mut std::ffi::c_void,
    target_revision: subversion_sys::svn_revnum_t,
    _scratch_pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let editor = &mut *(edit_baton as *mut E);
    let revision = if target_revision < 0 {
        None
    } else {
        Some(Revnum(target_revision))
    };
    match editor.set_target_revision(revision) {
        Ok(()) => std::ptr::null_mut(),
        Err(e) => e.into_raw(),
    }
}

/// C callback for Editor::open_root - monomorphized for concrete types E and D
unsafe extern "C" fn rust_editor_open_root<E, D>(
    edit_baton: *mut std::ffi::c_void,
    base_revision: subversion_sys::svn_revnum_t,
    _dir_pool: *mut apr_sys::apr_pool_t,
    root_baton: *mut *mut std::ffi::c_void,
) -> *mut subversion_sys::svn_error_t
where
    E: Editor<RootEditor = D>,
    D: DirectoryEditor,
{
    let editor = &mut *(edit_baton as *mut E);
    let revision = if base_revision < 0 {
        None
    } else {
        Some(Revnum(base_revision))
    };
    match editor.open_root(revision) {
        Ok(dir_editor) => {
            *root_baton = Box::into_raw(Box::new(dir_editor)) as *mut std::ffi::c_void;
            std::ptr::null_mut()
        }
        Err(e) => e.into_raw(),
    }
}

/// C callback for Editor::close - monomorphized for concrete type E
unsafe extern "C" fn rust_editor_close<E: Editor>(
    edit_baton: *mut std::ffi::c_void,
    _scratch_pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let editor = &mut *(edit_baton as *mut E);
    match editor.close() {
        Ok(()) => std::ptr::null_mut(),
        Err(e) => e.into_raw(),
    }
}

/// C callback for Editor::abort - monomorphized for concrete type E
unsafe extern "C" fn rust_editor_abort<E: Editor>(
    edit_baton: *mut std::ffi::c_void,
    _scratch_pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let editor = &mut *(edit_baton as *mut E);
    match editor.abort() {
        Ok(()) => std::ptr::null_mut(),
        Err(e) => e.into_raw(),
    }
}

/// C callback for DirectoryEditor::delete_entry - monomorphized for type D
unsafe extern "C" fn rust_dir_delete_entry<D: DirectoryEditor>(
    path: *const std::ffi::c_char,
    revision: subversion_sys::svn_revnum_t,
    parent_baton: *mut std::ffi::c_void,
    _scratch_pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let dir = &mut *(parent_baton as *mut D);
    let path_str = std::ffi::CStr::from_ptr(path).to_str().unwrap();
    let rev = if revision < 0 {
        None
    } else {
        Some(Revnum(revision))
    };
    match dir.delete_entry(path_str, rev) {
        Ok(()) => std::ptr::null_mut(),
        Err(e) => e.into_raw(),
    }
}

/// C callback for DirectoryEditor::add_directory - monomorphized for type D
unsafe extern "C" fn rust_dir_add_directory<D>(
    path: *const std::ffi::c_char,
    parent_baton: *mut std::ffi::c_void,
    copyfrom_path: *const std::ffi::c_char,
    copyfrom_revision: subversion_sys::svn_revnum_t,
    _child_pool: *mut apr_sys::apr_pool_t,
    child_baton: *mut *mut std::ffi::c_void,
) -> *mut subversion_sys::svn_error_t
where
    D: DirectoryEditor<SubDirectory = D>,
{
    let dir = &mut *(parent_baton as *mut D);
    let path_str = std::ffi::CStr::from_ptr(path).to_str().unwrap();
    let copyfrom = if copyfrom_path.is_null() {
        None
    } else {
        let cf_path = std::ffi::CStr::from_ptr(copyfrom_path).to_str().unwrap();
        Some((cf_path, Revnum(copyfrom_revision)))
    };
    match dir.add_directory(path_str, copyfrom) {
        Ok(subdir) => {
            *child_baton = Box::into_raw(Box::new(subdir)) as *mut std::ffi::c_void;
            std::ptr::null_mut()
        }
        Err(e) => e.into_raw(),
    }
}

/// C callback for DirectoryEditor::open_directory - monomorphized for type D
unsafe extern "C" fn rust_dir_open_directory<D>(
    path: *const std::ffi::c_char,
    parent_baton: *mut std::ffi::c_void,
    base_revision: subversion_sys::svn_revnum_t,
    _child_pool: *mut apr_sys::apr_pool_t,
    child_baton: *mut *mut std::ffi::c_void,
) -> *mut subversion_sys::svn_error_t
where
    D: DirectoryEditor<SubDirectory = D>,
{
    let dir = &mut *(parent_baton as *mut D);
    let path_str = std::ffi::CStr::from_ptr(path).to_str().unwrap();
    let revision = if base_revision < 0 {
        None
    } else {
        Some(Revnum(base_revision))
    };
    match dir.open_directory(path_str, revision) {
        Ok(subdir) => {
            *child_baton = Box::into_raw(Box::new(subdir)) as *mut std::ffi::c_void;
            std::ptr::null_mut()
        }
        Err(e) => e.into_raw(),
    }
}

/// C callback for DirectoryEditor::close - monomorphized for type D
unsafe extern "C" fn rust_dir_close<D: DirectoryEditor>(
    dir_baton: *mut std::ffi::c_void,
    _scratch_pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let dir = &mut *(dir_baton as *mut D);
    match dir.close() {
        Ok(()) => {
            let _ = Box::from_raw(dir_baton as *mut D);
            std::ptr::null_mut()
        }
        Err(e) => e.into_raw(),
    }
}

/// C callback for DirectoryEditor::absent_directory - monomorphized for type D
unsafe extern "C" fn rust_dir_absent_directory<D: DirectoryEditor>(
    path: *const std::ffi::c_char,
    parent_baton: *mut std::ffi::c_void,
    _scratch_pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let dir = &mut *(parent_baton as *mut D);
    let path_str = std::ffi::CStr::from_ptr(path).to_str().unwrap();
    match dir.absent_directory(path_str) {
        Ok(()) => std::ptr::null_mut(),
        Err(e) => e.into_raw(),
    }
}

/// C callback for DirectoryEditor::change_prop - monomorphized for type D
unsafe extern "C" fn rust_dir_change_prop<D: DirectoryEditor>(
    dir_baton: *mut std::ffi::c_void,
    name: *const std::ffi::c_char,
    value: *const subversion_sys::svn_string_t,
    _scratch_pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let dir = &mut *(dir_baton as *mut D);
    let name_str = std::ffi::CStr::from_ptr(name).to_str().unwrap();
    let value_bytes = if value.is_null() {
        None
    } else {
        Some(std::slice::from_raw_parts(
            (*value).data as *const u8,
            (*value).len,
        ))
    };
    match dir.change_prop(name_str, value_bytes) {
        Ok(()) => std::ptr::null_mut(),
        Err(e) => e.into_raw(),
    }
}

/// C callback for DirectoryEditor::add_file - monomorphized for types D and F
unsafe extern "C" fn rust_dir_add_file<D, F>(
    path: *const std::ffi::c_char,
    parent_baton: *mut std::ffi::c_void,
    copyfrom_path: *const std::ffi::c_char,
    copyfrom_revision: subversion_sys::svn_revnum_t,
    _file_pool: *mut apr_sys::apr_pool_t,
    file_baton: *mut *mut std::ffi::c_void,
) -> *mut subversion_sys::svn_error_t
where
    D: DirectoryEditor<File = F>,
    F: FileEditor,
{
    let dir = &mut *(parent_baton as *mut D);
    let path_str = std::ffi::CStr::from_ptr(path).to_str().unwrap();
    let copyfrom = if copyfrom_path.is_null() {
        None
    } else {
        let cf_path = std::ffi::CStr::from_ptr(copyfrom_path).to_str().unwrap();
        Some((cf_path, Revnum(copyfrom_revision)))
    };
    match dir.add_file(path_str, copyfrom) {
        Ok(file) => {
            *file_baton = Box::into_raw(Box::new(file)) as *mut std::ffi::c_void;
            std::ptr::null_mut()
        }
        Err(e) => e.into_raw(),
    }
}

/// C callback for DirectoryEditor::open_file - monomorphized for types D and F
unsafe extern "C" fn rust_dir_open_file<D, F>(
    path: *const std::ffi::c_char,
    parent_baton: *mut std::ffi::c_void,
    base_revision: subversion_sys::svn_revnum_t,
    _file_pool: *mut apr_sys::apr_pool_t,
    file_baton: *mut *mut std::ffi::c_void,
) -> *mut subversion_sys::svn_error_t
where
    D: DirectoryEditor<File = F>,
    F: FileEditor,
{
    let dir = &mut *(parent_baton as *mut D);
    let path_str = std::ffi::CStr::from_ptr(path).to_str().unwrap();
    let revision = if base_revision < 0 {
        None
    } else {
        Some(Revnum(base_revision))
    };
    match dir.open_file(path_str, revision) {
        Ok(file) => {
            *file_baton = Box::into_raw(Box::new(file)) as *mut std::ffi::c_void;
            std::ptr::null_mut()
        }
        Err(e) => e.into_raw(),
    }
}

/// C callback for DirectoryEditor::absent_file - monomorphized for type D
unsafe extern "C" fn rust_dir_absent_file<D: DirectoryEditor>(
    path: *const std::ffi::c_char,
    parent_baton: *mut std::ffi::c_void,
    _scratch_pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let dir = &mut *(parent_baton as *mut D);
    let path_str = std::ffi::CStr::from_ptr(path).to_str().unwrap();
    match dir.absent_file(path_str) {
        Ok(()) => std::ptr::null_mut(),
        Err(e) => e.into_raw(),
    }
}

/// C callback for FileEditor::change_prop - monomorphized for type F
unsafe extern "C" fn rust_file_change_prop<F: FileEditor>(
    file_baton: *mut std::ffi::c_void,
    name: *const std::ffi::c_char,
    value: *const subversion_sys::svn_string_t,
    _scratch_pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let file = &mut *(file_baton as *mut F);
    let name_str = std::ffi::CStr::from_ptr(name).to_str().unwrap();
    let value_bytes = if value.is_null() {
        None
    } else {
        Some(std::slice::from_raw_parts(
            (*value).data as *const u8,
            (*value).len,
        ))
    };
    match file.change_prop(name_str, value_bytes) {
        Ok(()) => std::ptr::null_mut(),
        Err(e) => e.into_raw(),
    }
}

/// C callback for FileEditor::close - monomorphized for type F
unsafe extern "C" fn rust_file_close<F: FileEditor>(
    file_baton: *mut std::ffi::c_void,
    text_checksum: *const std::ffi::c_char,
    _scratch_pool: *mut apr_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let file = &mut *(file_baton as *mut F);
    let checksum = if text_checksum.is_null() {
        None
    } else {
        Some(std::ffi::CStr::from_ptr(text_checksum).to_str().unwrap())
    };
    match file.close(checksum) {
        Ok(()) => {
            // Free the file editor
            let _ = Box::from_raw(file_baton as *mut F);
            std::ptr::null_mut()
        }
        Err(e) => e.into_raw(),
    }
}

/// C trampoline that forwards txdelta windows to a boxed Rust closure.
///
/// The baton is a leaked `Box<Box<dyn Fn>>`. When `window` is null
/// (signaling end of delta), the closure box is freed.
unsafe extern "C" fn rust_txdelta_window_handler(
    window: *mut subversion_sys::svn_txdelta_window_t,
    baton: *mut std::ffi::c_void,
) -> *mut subversion_sys::svn_error_t {
    let closure = &*(baton
        as *const Box<dyn for<'a> Fn(&'a mut TxDeltaWindow) -> Result<(), crate::Error<'static>>>);
    if window.is_null() {
        // End of delta - call with a null window to signal completion,
        // then free the closure.
        let mut w = TxDeltaWindow {
            ptr: std::ptr::null_mut(),
            pool: apr::Pool::new(),
            _phantom: PhantomData,
        };
        let result = match closure(&mut w) {
            Ok(()) => std::ptr::null_mut(),
            Err(e) => e.into_raw(),
        };
        // Free the closure
        let _ = Box::from_raw(
            baton
                as *mut Box<
                    dyn for<'a> Fn(&'a mut TxDeltaWindow) -> Result<(), crate::Error<'static>>,
                >,
        );
        result
    } else {
        let mut w = TxDeltaWindow {
            ptr: window,
            pool: apr::Pool::new(),
            _phantom: PhantomData,
        };
        match closure(&mut w) {
            Ok(()) => std::ptr::null_mut(),
            Err(e) => e.into_raw(),
        }
    }
}

/// C callback for apply_textdelta that bridges to `FileEditor::apply_textdelta`.
unsafe extern "C" fn rust_file_apply_textdelta<F: FileEditor>(
    file_baton: *mut std::ffi::c_void,
    base_checksum: *const std::ffi::c_char,
    _result_pool: *mut apr_sys::apr_pool_t,
    handler: *mut subversion_sys::svn_txdelta_window_handler_t,
    handler_baton: *mut *mut std::ffi::c_void,
) -> *mut subversion_sys::svn_error_t {
    let file = &mut *(file_baton as *mut F);
    let checksum = if base_checksum.is_null() {
        None
    } else {
        std::ffi::CStr::from_ptr(base_checksum).to_str().ok()
    };
    match file.apply_textdelta(checksum) {
        Ok(closure) => {
            let boxed: Box<
                Box<dyn for<'a> Fn(&'a mut TxDeltaWindow) -> Result<(), crate::Error<'static>>>,
            > = Box::new(closure);
            *handler_baton = Box::into_raw(boxed) as *mut std::ffi::c_void;
            *handler = Some(rust_txdelta_window_handler);
            std::ptr::null_mut()
        }
        Err(e) => e.into_raw(),
    }
}

/// Generate a C vtable for a Rust editor with concrete types E, D, and F
fn rust_editor_vtable<E, D, F>() -> subversion_sys::svn_delta_editor_t
where
    E: Editor<RootEditor = D> + 'static,
    D: DirectoryEditor<SubDirectory = D, File = F> + 'static,
    F: FileEditor + 'static,
{
    subversion_sys::svn_delta_editor_t {
        set_target_revision: Some(rust_editor_set_target_revision::<E>),
        open_root: Some(rust_editor_open_root::<E, D>),
        delete_entry: Some(rust_dir_delete_entry::<D>),
        add_directory: Some(rust_dir_add_directory::<D>),
        open_directory: Some(rust_dir_open_directory::<D>),
        change_dir_prop: Some(rust_dir_change_prop::<D>),
        close_directory: Some(rust_dir_close::<D>),
        absent_directory: Some(rust_dir_absent_directory::<D>),
        add_file: Some(rust_dir_add_file::<D, F>),
        open_file: Some(rust_dir_open_file::<D, F>),
        apply_textdelta: Some(rust_file_apply_textdelta::<F>),
        change_file_prop: Some(rust_file_change_prop::<F>),
        close_file: Some(rust_file_close::<F>),
        absent_file: Some(rust_dir_absent_file::<D>),
        close_edit: Some(rust_editor_close::<E>),
        abort_edit: Some(rust_editor_abort::<E>),
        // apply_textdelta_stream requires bridging svn_txdelta_stream_t creation
        // via a C open-func callback, which is significantly more complex.
        // Leaving as None falls back to apply_textdelta.
        apply_textdelta_stream: None,
    }
}

impl WrapEditor<'static> {
    /// Wrap a custom Rust editor implementation for use with C APIs.
    ///
    /// This creates a WrapEditor that forwards calls from C to your Rust editor.
    /// The editor must be homogeneous - all subdirectories must have the same type.
    ///
    /// # Type Parameters
    ///
    /// * `E` - The Editor implementation
    /// * `D` - The DirectoryEditor type (must be E::RootEditor)
    /// * `F` - The FileEditor type (must be D::File)
    ///
    /// # Requirements
    ///
    /// * All subdirectories must have the same type: `D::SubDirectory = D`
    /// * This ensures we can reconstruct types from void pointers in C callbacks
    ///
    /// # Example
    ///
    /// ```ignore
    /// let my_editor = MyCustomEditor::new();
    /// let mut wrap_editor = WrapEditor::from_rust_editor(my_editor);
    /// session.do_update(..., &mut wrap_editor)?;
    /// ```
    pub fn from_rust_editor<E, D, F>(editor: E) -> Self
    where
        E: Editor<RootEditor = D> + 'static,
        D: DirectoryEditor<SubDirectory = D, File = F> + 'static,
        F: FileEditor + 'static,
    {
        let pool = Pool::new();

        // Box the editor and get raw pointer
        let editor_box = Box::new(editor);
        let editor_ptr = Box::into_raw(editor_box) as *mut std::ffi::c_void;

        // Create dropper function for the editor
        let dropper: DropperFn = |ptr| unsafe {
            let _ = Box::from_raw(ptr as *mut E);
        };

        // Generate vtable and allocate in pool
        let vtable = rust_editor_vtable::<E, D, F>();
        let vtable_ptr = unsafe {
            let ptr = apr_sys::apr_palloc(
                pool.as_mut_ptr(),
                std::mem::size_of::<subversion_sys::svn_delta_editor_t>(),
            ) as *mut subversion_sys::svn_delta_editor_t;
            *ptr = vtable;
            ptr as *const subversion_sys::svn_delta_editor_t
        };

        WrapEditor {
            editor: vtable_ptr,
            baton: editor_ptr,
            _pool: apr::PoolHandle::owned(pool),
            callback_batons: vec![(editor_ptr, dropper)],
        }
    }
}

/// Txdelta stream for generating delta windows
pub struct TxDeltaStream {
    ptr: *mut subversion_sys::svn_txdelta_stream_t,
    _pool: apr::Pool<'static>,
}

impl TxDeltaStream {
    /// Create a new txdelta stream between source and target
    pub fn new(source: &mut crate::io::Stream, target: &mut crate::io::Stream) -> Self {
        let pool = apr::Pool::new();

        let mut stream_ptr: *mut subversion_sys::svn_txdelta_stream_t = std::ptr::null_mut();

        unsafe {
            subversion_sys::svn_txdelta(
                &mut stream_ptr,
                source.as_mut_ptr(),
                target.as_mut_ptr(),
                pool.as_mut_ptr(),
            );
        }

        Self {
            ptr: stream_ptr,
            _pool: pool,
        }
    }

    /// Create from raw pointer
    pub(crate) fn from_raw(
        ptr: *mut subversion_sys::svn_txdelta_stream_t,
        pool: apr::Pool<'static>,
    ) -> Self {
        Self { ptr, _pool: pool }
    }

    /// Get the next window from the stream
    pub fn next_window(&mut self) -> Result<Option<TxDeltaWindow>, crate::Error<'_>> {
        let mut window_ptr: *mut subversion_sys::svn_txdelta_window_t = std::ptr::null_mut();

        let ret = unsafe {
            subversion_sys::svn_txdelta_next_window(
                &mut window_ptr,
                self.ptr,
                self._pool.as_mut_ptr(),
            )
        };

        crate::svn_result(ret)?;

        if window_ptr.is_null() {
            Ok(None)
        } else {
            // Create a new TxDeltaWindow
            let mut window = TxDeltaWindow::new();
            window.ptr = window_ptr;
            Ok(Some(window))
        }
    }

    /// Get the MD5 digest of the target data
    pub fn md5_digest(&self) -> Vec<u8> {
        let digest_ptr = unsafe { subversion_sys::svn_txdelta_md5_digest(self.ptr) };

        if digest_ptr.is_null() {
            Vec::new()
        } else {
            let mut digest = Vec::with_capacity(16);
            unsafe {
                for i in 0..16 {
                    digest.push(*digest_ptr.add(i));
                }
            }
            digest
        }
    }
}

/// Create a txdelta between source and target streams
pub fn txdelta(source: &mut crate::io::Stream, target: &mut crate::io::Stream) -> TxDeltaStream {
    let pool = apr::Pool::new();

    let mut stream_ptr: *mut subversion_sys::svn_txdelta_stream_t = std::ptr::null_mut();

    unsafe {
        subversion_sys::svn_txdelta(
            &mut stream_ptr,
            source.as_mut_ptr(),
            target.as_mut_ptr(),
            pool.as_mut_ptr(),
        );
    }

    TxDeltaStream {
        ptr: stream_ptr,
        _pool: pool,
    }
}

/// Create a txdelta between source and target streams with checksums
pub fn txdelta2(
    source: &mut crate::io::Stream,
    target: &mut crate::io::Stream,
    calculate_checksum: bool,
) -> TxDeltaStream {
    let pool = apr::Pool::new();

    let mut stream_ptr: *mut subversion_sys::svn_txdelta_stream_t = std::ptr::null_mut();

    unsafe {
        subversion_sys::svn_txdelta2(
            &mut stream_ptr,
            source.as_mut_ptr(),
            target.as_mut_ptr(),
            calculate_checksum as i32,
            pool.as_mut_ptr(),
        );
    }

    TxDeltaStream {
        ptr: stream_ptr,
        _pool: pool,
    }
}

/// Send a string through a delta handler
///
/// # Safety
///
/// The caller must ensure that `handler` and `handler_baton` are valid and compatible.
/// The handler function pointer must be safe to call with the provided baton.
pub unsafe fn send_string(
    string: &str,
    handler: subversion_sys::svn_txdelta_window_handler_t,
    handler_baton: *mut std::ffi::c_void,
) -> Result<(), crate::Error<'_>> {
    let pool = apr::Pool::new();

    let svn_string = unsafe {
        subversion_sys::svn_string_ncreate(
            string.as_ptr() as *const i8,
            string.len(),
            pool.as_mut_ptr(),
        )
    };

    let ret = unsafe {
        subversion_sys::svn_txdelta_send_string(
            svn_string,
            handler,
            handler_baton,
            pool.as_mut_ptr(),
        )
    };

    crate::svn_result(ret)
}

/// Send a stream through a delta handler
///
/// # Safety
///
/// The caller must ensure that `handler` and `handler_baton` are valid and compatible.
/// The handler function pointer must be safe to call with the provided baton.
pub unsafe fn send_stream(
    stream: &mut crate::io::Stream,
    handler: subversion_sys::svn_txdelta_window_handler_t,
    handler_baton: *mut std::ffi::c_void,
    digest: Option<&mut [u8; 16]>,
) -> Result<(), crate::Error<'static>> {
    let pool = apr::Pool::new();

    let digest_ptr = digest.map_or(std::ptr::null_mut(), |d| d.as_mut_ptr());

    let ret = unsafe {
        subversion_sys::svn_txdelta_send_stream(
            stream.as_mut_ptr(),
            handler,
            handler_baton,
            digest_ptr,
            pool.as_mut_ptr(),
        )
    };

    crate::svn_result(ret)
}

/// Send a txdelta stream through a delta handler
///
/// # Safety
///
/// The caller must ensure that `handler` and `handler_baton` are valid and compatible.
/// The handler function pointer must be safe to call with the provided baton.
pub unsafe fn send_txstream(
    txstream: &mut TxDeltaStream,
    handler: subversion_sys::svn_txdelta_window_handler_t,
    handler_baton: *mut std::ffi::c_void,
) -> Result<(), crate::Error<'_>> {
    let pool = apr::Pool::new();

    let ret = unsafe {
        subversion_sys::svn_txdelta_send_txstream(
            txstream.ptr,
            handler,
            handler_baton,
            pool.as_mut_ptr(),
        )
    };

    crate::svn_result(ret)
}

/// Send raw byte contents through a delta handler
///
/// This is effectively a 'copy' operation, resulting in delta windows that
/// make the target equivalent to the provided bytes.
///
/// # Safety
///
/// The caller must ensure that `handler` and `handler_baton` are valid and compatible.
/// The handler function pointer must be safe to call with the provided baton.
///
/// Wraps `svn_txdelta_send_contents`.
pub unsafe fn send_contents(
    contents: &[u8],
    handler: subversion_sys::svn_txdelta_window_handler_t,
    handler_baton: *mut std::ffi::c_void,
) -> Result<(), crate::Error<'static>> {
    let pool = apr::Pool::new();

    let ret = unsafe {
        subversion_sys::svn_txdelta_send_contents(
            contents.as_ptr(),
            contents.len(),
            handler,
            handler_baton,
            pool.as_mut_ptr(),
        )
    };

    crate::svn_result(ret)
}

/// Create a push-based delta target stream
///
/// Returns a writable stream which, when fed target data, will send delta windows
/// to the provided handler that transform the source data to the target data.
///
/// # Safety
///
/// The caller must ensure that `handler` and `handler_baton` are valid and compatible.
///
/// Wraps `svn_txdelta_target_push`.
pub unsafe fn target_push(
    handler: subversion_sys::svn_txdelta_window_handler_t,
    handler_baton: *mut std::ffi::c_void,
    source: &mut crate::io::Stream,
) -> crate::io::Stream {
    let pool = apr::Pool::new();

    let stream_ptr = unsafe {
        subversion_sys::svn_txdelta_target_push(
            handler,
            handler_baton,
            source.as_mut_ptr(),
            pool.as_mut_ptr(),
        )
    };

    crate::io::Stream::from_ptr(stream_ptr, pool)
}

/// Convert a txdelta stream to an svndiff-format stream
///
/// Returns a readable stream that will produce svndiff-format data when read.
/// The data will be in the specified svndiff version format with the given
/// compression level.
///
/// # Arguments
///
/// * `txstream` - The txdelta stream to convert
/// * `svndiff_version` - The svndiff format version (typically 0, 1, or 2)
/// * `compression_level` - Compression level (0-9, where 0 is no compression)
///
/// Wraps `svn_txdelta_to_svndiff_stream`.
pub fn to_svndiff_stream(
    txstream: &mut TxDeltaStream,
    svndiff_version: i32,
    compression_level: i32,
) -> crate::io::Stream {
    let pool = apr::Pool::new();

    let stream_ptr = unsafe {
        subversion_sys::svn_txdelta_to_svndiff_stream(
            txstream.ptr,
            svndiff_version,
            compression_level,
            pool.as_mut_ptr(),
        )
    };

    crate::io::Stream::from_ptr(stream_ptr, pool)
}

/// Read one delta window in svndiff format from a stream
///
/// Reads and parses a single delta window from the stream. The caller must
/// strip off the four-byte 'SVN\x00' header before reading the first window.
///
/// # Arguments
///
/// * `stream` - The stream to read from
/// * `svndiff_version` - The svndiff format version (from the header's 4th byte)
///
/// Returns `Ok(Some(window))` if a window was read, `Ok(None)` if at end of stream.
///
/// Wraps `svn_txdelta_read_svndiff_window`.
pub fn read_svndiff_window(
    stream: &mut crate::io::Stream,
    svndiff_version: i32,
) -> Result<Option<TxDeltaWindow>, crate::Error<'static>> {
    let pool = apr::Pool::new();

    let mut window_ptr: *mut subversion_sys::svn_txdelta_window_t = std::ptr::null_mut();

    let ret = unsafe {
        subversion_sys::svn_txdelta_read_svndiff_window(
            &mut window_ptr,
            stream.as_mut_ptr(),
            svndiff_version,
            pool.as_mut_ptr(),
        )
    };

    crate::svn_result(ret)?;

    if window_ptr.is_null() {
        Ok(None)
    } else {
        let mut window = TxDeltaWindow::new();
        window.ptr = window_ptr;
        Ok(Some(window))
    }
}

/// Apply a delta, returning the resulting stream and handler
pub fn apply(
    source: &mut crate::io::Stream,
    target: &mut crate::io::Stream,
) -> Result<
    (
        subversion_sys::svn_txdelta_window_handler_t,
        *mut std::ffi::c_void,
    ),
    crate::Error<'static>,
> {
    let pool = apr::Pool::new();

    let mut handler: subversion_sys::svn_txdelta_window_handler_t = None;
    let mut handler_baton: *mut std::ffi::c_void = std::ptr::null_mut();

    unsafe {
        subversion_sys::svn_txdelta_apply(
            source.as_mut_ptr(),
            target.as_mut_ptr(),
            std::ptr::null_mut(), // result_digest
            std::ptr::null_mut(), // error_info
            pool.as_mut_ptr(),
            &mut handler,
            &mut handler_baton,
        );
    }

    // svn_txdelta_apply doesn't return an error directly
    Ok((handler, handler_baton))
}

/// Create a stream that parses svndiff data
///
/// Returns a writable stream. When svndiff data is written to this stream,
/// it will be parsed and the provided handler will be called for each delta window.
///
/// The handler must outlive the returned stream.
pub fn parse_svndiff<F>(handler: &mut F) -> Result<crate::io::Stream, crate::Error<'_>>
where
    F: FnMut(&mut TxDeltaWindow) -> Result<(), crate::Error>,
{
    let pool = apr::Pool::new();

    // Get a raw pointer to the handler
    let handler_ptr = handler as *mut F as *mut std::ffi::c_void;

    // Create a C-compatible wrapper function
    extern "C" fn window_handler_wrapper<F>(
        window: *mut subversion_sys::svn_txdelta_window_t,
        baton: *mut std::ffi::c_void,
    ) -> *mut subversion_sys::svn_error_t
    where
        F: FnMut(&mut TxDeltaWindow) -> Result<(), crate::Error>,
    {
        unsafe {
            let handler = &mut *(baton as *mut F);

            if window.is_null() {
                // NULL window means end of stream
                return std::ptr::null_mut();
            }

            // Wrap the window
            let mut tx_window = TxDeltaWindow::new();
            tx_window.ptr = window;

            let result = handler(&mut tx_window);
            match result {
                Ok(()) => std::ptr::null_mut(),
                Err(err) => err.as_ptr() as *mut subversion_sys::svn_error_t,
            }
        }
    }

    let stream_ptr = unsafe {
        subversion_sys::svn_txdelta_parse_svndiff(
            Some(window_handler_wrapper::<F>),
            handler_ptr,
            1, // error_on_early_close
            pool.as_mut_ptr(),
        )
    };

    if stream_ptr.is_null() {
        return Err(crate::Error::from_raw(unsafe {
            subversion_sys::svn_error_create(
                subversion_sys::svn_errno_t_SVN_ERR_DELTA_MD5_CHECKSUM_ABSENT as i32,
                std::ptr::null_mut(),
                c"Failed to create svndiff parser".as_ptr(),
            )
        })
        .unwrap_err());
    }

    Ok(crate::io::Stream::from_ptr(stream_ptr, pool))
}

/// Compute the delta between `source` and `target` streams and write it as
/// svndiff data to `output`.
///
/// Combines `svn_txdelta_to_svndiff3` (to create the svndiff encoder) with
/// `svn_txdelta_run` (to drive the computation).
///
/// * `source` — the base content; pass `None` to produce a "full text" delta
///   (i.e. the entire `target` is encoded as a single insert).
/// * `target` — the new content.
/// * `output` — writable stream that receives the svndiff-encoded bytes.
/// * `svndiff_version` — wire format version: `0`, `1`, or `2`.
/// * `compression_level` — zlib compression level `0`–`9`, or `-1` for the
///   library default.
/// * `checksum_kind` — kind of checksum to compute over the target bytes.
///   The resulting checksum is returned.
/// * `cancel_func` — optional cancellation callback.
///
/// Returns the target checksum, or `None` if the C library returned a null
/// checksum pointer.
///
/// Wraps `svn_txdelta_to_svndiff3` + `svn_txdelta_run`.
pub fn svndiff_from_streams(
    source: Option<&mut crate::io::Stream>,
    target: &mut crate::io::Stream,
    output: &mut crate::io::Stream,
    svndiff_version: i32,
    compression_level: i32,
    checksum_kind: crate::ChecksumKind,
    cancel_func: Option<Box<dyn Fn() -> Result<(), crate::Error<'static>>>>,
) -> Result<Option<crate::Checksum<'static>>, crate::Error<'static>> {
    // Use a single pool for both the svndiff encoder state and the delta run,
    // so the handler_baton allocated by svn_txdelta_to_svndiff3 stays alive
    // until after svn_txdelta_run returns.
    let pool = apr::Pool::new();

    let mut handler: subversion_sys::svn_txdelta_window_handler_t = None;
    let mut handler_baton: *mut std::ffi::c_void = std::ptr::null_mut();

    unsafe {
        subversion_sys::svn_txdelta_to_svndiff3(
            &mut handler,
            &mut handler_baton,
            output.as_mut_ptr(),
            svndiff_version,
            compression_level,
            pool.as_mut_ptr(),
        );
    }

    // Prepare source stream (empty stream if caller passed None).
    let empty_source_pool = apr::Pool::new();
    let mut empty_source: Option<crate::io::Stream>;
    let source_ptr: *mut subversion_sys::svn_stream_t = match source {
        Some(s) => s.as_mut_ptr(),
        None => {
            let s = unsafe {
                crate::io::Stream::from_ptr(
                    subversion_sys::svn_stream_empty(empty_source_pool.as_mut_ptr()),
                    empty_source_pool,
                )
            };
            empty_source = Some(s);
            empty_source.as_mut().unwrap().as_mut_ptr()
        }
    };

    let has_cancel = cancel_func.is_some();
    let cancel_baton = cancel_func
        .map(|f| Box::into_raw(Box::new(f)) as *mut std::ffi::c_void)
        .unwrap_or(std::ptr::null_mut());

    let mut checksum: *mut subversion_sys::svn_checksum_t = std::ptr::null_mut();

    let err = unsafe {
        subversion_sys::svn_txdelta_run(
            source_ptr,
            target.as_mut_ptr(),
            handler,
            handler_baton,
            checksum_kind.into(),
            &mut checksum,
            if has_cancel {
                Some(crate::wrap_cancel_func)
            } else {
                None
            },
            cancel_baton,
            pool.as_mut_ptr(), // result_pool
            pool.as_mut_ptr(), // scratch_pool (same is fine; C docs allow it)
        )
    };

    if has_cancel && !cancel_baton.is_null() {
        unsafe {
            drop(Box::from_raw(
                cancel_baton as *mut Box<dyn Fn() -> Result<(), crate::Error<'static>>>,
            ));
        }
    }

    crate::svn_result(err)?;

    let result_checksum = if checksum.is_null() {
        None
    } else {
        Some(crate::Checksum::from_raw(checksum))
    };

    Ok(result_checksum)
}

/// Drive an editor with a set of paths
///
/// Calls the provided callback function for each path in the sorted order, allowing
/// the callback to perform editor operations on that path. This is useful for
/// efficiently driving an editor with a specific set of paths.
///
/// The callback receives the parent directory baton and must return the directory
/// baton for the path if it opens/adds a directory, or None if it's a file or deleted.
///
/// # Arguments
///
/// * `editor` - The editor to drive
/// * `paths` - Array of paths to process (will be sorted)
/// * `callback` - Function called for each path
///
/// # Safety
///
/// The callback must properly manage batons and close files immediately or delay
/// closing until this function returns.
///
/// Wraps `svn_delta_path_driver`.
pub unsafe fn path_driver<F>(
    editor: &WrapEditor,
    paths: &[&str],
    callback: F,
) -> Result<(), crate::Error<'static>>
where
    F: FnMut(
        Option<*mut std::ffi::c_void>,
        &str,
    ) -> Result<Option<*mut std::ffi::c_void>, crate::Error<'static>>,
{
    let pool = apr::Pool::new();

    // Convert paths to APR array
    let paths_array = unsafe {
        apr_sys::apr_array_make(
            pool.as_mut_ptr(),
            paths.len() as i32,
            std::mem::size_of::<*const i8>() as i32,
        )
    };

    for path in paths {
        let path_cstr = std::ffi::CString::new(*path)?;
        unsafe {
            let elt = apr_sys::apr_array_push(paths_array) as *mut *const i8;
            *elt = apr_sys::apr_pstrdup(pool.as_mut_ptr(), path_cstr.as_ptr());
        }
    }

    // Create callback baton
    let mut boxed_callback: Box<
        Box<
            dyn FnMut(
                Option<*mut std::ffi::c_void>,
                &str,
            ) -> Result<Option<*mut std::ffi::c_void>, crate::Error<'static>>,
        >,
    > = Box::new(Box::new(callback));
    let baton_ptr = &mut *boxed_callback as *mut _ as *mut std::ffi::c_void;

    extern "C" fn trampoline(
        dir_baton: *mut *mut std::ffi::c_void,
        parent_baton: *mut std::ffi::c_void,
        callback_baton: *mut std::ffi::c_void,
        path: *const i8,
        _pool: *mut apr_sys::apr_pool_t,
    ) -> *mut subversion_sys::svn_error_t {
        unsafe {
            let callback = &mut *(callback_baton
                as *mut Box<
                    dyn FnMut(
                        Option<*mut std::ffi::c_void>,
                        &str,
                    )
                        -> Result<Option<*mut std::ffi::c_void>, crate::Error<'static>>,
                >);

            let path_str = std::ffi::CStr::from_ptr(path).to_str().unwrap();
            let parent = if parent_baton.is_null() {
                None
            } else {
                Some(parent_baton)
            };

            match callback(parent, path_str) {
                Ok(result) => {
                    if !dir_baton.is_null() {
                        *dir_baton = result.unwrap_or(std::ptr::null_mut());
                    }
                    std::ptr::null_mut()
                }
                Err(e) => e.into_raw(),
            }
        }
    }

    let ret = unsafe {
        subversion_sys::svn_delta_path_driver(
            editor.editor,
            editor.baton,
            -1, // SVN_INVALID_REVNUM
            paths_array,
            Some(trampoline),
            baton_ptr,
            pool.as_mut_ptr(),
        )
    };

    crate::svn_result(ret)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert_eq!(super::version().major(), 1);
    }

    // Simple test editor to use in our tests
    use std::cell::RefCell;
    use std::rc::Rc;

    struct TestEditor {
        operations: Rc<RefCell<Vec<String>>>,
    }

    impl Editor for TestEditor {
        type RootEditor = TestDirectoryEditor;
        fn set_target_revision(
            &mut self,
            revision: Option<crate::Revnum>,
        ) -> Result<(), crate::Error<'_>> {
            self.operations
                .borrow_mut()
                .push(format!("set_target_revision({:?})", revision.map(|r| r.0)));
            Ok(())
        }

        fn open_root(
            &mut self,
            base_revision: Option<crate::Revnum>,
        ) -> Result<TestDirectoryEditor, crate::Error<'_>> {
            self.operations
                .borrow_mut()
                .push(format!("open_root({:?})", base_revision.map(|r| r.0)));
            Ok(TestDirectoryEditor {
                operations: self.operations.clone(),
            })
        }

        fn close(&mut self) -> Result<(), crate::Error<'_>> {
            self.operations.borrow_mut().push("close".to_string());
            Ok(())
        }

        fn abort(&mut self) -> Result<(), crate::Error<'_>> {
            self.operations.borrow_mut().push("abort".to_string());
            Ok(())
        }
    }

    struct TestDirectoryEditor {
        operations: Rc<RefCell<Vec<String>>>,
    }

    impl DirectoryEditor for TestDirectoryEditor {
        type SubDirectory = TestDirectoryEditor;
        type File = TestFileEditor;
        fn delete_entry(
            &mut self,
            path: &str,
            revision: Option<crate::Revnum>,
        ) -> Result<(), crate::Error<'_>> {
            self.operations.borrow_mut().push(format!(
                "delete_entry({}, {:?})",
                path,
                revision.map(|r| r.0)
            ));
            Ok(())
        }

        fn add_directory(
            &mut self,
            path: &str,
            copyfrom: Option<(&str, crate::Revnum)>,
        ) -> Result<TestDirectoryEditor, crate::Error<'_>> {
            self.operations.borrow_mut().push(format!(
                "add_directory({}, {:?})",
                path,
                copyfrom.map(|(p, r)| (p, r.0))
            ));
            Ok(TestDirectoryEditor {
                operations: self.operations.clone(),
            })
        }

        fn open_directory(
            &mut self,
            path: &str,
            base_revision: Option<crate::Revnum>,
        ) -> Result<TestDirectoryEditor, crate::Error<'_>> {
            self.operations.borrow_mut().push(format!(
                "open_directory({}, {:?})",
                path,
                base_revision.map(|r| r.0)
            ));
            Ok(TestDirectoryEditor {
                operations: self.operations.clone(),
            })
        }

        fn change_prop(
            &mut self,
            name: &str,
            value: Option<&[u8]>,
        ) -> Result<(), crate::Error<'_>> {
            self.operations
                .borrow_mut()
                .push(format!("change_prop({}, {:?})", name, value));
            Ok(())
        }

        fn close(&mut self) -> Result<(), crate::Error<'_>> {
            self.operations
                .borrow_mut()
                .push("close_directory".to_string());
            Ok(())
        }

        fn absent_directory(&mut self, path: &str) -> Result<(), crate::Error<'_>> {
            self.operations
                .borrow_mut()
                .push(format!("absent_directory({})", path));
            Ok(())
        }

        fn add_file(
            &mut self,
            path: &str,
            copyfrom: Option<(&str, crate::Revnum)>,
        ) -> Result<TestFileEditor, crate::Error<'_>> {
            self.operations.borrow_mut().push(format!(
                "add_file({}, {:?})",
                path,
                copyfrom.map(|(p, r)| (p, r.0))
            ));
            Ok(TestFileEditor {
                operations: self.operations.clone(),
            })
        }

        fn open_file(
            &mut self,
            path: &str,
            base_revision: Option<crate::Revnum>,
        ) -> Result<TestFileEditor, crate::Error<'_>> {
            self.operations.borrow_mut().push(format!(
                "open_file({}, {:?})",
                path,
                base_revision.map(|r| r.0)
            ));
            Ok(TestFileEditor {
                operations: self.operations.clone(),
            })
        }

        fn absent_file(&mut self, path: &str) -> Result<(), crate::Error<'_>> {
            self.operations
                .borrow_mut()
                .push(format!("absent_file({})", path));
            Ok(())
        }
    }

    struct TestFileEditor {
        operations: Rc<RefCell<Vec<String>>>,
    }

    impl FileEditor for TestFileEditor {
        fn apply_textdelta(
            &mut self,
            base_checksum: Option<&str>,
        ) -> Result<
            Box<dyn for<'b> Fn(&'b mut TxDeltaWindow) -> Result<(), crate::Error<'static>>>,
            crate::Error<'static>,
        > {
            self.operations
                .borrow_mut()
                .push(format!("apply_textdelta({:?})", base_checksum));
            Ok(Box::new(|_window| Ok(())))
        }

        fn change_prop(
            &mut self,
            name: &str,
            value: Option<&[u8]>,
        ) -> Result<(), crate::Error<'static>> {
            self.operations
                .borrow_mut()
                .push(format!("change_file_prop({}, {:?})", name, value));
            Ok(())
        }

        fn close(&mut self, text_checksum: Option<&str>) -> Result<(), crate::Error<'static>> {
            self.operations
                .borrow_mut()
                .push(format!("close_file({:?})", text_checksum));
            Ok(())
        }

        fn apply_textdelta_stream(
            &mut self,
            base_checksum: Option<&str>,
            open_stream: Box<dyn FnOnce() -> Result<TxDeltaStream, crate::Error<'static>>>,
        ) -> Result<(), crate::Error<'static>> {
            self.operations
                .borrow_mut()
                .push(format!("apply_textdelta_stream({:?})", base_checksum));
            // Call the open_stream to verify it works
            let _stream = open_stream()?;
            Ok(())
        }
    }

    #[test]
    fn test_txdelta_stream() {
        // Create source and target streams from strings
        let source_data = "Hello, world!";
        let target_data = "Hello, Rust world!";

        let mut source_buf = crate::io::StringBuf::try_from(source_data).unwrap();
        let mut target_buf = crate::io::StringBuf::try_from(target_data).unwrap();
        let mut source_stream = crate::io::Stream::from_stringbuf(&mut source_buf);
        let mut target_stream = crate::io::Stream::from_stringbuf(&mut target_buf);

        // Create a txdelta stream
        let mut txstream = TxDeltaStream::new(&mut source_stream, &mut target_stream);

        // Get windows from the stream
        let mut window_count = 0;
        while let Ok(Some(_window)) = txstream.next_window() {
            window_count += 1;
            // Safety check to prevent infinite loop
            if window_count > 100 {
                panic!("Too many windows!");
            }
        }

        // We should have gotten at least one window
        assert!(
            window_count > 0,
            "Should have gotten at least one delta window"
        );
    }

    #[test]
    fn test_txdelta_functions() {
        // Test txdelta function
        let source_data = "Original content";
        let target_data = "Modified content";

        let mut source_buf = crate::io::StringBuf::try_from(source_data).unwrap();
        let mut target_buf = crate::io::StringBuf::try_from(target_data).unwrap();
        let mut source_stream = crate::io::Stream::from_stringbuf(&mut source_buf);
        let mut target_stream = crate::io::Stream::from_stringbuf(&mut target_buf);

        let mut txstream = txdelta(&mut source_stream, &mut target_stream);

        // Should be able to get at least one window
        let window = txstream.next_window();
        assert!(window.is_ok(), "Failed to get window: {:?}", window.err());
    }

    #[test]
    fn test_txdelta2_with_checksum() {
        // Test txdelta2 with checksum calculation
        let source_data = "Source data";
        let target_data = "Target data";

        let mut source_buf = crate::io::StringBuf::try_from(source_data).unwrap();
        let mut target_buf = crate::io::StringBuf::try_from(target_data).unwrap();
        let mut source_stream = crate::io::Stream::from_stringbuf(&mut source_buf);
        let mut target_stream = crate::io::Stream::from_stringbuf(&mut target_buf);

        let mut txstream = txdelta2(&mut source_stream, &mut target_stream, true);

        // Process all windows until we get NULL (end of stream)
        loop {
            let window = txstream.next_window().unwrap();
            if window.is_none() {
                break;
            }
        }

        // Now get the MD5 digest after processing all windows
        let digest = txstream.md5_digest();
        let expected_digest = [
            0x4a, 0xa8, 0x53, 0x7f, 0x53, 0xf3, 0x29, 0x99, 0x13, 0x78, 0x90, 0x52, 0xa9, 0x2c,
            0xd0, 0x73,
        ];
        assert_eq!(digest, expected_digest, "MD5 digest mismatch");
    }

    #[test]
    fn test_apply_delta() {
        // Test applying a delta
        let source_data = "Source content";
        let target_data = "";

        let mut source_buf = crate::io::StringBuf::try_from(source_data).unwrap();
        let mut target_buf = crate::io::StringBuf::try_from(target_data).unwrap();
        let mut source_stream = crate::io::Stream::from_stringbuf(&mut source_buf);
        let mut target_stream = crate::io::Stream::from_stringbuf(&mut target_buf);

        // Apply should give us a handler and baton
        let result = apply(&mut source_stream, &mut target_stream);
        assert!(result.is_ok(), "Failed to apply delta: {:?}", result.err());

        let (handler, _baton) = result.unwrap();
        assert!(handler.is_some(), "Handler should not be None");
    }

    #[test]
    fn test_apply_textdelta_stream_default() {
        // Test the default implementation returns an error
        struct MinimalFileEditor;

        impl FileEditor for MinimalFileEditor {
            fn apply_textdelta(
                &mut self,
                _base_checksum: Option<&str>,
            ) -> Result<
                Box<dyn for<'a> Fn(&'a mut TxDeltaWindow) -> Result<(), crate::Error<'static>>>,
                crate::Error<'static>,
            > {
                Ok(Box::new(|_| Ok(())))
            }

            fn change_prop(
                &mut self,
                _name: &str,
                _value: Option<&[u8]>,
            ) -> Result<(), crate::Error<'static>> {
                Ok(())
            }

            fn close(&mut self, _text_checksum: Option<&str>) -> Result<(), crate::Error<'static>> {
                Ok(())
            }
        }

        let mut editor = MinimalFileEditor;
        let open_stream: Box<dyn FnOnce() -> Result<TxDeltaStream, crate::Error<'static>>> =
            Box::new(|| Err(crate::Error::from_message("Not called")));

        // Default implementation should return an error
        let result = editor.apply_textdelta_stream(None, open_stream);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("not supported"));
    }

    #[test]
    fn test_apply_textdelta_stream_custom() {
        // Test a custom implementation that accepts the stream
        let operations: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let mut editor = TestFileEditor {
            operations: operations.clone(),
        };

        // Create a mock open_stream that returns an error (since we can't easily create a real stream)
        let open_stream: Box<dyn FnOnce() -> Result<TxDeltaStream, crate::Error<'static>>> =
            Box::new(|| Err(crate::Error::from_message("Mock stream error")));

        let result = editor.apply_textdelta_stream(Some("abc123"), open_stream);
        // Should fail because our mock returns an error
        assert!(result.is_err());

        // But the operation should have been recorded
        let ops = operations.borrow();
        assert!(ops.contains(&"apply_textdelta_stream(Some(\"abc123\"))".to_string()));
    }

    #[test]
    fn test_svndiff_from_streams_full_text() {
        // A full-text delta (no source) should produce svndiff data whose
        // first 4 bytes are the svndiff magic: 'S', 'V', 'N', version.
        let target_data = b"Hello, world!\n";

        let mut target_stream = crate::io::Stream::from(&target_data[..]);
        let mut output = crate::io::Stream::buffered();

        let checksum = svndiff_from_streams(
            None, // no source → full-text delta
            &mut target_stream,
            &mut output,
            0,  // svndiff version 0
            -1, // default compression
            crate::ChecksumKind::MD5,
            None,
        )
        .unwrap();

        // The result should be Some checksum
        assert!(checksum.is_some(), "expected a checksum");

        // Read the output and verify it starts with the svndiff magic bytes.
        let mut buf = [0u8; 4];
        // Re-open the output for reading by getting its contents.
        let mut read_buf = vec![0u8; 1024];
        let n = output.read_full(&mut read_buf).unwrap();
        assert!(n >= 4, "svndiff output should be at least 4 bytes, got {n}");
        buf.copy_from_slice(&read_buf[..4]);
        assert_eq!(&buf[..3], b"SVN", "svndiff magic bytes missing");
        assert_eq!(buf[3], 0, "expected svndiff version 0");
    }

    #[test]
    fn test_svndiff_from_streams_with_source() {
        // A delta with a source should still produce valid svndiff output.
        let source_data = b"Hello, world!\n";
        let target_data = b"Hello, Rust!\n";

        let mut source_stream = crate::io::Stream::from(&source_data[..]);
        let mut target_stream = crate::io::Stream::from(&target_data[..]);
        let mut output = crate::io::Stream::buffered();

        let checksum = svndiff_from_streams(
            Some(&mut source_stream),
            &mut target_stream,
            &mut output,
            0,
            -1,
            crate::ChecksumKind::MD5,
            None,
        )
        .unwrap();

        assert!(checksum.is_some());

        let mut read_buf = vec![0u8; 1024];
        let n = output.read_full(&mut read_buf).unwrap();
        assert!(n >= 4, "svndiff output should be at least 4 bytes");
        assert_eq!(&read_buf[..3], b"SVN");
    }

    #[test]
    fn test_editor_set_target_revision_value() {
        let operations = Rc::new(RefCell::new(Vec::new()));
        let test_editor = TestEditor {
            operations: operations.clone(),
        };
        let mut wrap_editor = WrapEditor::from_rust_editor(test_editor);

        // Test with None
        wrap_editor.set_target_revision(None).unwrap();
        // Test with Some
        wrap_editor
            .set_target_revision(Some(crate::Revnum(100)))
            .unwrap();

        wrap_editor.close().unwrap();

        let ops = operations.borrow();
        assert_eq!(ops[0], "set_target_revision(None)");
        assert_eq!(ops[1], "set_target_revision(Some(100))");
        // Verify the revision value matters (catches delete - mutation)
        assert!(ops[1].contains("100"));
    }

    #[test]
    fn test_editor_delete_entry_revision_matters() {
        let operations = Rc::new(RefCell::new(Vec::new()));
        let test_editor = TestEditor {
            operations: operations.clone(),
        };
        let mut wrap_editor = WrapEditor::from_rust_editor(test_editor);
        let mut root = wrap_editor.open_root(None).unwrap();

        // Delete with specific revision
        root.delete_entry("file1.txt", Some(crate::Revnum(50)))
            .unwrap();
        // Delete without revision
        root.delete_entry("file2.txt", None).unwrap();

        root.close().unwrap();
        wrap_editor.close().unwrap();

        let ops = operations.borrow();
        // Verify revision values are preserved
        assert!(ops
            .iter()
            .any(|s| s.contains("file1.txt") && s.contains("50")));
        assert!(ops
            .iter()
            .any(|s| s.contains("file2.txt") && s.contains("None")));
    }

    #[test]
    fn test_apply_textdelta_through_c_vtable() {
        // Exercise the apply_textdelta trampoline through from_rust_editor's C vtable
        let operations = Rc::new(RefCell::new(Vec::new()));
        let test_editor = TestEditor {
            operations: operations.clone(),
        };
        let mut wrap_editor = WrapEditor::from_rust_editor(test_editor);
        let mut root = wrap_editor.open_root(None).unwrap();
        let mut file = root.add_file("test.txt", None).unwrap();

        // Call apply_textdelta through the C vtable roundtrip
        let _handler = file.apply_textdelta(Some("abc123")).unwrap();

        // The handler is a closure returned by TestFileEditor::apply_textdelta
        // which was invoked via the C trampoline rust_file_apply_textdelta
        // Verify the operation was recorded
        let ops = operations.borrow();
        assert!(
            ops.iter().any(|s| s == "apply_textdelta(Some(\"abc123\"))"),
            "apply_textdelta should have been recorded, got: {:?}",
            *ops
        );
        drop(ops);

        // Also test with None checksum
        let _handler2 = file.apply_textdelta(None).unwrap();
        let ops = operations.borrow();
        assert!(
            ops.iter().any(|s| s == "apply_textdelta(None)"),
            "apply_textdelta(None) should have been recorded, got: {:?}",
            *ops
        );
        drop(ops);

        // Use apply_textdelta_raw to test the WrapTxdeltaWindowHandler path too
        let raw_handler = file.apply_textdelta_raw(Some("def456")).unwrap();
        let ops = operations.borrow();
        assert!(
            ops.iter().any(|s| s == "apply_textdelta(Some(\"def456\"))"),
            "apply_textdelta_raw should route through same trampoline, got: {:?}",
            *ops
        );
        drop(ops);

        // Signal completion
        raw_handler.finish().unwrap();

        file.close(None).unwrap();
        root.close().unwrap();
        wrap_editor.close().unwrap();
    }

    #[test]
    fn test_directory_copyfrom_revision_matters() {
        let operations = Rc::new(RefCell::new(Vec::new()));
        let test_editor = TestEditor {
            operations: operations.clone(),
        };
        let mut wrap_editor = WrapEditor::from_rust_editor(test_editor);
        let mut root = wrap_editor.open_root(None).unwrap();

        // Add directory with copyfrom
        let mut dir1 = root
            .add_directory("dir1", Some(("/src", crate::Revnum(25))))
            .unwrap();
        dir1.close().unwrap();

        root.close().unwrap();
        wrap_editor.close().unwrap();

        let ops = operations.borrow();
        // Verify copyfrom revision is preserved (catches delete - mutation)
        assert!(ops.iter().any(|s| s.contains("dir1") && s.contains("25")));
    }
}
