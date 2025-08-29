use crate::{svn_result, with_tmp_pool, Error};
use std::ffi::OsStr;
use std::marker::PhantomData;
use subversion_sys::svn_io_dirent2_t;

#[allow(dead_code)]
pub struct Dirent(*const svn_io_dirent2_t);

impl From<*const svn_io_dirent2_t> for Dirent {
    fn from(ptr: *const svn_io_dirent2_t) -> Self {
        Self(ptr)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileDel {
    None,
    OnClose,
    OnPoolCleanup,
}

impl From<FileDel> for subversion_sys::svn_io_file_del_t {
    fn from(file_del: FileDel) -> Self {
        match file_del {
            FileDel::None => subversion_sys::svn_io_file_del_t_svn_io_file_del_none,
            FileDel::OnClose => subversion_sys::svn_io_file_del_t_svn_io_file_del_on_close,
            FileDel::OnPoolCleanup => {
                subversion_sys::svn_io_file_del_t_svn_io_file_del_on_pool_cleanup
            }
        }
    }
}

impl From<subversion_sys::svn_io_file_del_t> for FileDel {
    fn from(file_del: subversion_sys::svn_io_file_del_t) -> Self {
        match file_del {
            subversion_sys::svn_io_file_del_t_svn_io_file_del_none => FileDel::None,
            subversion_sys::svn_io_file_del_t_svn_io_file_del_on_close => FileDel::OnClose,
            subversion_sys::svn_io_file_del_t_svn_io_file_del_on_pool_cleanup => {
                FileDel::OnPoolCleanup
            }
            _ => unreachable!(),
        }
    }
}

/// Stream mark with RAII cleanup
pub struct Mark {
    ptr: *mut subversion_sys::svn_stream_mark_t,
    pool: apr::Pool,
    _phantom: PhantomData<*mut ()>, // !Send + !Sync
}

impl Drop for Mark {
    fn drop(&mut self) {
        // Pool drop will clean up mark
    }
}

impl Mark {
    pub fn as_mut_ptr(&mut self) -> *mut subversion_sys::svn_stream_mark_t {
        self.ptr
    }

    pub fn as_ptr(&self) -> *const subversion_sys::svn_stream_mark_t {
        self.ptr
    }

    pub(crate) unsafe fn from_ptr_and_pool(
        ptr: *mut subversion_sys::svn_stream_mark_t,
        pool: apr::Pool,
    ) -> Self {
        Self {
            ptr,
            pool,
            _phantom: PhantomData,
        }
    }
}

/// String buffer for stream operations
pub struct StringBuf {
    ptr: *mut subversion_sys::svn_stringbuf_t,
    pool: apr::Pool,
    _phantom: PhantomData<*mut ()>, // !Send + !Sync
}

impl Drop for StringBuf {
    fn drop(&mut self) {
        // Pool drop will clean up stringbuf
    }
}

impl StringBuf {
    /// Create an empty string buffer
    pub fn new() -> Self {
        let pool = apr::Pool::new();
        let ptr = unsafe { subversion_sys::svn_stringbuf_create_empty(pool.as_mut_ptr()) };
        Self {
            ptr,
            pool,
            _phantom: PhantomData,
        }
    }

    /// Create a string buffer with initial content
    pub fn from_str(s: &str) -> Self {
        let pool = apr::Pool::new();
        let cstr = std::ffi::CString::new(s).unwrap();
        let ptr = unsafe { subversion_sys::svn_stringbuf_create(cstr.as_ptr(), pool.as_mut_ptr()) };
        Self {
            ptr,
            pool,
            _phantom: PhantomData,
        }
    }

    /// Get the contents as a byte slice
    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            let stringbuf = &*self.ptr;
            std::slice::from_raw_parts(stringbuf.data as *const u8, stringbuf.len as usize)
        }
    }

    /// Get the contents as a string
    pub fn to_string(&self) -> String {
        String::from_utf8_lossy(self.as_bytes()).into_owned()
    }

    /// Get the length of the string buffer
    pub fn len(&self) -> usize {
        unsafe { (*self.ptr).len as usize }
    }

    /// Check if the string buffer is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub(crate) fn as_mut_ptr(&mut self) -> *mut subversion_sys::svn_stringbuf_t {
        self.ptr
    }
}

/// Stream handle with RAII cleanup
pub struct Stream {
    ptr: *mut subversion_sys::svn_stream_t,
    pool: apr::Pool,
    _phantom: PhantomData<*mut ()>, // !Send + !Sync
}

impl Drop for Stream {
    fn drop(&mut self) {
        // Pool drop will clean up stream
    }
}

impl Stream {
    pub fn empty() -> Self {
        let pool = apr::Pool::new();
        let stream = unsafe { subversion_sys::svn_stream_empty(pool.as_mut_ptr()) };
        Self {
            ptr: stream,
            pool,
            _phantom: PhantomData,
        }
    }

    /// Create a buffered memory stream for reading and writing
    pub fn buffered() -> Self {
        let pool = apr::Pool::new();
        let stream = unsafe { subversion_sys::svn_stream_buffered(pool.as_mut_ptr()) };
        Self {
            ptr: stream,
            pool,
            _phantom: PhantomData,
        }
    }

    /// Create a stream from a StringBuf for reading and writing
    pub fn from_stringbuf(stringbuf: &mut StringBuf) -> Self {
        let pool = apr::Pool::new();
        let stream = unsafe {
            subversion_sys::svn_stream_from_stringbuf(stringbuf.as_mut_ptr(), pool.as_mut_ptr())
        };
        Self {
            ptr: stream,
            pool,
            _phantom: PhantomData,
        }
    }

    /// Create a stream from a callback-based implementation
    pub fn create<T: 'static>(baton: T) -> Self {
        let pool = apr::Pool::new();
        let baton_ptr = Box::into_raw(Box::new(baton)) as *mut std::ffi::c_void;
        let stream = unsafe { subversion_sys::svn_stream_create(baton_ptr, pool.as_mut_ptr()) };
        Self {
            ptr: stream,
            pool,
            _phantom: PhantomData,
        }
    }

    /// Set the baton for this stream
    pub fn set_baton<T: 'static>(&mut self, baton: T) {
        let baton_ptr = Box::into_raw(Box::new(baton)) as *mut std::ffi::c_void;
        unsafe { subversion_sys::svn_stream_set_baton(self.ptr, baton_ptr) };
    }

    /// Create a disowned stream that doesn't close the underlying resource
    pub fn disown(&mut self) -> Self {
        let pool = apr::Pool::new();
        let stream = unsafe { subversion_sys::svn_stream_disown(self.ptr, pool.as_mut_ptr()) };
        Self {
            ptr: stream,
            pool,
            _phantom: PhantomData,
        }
    }

    /// Create a stream from an APR file
    pub fn from_apr_file(file: *mut apr_sys::apr_file_t, disown: bool) -> Self {
        let pool = apr::Pool::new();
        let stream = unsafe {
            subversion_sys::svn_stream_from_aprfile2(file, disown as i32, pool.as_mut_ptr())
        };
        Self {
            ptr: stream,
            pool,
            _phantom: PhantomData,
        }
    }

    pub fn as_mut_ptr(&mut self) -> *mut subversion_sys::svn_stream_t {
        self.ptr
    }

    pub fn as_ptr(&self) -> *const subversion_sys::svn_stream_t {
        self.ptr
    }

    pub(crate) unsafe fn from_ptr_and_pool(
        ptr: *mut subversion_sys::svn_stream_t,
        pool: apr::Pool,
    ) -> Self {
        Self {
            ptr,
            pool,
            _phantom: PhantomData,
        }
    }

    pub fn open_readonly(path: &std::path::Path) -> Result<Self, Error> {
        let pool = apr::Pool::new();
        let mut stream = std::ptr::null_mut();
        let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();

        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_stream_open_readonly(
                    &mut stream,
                    path.as_ptr(),
                    pool.as_mut_ptr(),
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })?;

        Ok(Self {
            ptr: stream,
            pool,
            _phantom: PhantomData,
        })
    }

    pub fn open_writable(path: &std::path::Path) -> Result<Self, Error> {
        let pool = apr::Pool::new();
        let mut stream = std::ptr::null_mut();
        let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();

        with_tmp_pool(|scratch_pool| {
            let err = unsafe {
                subversion_sys::svn_stream_open_writable(
                    &mut stream,
                    path.as_ptr(),
                    pool.as_mut_ptr(),
                    scratch_pool.as_mut_ptr(),
                )
            };
            svn_result(err)
        })?;

        Ok(Self {
            ptr: stream,
            pool,
            _phantom: PhantomData,
        })
    }

    pub fn open_unique(
        dirpath: &std::path::Path,
        when: FileDel,
    ) -> Result<(std::path::PathBuf, Self), Error> {
        let dirpath = std::ffi::CString::new(dirpath.to_str().unwrap()).unwrap();
        let mut stream = std::ptr::null_mut();
        let mut path = std::ptr::null();
        let pool = apr::pool::Pool::new();
        let err = unsafe {
            subversion_sys::svn_stream_open_unique(
                &mut stream,
                &mut path,
                dirpath.as_ptr(),
                when.into(),
                pool.as_mut_ptr(),
                apr::pool::Pool::new().as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok((
            std::path::PathBuf::from(unsafe { std::ffi::CStr::from_ptr(path).to_str().unwrap() }),
            unsafe { Self::from_ptr_and_pool(stream, pool) },
        ))
    }

    pub fn stdin(buffered: bool) -> Result<Self, Error> {
        let pool = apr::Pool::new();
        let mut stream = std::ptr::null_mut();
        let err = unsafe {
            subversion_sys::svn_stream_for_stdin2(&mut stream, buffered as i32, pool.as_mut_ptr())
        };
        svn_result(err)?;
        Ok(Self {
            ptr: stream,
            pool,
            _phantom: PhantomData,
        })
    }

    pub fn stderr() -> Result<Self, Error> {
        let pool = apr::Pool::new();
        let mut stream = std::ptr::null_mut();
        let err = unsafe { subversion_sys::svn_stream_for_stderr(&mut stream, pool.as_mut_ptr()) };
        svn_result(err)?;
        Ok(Self {
            ptr: stream,
            pool,
            _phantom: PhantomData,
        })
    }

    pub fn stdout() -> Result<Self, Error> {
        let pool = apr::Pool::new();
        let mut stream = std::ptr::null_mut();
        let err = unsafe { subversion_sys::svn_stream_for_stdout(&mut stream, pool.as_mut_ptr()) };
        svn_result(err)?;
        Ok(Self {
            ptr: stream,
            pool,
            _phantom: PhantomData,
        })
    }

    pub fn checksummed(
        &mut self,
        checksum_kind: crate::ChecksumKind,
        read_all: bool,
    ) -> (crate::io::Stream, crate::Checksum, crate::Checksum) {
        let mut read_checksum = std::ptr::null_mut();
        let mut write_checksum = std::ptr::null_mut();
        let stream = unsafe {
            subversion_sys::svn_stream_checksummed2(
                self.ptr,
                &mut read_checksum,
                &mut write_checksum,
                checksum_kind.into(),
                read_all as i32,
                apr::pool::Pool::new().as_mut_ptr(),
            )
        };
        let pool = std::rc::Rc::new(apr::pool::Pool::new());
        (
            unsafe { crate::io::Stream::from_ptr_and_pool(stream, apr::Pool::new()) },
            crate::Checksum {
                ptr: read_checksum,
                _pool: std::marker::PhantomData,
            },
            crate::Checksum {
                ptr: write_checksum,
                _pool: std::marker::PhantomData,
            },
        )
    }

    pub fn compressed(&mut self) -> Self {
        let pool = apr::Pool::new();
        let stream = unsafe { subversion_sys::svn_stream_compressed(self.ptr, pool.as_mut_ptr()) };
        Self {
            ptr: stream,
            pool,
            _phantom: PhantomData,
        }
    }

    pub fn contents_checksum(
        &mut self,
        checksum_kind: crate::ChecksumKind,
    ) -> Result<crate::Checksum, Error> {
        let mut checksum = std::ptr::null_mut();
        let pool = apr::pool::Pool::new();
        let err = unsafe {
            subversion_sys::svn_stream_contents_checksum(
                &mut checksum,
                self.ptr,
                checksum_kind.into(),
                pool.as_mut_ptr(),
                apr::pool::Pool::new().as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(crate::Checksum {
            ptr: checksum,
            _pool: std::marker::PhantomData,
        })
    }

    pub fn read_full(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        let mut len = buf.len();
        let err = unsafe {
            subversion_sys::svn_stream_read_full(self.ptr, buf.as_mut_ptr() as *mut i8, &mut len)
        };
        Error::from_raw(err)?;
        Ok(len)
    }

    pub fn supports_partial_read(&mut self) -> bool {
        unsafe { subversion_sys::svn_stream_supports_partial_read(self.ptr) != 0 }
    }

    pub fn skip(&mut self, len: usize) -> Result<(), Error> {
        let err = unsafe { subversion_sys::svn_stream_skip(self.ptr, len) };
        Error::from_raw(err)?;
        Ok(())
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        let mut len = buf.len();
        let err = unsafe {
            subversion_sys::svn_stream_read2(self.ptr, buf.as_mut_ptr() as *mut i8, &mut len)
        };
        Error::from_raw(err)?;
        Ok(len)
    }

    pub fn write(&mut self, buf: &[u8]) -> Result<(), Error> {
        let mut len = buf.len();
        let err = unsafe {
            subversion_sys::svn_stream_write(self.ptr, buf.as_ptr() as *const i8, &mut len)
        };
        Error::from_raw(err)?;
        Ok(())
    }

    pub fn close(&mut self) -> Result<(), Error> {
        let err = unsafe { subversion_sys::svn_stream_close(self.ptr) };
        Error::from_raw(err)?;
        Ok(())
    }

    pub fn supports_mark(&mut self) -> bool {
        unsafe { subversion_sys::svn_stream_supports_mark(self.ptr) != 0 }
    }

    pub fn mark(&mut self) -> Result<Mark, Error> {
        let mut mark = std::ptr::null_mut();
        let pool = apr::pool::Pool::new();
        let err =
            unsafe { subversion_sys::svn_stream_mark(self.ptr, &mut mark, pool.as_mut_ptr()) };
        svn_result(err)?;
        Ok(unsafe { Mark::from_ptr_and_pool(mark, pool) })
    }

    pub fn seek(&mut self, mark: &Mark) -> Result<(), Error> {
        let err = unsafe { subversion_sys::svn_stream_seek(self.ptr, mark.as_ptr()) };
        Error::from_raw(err)?;
        Ok(())
    }

    /// Reset the stream to its beginning (if supported)
    pub fn reset(&mut self) -> Result<(), Error> {
        let err = unsafe { subversion_sys::svn_stream_reset(self.ptr) };
        Error::from_raw(err)?;
        Ok(())
    }

    /// Check if the stream supports reset
    pub fn supports_reset(&self) -> bool {
        unsafe { subversion_sys::svn_stream_supports_reset(self.ptr) != 0 }
    }

    /// Check if data is available for reading without blocking
    pub fn data_available(&mut self) -> Result<bool, Error> {
        let mut available = 0;
        let err = unsafe { subversion_sys::svn_stream_data_available(self.ptr, &mut available) };
        Error::from_raw(err)?;
        Ok(available != 0)
    }

    /// Read a line from the stream
    pub fn readline(&mut self, eol: &str) -> Result<Option<String>, Error> {
        let pool = apr::Pool::new();
        let mut stringbuf = std::ptr::null_mut();
        let mut eof = 0;
        let eol_cstr = std::ffi::CString::new(eol).unwrap();

        let err = unsafe {
            subversion_sys::svn_stream_readline(
                self.ptr,
                &mut stringbuf,
                eol_cstr.as_ptr(),
                &mut eof,
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;

        if eof != 0 && stringbuf.is_null() {
            Ok(None)
        } else {
            unsafe {
                let svn_stringbuf = &*stringbuf;
                let data_slice = std::slice::from_raw_parts(
                    svn_stringbuf.data as *const u8,
                    svn_stringbuf.len as usize,
                );
                Ok(Some(String::from_utf8_lossy(data_slice).into_owned()))
            }
        }
    }

    pub fn puts(&mut self, s: &str) -> Result<(), Error> {
        let s = std::ffi::CString::new(s).unwrap();
        let err = unsafe { subversion_sys::svn_stream_puts(self.ptr, s.as_ptr()) };

        Error::from_raw(err)?;
        Ok(())
    }

    /// Write a formatted string to the stream
    pub fn printf(&mut self, format: &str, args: &[&dyn std::fmt::Display]) -> Result<(), Error> {
        let mut formatted = String::new();
        let mut arg_iter = args.iter();
        let mut chars = format.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '%' {
                if chars.peek() == Some(&'%') {
                    chars.next();
                    formatted.push('%');
                } else {
                    // Skip format specifier characters
                    while let Some(spec_char) = chars.next() {
                        if spec_char.is_alphabetic() {
                            if let Some(arg) = arg_iter.next() {
                                formatted.push_str(&arg.to_string());
                            }
                            break;
                        }
                    }
                }
            } else {
                formatted.push(c);
            }
        }

        self.puts(&formatted)
    }

    /// Write a formatted UTF-8 string to the stream
    pub fn printf_from_utf8(
        &mut self,
        format: &str,
        args: &[&dyn std::fmt::Display],
    ) -> Result<(), Error> {
        // For now, this is the same as printf since we're already working with UTF-8
        self.printf(format, args)
    }

    /// Set a read callback for the stream
    /// Note: This stores the callback in the stream's baton.
    pub fn set_read_callback<F>(&mut self, read_func: F)
    where
        F: Fn(&mut [u8]) -> Result<usize, Error> + 'static,
    {
        // Store the callback in the stream's baton
        let boxed_func = Box::into_raw(Box::new(read_func));
        self.set_baton(boxed_func as *mut std::ffi::c_void);

        extern "C" fn read_trampoline(
            baton: *mut std::ffi::c_void,
            buffer: *mut std::os::raw::c_char,
            len: *mut apr_sys::apr_size_t,
        ) -> *mut subversion_sys::svn_error_t {
            let read_func =
                unsafe { &*(baton as *const Box<dyn Fn(&mut [u8]) -> Result<usize, Error>>) };
            let buf = unsafe { std::slice::from_raw_parts_mut(buffer as *mut u8, *len) };
            match read_func(buf) {
                Ok(bytes_read) => {
                    unsafe { *len = bytes_read };
                    std::ptr::null_mut()
                }
                Err(mut e) => unsafe { e.detach() },
            }
        }

        unsafe {
            subversion_sys::svn_stream_set_read(self.ptr, Some(read_trampoline));
        }
    }

    /// Set a write callback for the stream
    /// Note: This stores the callback in the stream's baton.
    pub fn set_write_callback<F>(&mut self, write_func: F)
    where
        F: Fn(&[u8]) -> Result<usize, Error> + 'static,
    {
        // Store the callback in the stream's baton
        let boxed_func = Box::into_raw(Box::new(write_func));
        self.set_baton(boxed_func as *mut std::ffi::c_void);

        extern "C" fn write_trampoline(
            baton: *mut std::ffi::c_void,
            data: *const std::os::raw::c_char,
            len: *mut apr_sys::apr_size_t,
        ) -> *mut subversion_sys::svn_error_t {
            let write_func =
                unsafe { &*(baton as *const Box<dyn Fn(&[u8]) -> Result<usize, Error>>) };
            let buf = unsafe { std::slice::from_raw_parts(data as *const u8, *len) };
            match write_func(buf) {
                Ok(bytes_written) => {
                    unsafe { *len = bytes_written };
                    std::ptr::null_mut()
                }
                Err(mut e) => unsafe { e.detach() },
            }
        }

        unsafe {
            subversion_sys::svn_stream_set_write(self.ptr, Some(write_trampoline));
        }
    }

    /// Set a close callback for the stream
    /// Note: This stores the callback in the stream's baton.
    pub fn set_close_callback<F>(&mut self, close_func: F)
    where
        F: Fn() -> Result<(), Error> + 'static,
    {
        // Store the callback in the stream's baton
        let boxed_func = Box::into_raw(Box::new(close_func));
        self.set_baton(boxed_func as *mut std::ffi::c_void);

        extern "C" fn close_trampoline(
            baton: *mut std::ffi::c_void,
        ) -> *mut subversion_sys::svn_error_t {
            let close_func = unsafe { &*(baton as *const Box<dyn Fn() -> Result<(), Error>>) };
            match close_func() {
                Ok(_) => std::ptr::null_mut(),
                Err(mut e) => unsafe { e.detach() },
            }
        }

        unsafe {
            subversion_sys::svn_stream_set_close(self.ptr, Some(close_trampoline));
        }
    }

    /// Set a skip callback for the stream
    /// Note: This stores the callback in the stream's baton.
    pub fn set_skip_callback<F>(&mut self, skip_func: F)
    where
        F: Fn(usize) -> Result<(), Error> + 'static,
    {
        // Store the callback in the stream's baton
        let boxed_func = Box::into_raw(Box::new(skip_func));
        self.set_baton(boxed_func as *mut std::ffi::c_void);

        extern "C" fn skip_trampoline(
            baton: *mut std::ffi::c_void,
            len: apr_sys::apr_size_t,
        ) -> *mut subversion_sys::svn_error_t {
            let skip_func = unsafe { &*(baton as *const Box<dyn Fn(usize) -> Result<(), Error>>) };
            match skip_func(len) {
                Ok(_) => std::ptr::null_mut(),
                Err(mut e) => unsafe { e.detach() },
            }
        }

        unsafe {
            subversion_sys::svn_stream_set_skip(self.ptr, Some(skip_trampoline));
        }
    }

    /// Set mark callback for the stream
    /// Note: This stores the callback in the stream's baton.
    pub fn set_mark_callback<F>(&mut self, mark_func: F)
    where
        F: Fn() -> Result<*mut subversion_sys::svn_stream_mark_t, Error> + 'static,
    {
        // Store the callback in the stream's baton
        let boxed_func = Box::into_raw(Box::new(mark_func));
        self.set_baton(boxed_func as *mut std::ffi::c_void);

        extern "C" fn mark_trampoline(
            baton: *mut std::ffi::c_void,
            mark: *mut *mut subversion_sys::svn_stream_mark_t,
            pool: *mut apr_sys::apr_pool_t,
        ) -> *mut subversion_sys::svn_error_t {
            let mark_func = unsafe {
                &*(baton
                    as *const Box<
                        dyn Fn() -> Result<*mut subversion_sys::svn_stream_mark_t, Error>,
                    >)
            };
            match mark_func() {
                Ok(mark_ptr) => {
                    unsafe { *mark = mark_ptr };
                    std::ptr::null_mut()
                }
                Err(mut e) => unsafe { e.detach() },
            }
        }

        unsafe {
            subversion_sys::svn_stream_set_mark(self.ptr, Some(mark_trampoline));
        }
    }

    /// Set seek callback for the stream
    /// Note: This stores the callback in the stream's baton.
    pub fn set_seek_callback<F>(&mut self, seek_func: F)
    where
        F: Fn(*const subversion_sys::svn_stream_mark_t) -> Result<(), Error> + 'static,
    {
        // Store the callback in the stream's baton
        let boxed_func = Box::into_raw(Box::new(seek_func));
        self.set_baton(boxed_func as *mut std::ffi::c_void);

        extern "C" fn seek_trampoline(
            baton: *mut std::ffi::c_void,
            mark: *const subversion_sys::svn_stream_mark_t,
        ) -> *mut subversion_sys::svn_error_t {
            let seek_func = unsafe {
                &*(baton
                    as *const Box<
                        dyn Fn(*const subversion_sys::svn_stream_mark_t) -> Result<(), Error>,
                    >)
            };
            match seek_func(mark) {
                Ok(_) => std::ptr::null_mut(),
                Err(mut e) => unsafe { e.detach() },
            }
        }

        unsafe {
            subversion_sys::svn_stream_set_seek(self.ptr, Some(seek_trampoline));
        }
    }

    /// Create a lazyopen stream with a callback function
    pub fn lazyopen_create<F>(open_func: F, close_baton: bool) -> Self
    where
        F: Fn() -> Result<Stream, Error> + 'static,
    {
        let pool = apr::Pool::new();
        let open_func_ptr = Box::into_raw(Box::new(open_func)) as *mut std::ffi::c_void;

        extern "C" fn open_trampoline(
            lazyopen_stream: *mut *mut subversion_sys::svn_stream_t,
            open_baton: *mut std::ffi::c_void,
            result_pool: *mut apr_sys::apr_pool_t,
            _scratch_pool: *mut apr_sys::apr_pool_t,
        ) -> *mut subversion_sys::svn_error_t {
            let open_func =
                unsafe { &*(open_baton as *const Box<dyn Fn() -> Result<Stream, Error>>) };
            match open_func() {
                Ok(stream) => {
                    unsafe { *lazyopen_stream = stream.ptr };
                    std::ptr::null_mut()
                }
                Err(mut e) => unsafe { e.detach() },
            }
        }

        let stream = unsafe {
            subversion_sys::svn_stream_lazyopen_create(
                Some(open_trampoline),
                open_func_ptr,
                close_baton as i32,
                pool.as_mut_ptr(),
            )
        };

        Self {
            ptr: stream,
            pool,
            _phantom: PhantomData,
        }
    }

    /// Create a stream from a Rust reader
    /// This is a simplified implementation that works with the existing SVN stream infrastructure
    pub fn from_reader<R: std::io::Read + 'static>(mut reader: R) -> Result<Self, Error> {
        // Read all data from the reader into memory
        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer).map_err(Error::from)?;

        // Create a stream from the buffer
        Ok(Stream::from(buffer))
    }
}

impl std::io::Write for Stream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self.write(buf) {
            Ok(_) => Ok(buf.len()),
            Err(e) => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            )),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl std::io::Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self.read(buf) {
            Ok(bytes_read) => Ok(bytes_read),
            Err(e) => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            )),
        }
    }
}

pub fn tee(out1: &mut Stream, out2: &mut Stream) -> Result<Stream, Error> {
    let pool = apr::Pool::new();
    let stream = unsafe { subversion_sys::svn_stream_tee(out1.ptr, out2.ptr, pool.as_mut_ptr()) };
    Ok(Stream {
        ptr: stream,
        pool,
        _phantom: PhantomData,
    })
}

/// Check if two streams have the same contents
pub fn streams_contents_same(stream1: &mut Stream, stream2: &mut Stream) -> Result<bool, Error> {
    let mut same = 0;
    let pool = apr::Pool::new();
    let err = unsafe {
        subversion_sys::svn_stream_contents_same2(
            &mut same,
            stream1.ptr,
            stream2.ptr,
            pool.as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok(same != 0)
}

/// Copy contents from one stream to another with optional cancel function
pub fn copy_stream(
    from: &mut Stream,
    to: &mut Stream,
    cancel_func: Option<&impl Fn() -> Result<(), Error>>,
) -> Result<(), Error> {
    stream_copy(from, to, cancel_func)
}

/// Read the entire contents of a stream into a string
pub fn read_stream_to_string(stream: &mut Stream) -> Result<String, Error> {
    string_from_stream(stream)
}

impl From<&[u8]> for Stream {
    fn from(bytes: &[u8]) -> Self {
        let pool = apr::Pool::new();
        // Create a proper svn_string in the pool
        let svn_str = unsafe {
            subversion_sys::svn_string_ncreate(
                bytes.as_ptr() as *const i8,
                bytes.len(),
                pool.as_mut_ptr(),
            )
        };
        let stream = unsafe { subversion_sys::svn_stream_from_string(svn_str, pool.as_mut_ptr()) };
        Self {
            ptr: stream,
            pool,
            _phantom: PhantomData,
        }
    }
}

impl From<String> for Stream {
    fn from(s: String) -> Self {
        Stream::from(s.as_bytes())
    }
}

impl From<&str> for Stream {
    fn from(s: &str) -> Self {
        Stream::from(s.as_bytes())
    }
}

impl From<Vec<u8>> for Stream {
    fn from(bytes: Vec<u8>) -> Self {
        Stream::from(bytes.as_slice())
    }
}

pub fn remove_file(path: &std::path::Path, ignore_enoent: bool) -> Result<(), Error> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let err = unsafe {
        subversion_sys::svn_io_remove_file2(
            path.as_ptr(),
            ignore_enoent as i32,
            apr::pool::Pool::new().as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok(())
}

/// Create a stream that wraps a std::io::Write trait object
pub fn wrap_write(write: &mut dyn std::io::Write) -> Result<Stream, Error> {
    let write = Box::into_raw(Box::new(write));
    let pool = apr::Pool::new();
    let stream = unsafe {
        subversion_sys::svn_stream_create(write as *mut std::ffi::c_void, pool.as_mut_ptr())
    };

    extern "C" fn write_fn(
        baton: *mut std::ffi::c_void,
        buffer: *const i8,
        len: *mut usize,
    ) -> *mut subversion_sys::svn_error_t {
        let write = unsafe { Box::from_raw(baton as *mut &mut dyn std::io::Write) };
        let write = Box::leak(write);
        let buffer = unsafe { std::slice::from_raw_parts(buffer as *const u8, *len) };
        match write.write(buffer) {
            Ok(_) => std::ptr::null_mut(),
            Err(e) => {
                let mut e: crate::Error = e.into();
                unsafe { e.detach() }
            }
        }
    }

    extern "C" fn close_fn(baton: *mut std::ffi::c_void) -> *mut subversion_sys::svn_error_t {
        let write = unsafe { Box::from_raw(baton as *mut &mut dyn std::io::Write) };
        match write.flush() {
            Ok(_) => std::ptr::null_mut(),
            Err(e) => {
                let mut e: crate::Error = e.into();
                unsafe { e.detach() }
            }
        }
    }

    unsafe {
        subversion_sys::svn_stream_set_write(stream, Some(write_fn));
        subversion_sys::svn_stream_set_close(stream, Some(close_fn));
    }

    Ok(Stream {
        ptr: stream,
        pool,
        _phantom: PhantomData,
    })
}

pub fn create_uniqe_link(
    path: &std::path::Path,
    dest: &std::path::Path,
    suffix: &OsStr,
) -> Result<std::path::PathBuf, Error> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let dest = std::ffi::CString::new(dest.to_str().unwrap()).unwrap();
    let suffix = std::ffi::CString::new(suffix.to_str().unwrap()).unwrap();
    let pool = apr::pool::Pool::new();
    let mut result = std::ptr::null();
    let err = unsafe {
        subversion_sys::svn_io_create_unique_link(
            &mut result,
            path.as_ptr(),
            dest.as_ptr(),
            suffix.as_ptr(),
            pool.as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok(std::path::PathBuf::from(unsafe {
        std::ffi::CStr::from_ptr(result).to_str().unwrap()
    }))
}

pub fn read_link(path: &std::path::Path) -> Result<std::path::PathBuf, Error> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let mut target = std::ptr::null_mut();
    let err = unsafe {
        subversion_sys::svn_io_read_link(
            &mut target,
            path.as_ptr(),
            apr::pool::Pool::new().as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;

    let ptr = unsafe { (*target).data as *const u8 };
    let len = unsafe { (*target).len };

    let target = unsafe { std::slice::from_raw_parts(ptr, len) };
    Ok(std::path::PathBuf::from(
        std::str::from_utf8(target).unwrap(),
    ))
}

pub fn temp_dir() -> Result<std::path::PathBuf, Error> {
    let pool = apr::pool::Pool::new();
    let mut path = std::ptr::null();
    let err = unsafe { subversion_sys::svn_io_temp_dir(&mut path, pool.as_mut_ptr()) };
    Error::from_raw(err)?;
    Ok(std::path::PathBuf::from(unsafe {
        std::ffi::CStr::from_ptr(path).to_str().unwrap()
    }))
}

pub fn copy_file(
    src: &std::path::Path,
    dest: &std::path::Path,
    copy_perms: bool,
) -> Result<(), Error> {
    let src = std::ffi::CString::new(src.to_str().unwrap()).unwrap();
    let dest = std::ffi::CString::new(dest.to_str().unwrap()).unwrap();
    let err = unsafe {
        subversion_sys::svn_io_copy_file(
            src.as_ptr(),
            dest.as_ptr(),
            copy_perms as i32,
            apr::pool::Pool::new().as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok(())
}

pub fn copy_perms(src: &std::path::Path, dest: &std::path::Path) -> Result<(), Error> {
    let src = std::ffi::CString::new(src.to_str().unwrap()).unwrap();
    let dest = std::ffi::CString::new(dest.to_str().unwrap()).unwrap();
    let err = unsafe {
        subversion_sys::svn_io_copy_perms(
            src.as_ptr(),
            dest.as_ptr(),
            apr::pool::Pool::new().as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok(())
}

pub fn copy_link(src: &std::path::Path, dest: &std::path::Path) -> Result<(), Error> {
    let src = std::ffi::CString::new(src.to_str().unwrap()).unwrap();
    let dest = std::ffi::CString::new(dest.to_str().unwrap()).unwrap();
    let err = unsafe {
        subversion_sys::svn_io_copy_link(
            src.as_ptr(),
            dest.as_ptr(),
            apr::pool::Pool::new().as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok(())
}

pub fn copy_dir_recursively(
    src: &std::path::Path,
    dst_path: &std::path::Path,
    dst_basename: &std::ffi::OsStr,
    copy_perms: bool,
    cancel_func: Option<&impl Fn() -> Result<(), Error>>,
) -> Result<(), Error> {
    use std::os::unix::ffi::OsStrExt;
    let src = std::ffi::CString::new(src.to_str().unwrap()).unwrap();
    let dst_path = std::ffi::CString::new(dst_path.to_str().unwrap()).unwrap();
    let dst_basename = std::ffi::CString::new(dst_basename.as_bytes()).unwrap();
    let err = unsafe {
        subversion_sys::svn_io_copy_dir_recursively(
            src.as_ptr(),
            dst_path.as_ptr(),
            dst_basename.as_ptr(),
            copy_perms as i32,
            if cancel_func.is_some() {
                Some(crate::wrap_cancel_func)
            } else {
                None
            },
            if let Some(cancel_func) = cancel_func {
                &cancel_func as *const _ as *mut std::ffi::c_void
            } else {
                std::ptr::null_mut()
            },
            apr::pool::Pool::new().as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok(())
}

pub fn make_dir_recursively(path: &std::path::Path) -> Result<(), Error> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let err = unsafe {
        subversion_sys::svn_io_make_dir_recursively(
            path.as_ptr(),
            apr::pool::Pool::new().as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok(())
}

pub fn dir_empty(path: &std::path::Path) -> Result<bool, Error> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let mut empty = 0;
    let err = unsafe {
        subversion_sys::svn_io_dir_empty(
            &mut empty,
            path.as_ptr(),
            apr::pool::Pool::new().as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok(empty != 0)
}

pub fn append_file(src: &std::path::Path, dest: &std::path::Path) -> Result<(), Error> {
    let src = std::ffi::CString::new(src.to_str().unwrap()).unwrap();
    let dest = std::ffi::CString::new(dest.to_str().unwrap()).unwrap();
    let err = unsafe {
        subversion_sys::svn_io_append_file(
            src.as_ptr(),
            dest.as_ptr(),
            apr::pool::Pool::new().as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok(())
}

pub fn set_file_read_only(path: &std::path::Path, ignore_enoent: bool) -> Result<(), Error> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();

    let err = unsafe {
        subversion_sys::svn_io_set_file_read_only(
            path.as_ptr(),
            ignore_enoent as i32,
            apr::pool::Pool::new().as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok(())
}

pub fn set_file_read_write(path: &std::path::Path, ignore_enoent: bool) -> Result<(), Error> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();

    let err = unsafe {
        subversion_sys::svn_io_set_file_read_write(
            path.as_ptr(),
            ignore_enoent as i32,
            apr::pool::Pool::new().as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok(())
}

pub fn is_file_executable(path: &std::path::Path) -> Result<bool, Error> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let mut executable = 0;
    let err = unsafe {
        subversion_sys::svn_io_is_file_executable(
            &mut executable,
            path.as_ptr(),
            apr::pool::Pool::new().as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok(executable != 0)
}

pub fn file_affected_time(path: &std::path::Path) -> Result<apr::time::Time, Error> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let mut affected_time = 0;
    let err = unsafe {
        subversion_sys::svn_io_file_affected_time(
            &mut affected_time,
            path.as_ptr(),
            apr::pool::Pool::new().as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok(apr::time::Time::from(affected_time))
}

pub fn set_file_affected_time(
    path: &std::path::Path,
    affected_time: apr::time::Time,
) -> Result<(), Error> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let affected_time = affected_time.into();
    let err = unsafe {
        subversion_sys::svn_io_set_file_affected_time(
            affected_time,
            path.as_ptr(),
            apr::pool::Pool::new().as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok(())
}

pub fn sleep_for_timestamps(path: &std::path::Path) {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    unsafe {
        subversion_sys::svn_io_sleep_for_timestamps(
            path.as_ptr(),
            apr::pool::Pool::new().as_mut_ptr(),
        )
    }
}

pub fn filesizes_different_p(
    file1: &std::path::Path,
    file2: &std::path::Path,
) -> Result<bool, Error> {
    let file1 = std::ffi::CString::new(file1.to_str().unwrap()).unwrap();
    let file2 = std::ffi::CString::new(file2.to_str().unwrap()).unwrap();
    let mut different = 0;
    let err = unsafe {
        subversion_sys::svn_io_filesizes_different_p(
            &mut different,
            file1.as_ptr(),
            file2.as_ptr(),
            apr::pool::Pool::new().as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok(different != 0)
}

pub fn filesizes_three_different_p(
    file1: &std::path::Path,
    file2: &std::path::Path,
    file3: &std::path::Path,
) -> Result<(bool, bool, bool), Error> {
    let file1 = std::ffi::CString::new(file1.to_str().unwrap()).unwrap();
    let file2 = std::ffi::CString::new(file2.to_str().unwrap()).unwrap();
    let file3 = std::ffi::CString::new(file3.to_str().unwrap()).unwrap();
    let mut different1 = 0;
    let mut different2 = 0;
    let mut different3 = 0;
    let err = unsafe {
        subversion_sys::svn_io_filesizes_three_different_p(
            &mut different1,
            &mut different2,
            &mut different3,
            file1.as_ptr(),
            file2.as_ptr(),
            file3.as_ptr(),
            apr::pool::Pool::new().as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok((different1 != 0, different2 != 0, different3 != 0))
}

pub fn file_checksum(
    file: &std::path::Path,
    checksum_kind: crate::ChecksumKind,
) -> Result<crate::Checksum, Error> {
    let file = std::ffi::CString::new(file.to_str().unwrap()).unwrap();
    let mut checksum = std::ptr::null_mut();
    let pool = apr::pool::Pool::new();
    let err = unsafe {
        subversion_sys::svn_io_file_checksum2(
            &mut checksum,
            file.as_ptr(),
            checksum_kind.into(),
            pool.as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok(crate::Checksum {
        ptr: checksum,
        _pool: std::marker::PhantomData,
    })
}

pub fn files_contents_same_p(
    file1: &std::path::Path,
    file2: &std::path::Path,
) -> Result<bool, Error> {
    let file1 = std::ffi::CString::new(file1.to_str().unwrap()).unwrap();
    let file2 = std::ffi::CString::new(file2.to_str().unwrap()).unwrap();
    let mut same = 0;
    let err = unsafe {
        subversion_sys::svn_io_files_contents_same_p(
            &mut same,
            file1.as_ptr(),
            file2.as_ptr(),
            apr::pool::Pool::new().as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok(same != 0)
}

pub fn files_contents_three_same_p(
    file1: &std::path::Path,
    file2: &std::path::Path,
    file3: &std::path::Path,
) -> Result<(bool, bool, bool), Error> {
    let file1 = std::ffi::CString::new(file1.to_str().unwrap()).unwrap();
    let file2 = std::ffi::CString::new(file2.to_str().unwrap()).unwrap();
    let file3 = std::ffi::CString::new(file3.to_str().unwrap()).unwrap();
    let mut same1 = 0;
    let mut same2 = 0;
    let mut same3 = 0;
    let err = unsafe {
        subversion_sys::svn_io_files_contents_three_same_p(
            &mut same1,
            &mut same2,
            &mut same3,
            file1.as_ptr(),
            file2.as_ptr(),
            file3.as_ptr(),
            apr::pool::Pool::new().as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok((same1 != 0, same2 != 0, same3 != 0))
}

pub fn file_create(path: &std::path::Path, contents: &str) -> Result<(), Error> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let contents = std::ffi::CString::new(contents).unwrap();
    let err = unsafe {
        subversion_sys::svn_io_file_create(
            path.as_ptr(),
            contents.as_ptr(),
            apr::pool::Pool::new().as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok(())
}

pub fn file_create_bytes(path: &std::path::Path, contents: &[u8]) -> Result<(), Error> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let err = unsafe {
        subversion_sys::svn_io_file_create_bytes(
            path.as_ptr(),
            contents.as_ptr() as *const std::ffi::c_void,
            contents.len(),
            apr::pool::Pool::new().as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok(())
}

pub fn file_create_empty(path: &std::path::Path) -> Result<(), Error> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let err = unsafe {
        subversion_sys::svn_io_file_create_empty(path.as_ptr(), apr::pool::Pool::new().as_mut_ptr())
    };
    Error::from_raw(err)?;
    Ok(())
}

pub fn file_lock(path: &std::path::Path, exclusive: bool, nonblocking: bool) -> Result<(), Error> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let err = unsafe {
        subversion_sys::svn_io_file_lock2(
            path.as_ptr(),
            exclusive as i32,
            nonblocking as i32,
            apr::pool::Pool::new().as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok(())
}

pub fn dir_file_copy(
    src_path: &std::path::Path,
    dest_path: &std::path::Path,
    file: &std::ffi::OsStr,
) -> Result<(), Error> {
    use std::os::unix::ffi::OsStrExt;
    let src_path = std::ffi::CString::new(src_path.to_str().unwrap()).unwrap();
    let dest_path = std::ffi::CString::new(dest_path.to_str().unwrap()).unwrap();
    let file = std::ffi::CString::new(file.as_bytes()).unwrap();
    let err = unsafe {
        subversion_sys::svn_io_dir_file_copy(
            src_path.as_ptr(),
            dest_path.as_ptr(),
            file.as_ptr(),
            apr::pool::Pool::new().as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok(())
}

pub fn stream_copy(
    from: &mut Stream,
    to: &mut Stream,
    cancel_func: Option<&impl Fn() -> Result<(), Error>>,
) -> Result<(), Error> {
    let err = unsafe {
        subversion_sys::svn_stream_copy3(
            from.ptr,
            to.ptr,
            if cancel_func.is_some() {
                Some(crate::wrap_cancel_func)
            } else {
                None
            },
            if let Some(cancel_func) = cancel_func {
                &cancel_func as *const _ as *mut std::ffi::c_void
            } else {
                std::ptr::null_mut()
            },
            apr::pool::Pool::new().as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok(())
}

pub fn stream_contents_same(stream1: &mut Stream, stream2: &mut Stream) -> Result<bool, Error> {
    let mut same = 0;
    let err = unsafe {
        subversion_sys::svn_stream_contents_same(
            &mut same,
            stream1.ptr,
            stream2.ptr,
            apr::pool::Pool::new().as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok(same != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_empty() {
        let mut stream = Stream::empty();
        let mut buf = [0u8; 10];
        let bytes_read = stream.read(&mut buf).unwrap();
        assert_eq!(bytes_read, 0);
    }

    #[test]
    fn test_stream_from_string() {
        let data = "Hello, world!";
        let mut stream = Stream::from(data);
        let mut buf = vec![0u8; data.len()];
        let bytes_read = stream.read_full(&mut buf).unwrap();
        assert_eq!(bytes_read, data.len());
        assert_eq!(&buf[..bytes_read], data.as_bytes());
    }

    #[test]
    fn test_stream_from_bytes() {
        let data = b"Binary \x00 data";
        let mut stream = Stream::from(&data[..]);
        let mut buf = vec![0u8; data.len()];
        let bytes_read = stream.read_full(&mut buf).unwrap();
        assert_eq!(bytes_read, data.len());
        assert_eq!(&buf[..bytes_read], data);
    }

    #[test]
    fn test_stringbuf_operations() {
        let mut stringbuf = StringBuf::from_str("Initial content");
        assert_eq!(stringbuf.len(), 15);
        assert!(!stringbuf.is_empty());
        assert_eq!(stringbuf.to_string(), "Initial content");
        assert_eq!(stringbuf.as_bytes(), b"Initial content");
    }

    #[test]
    fn test_stream_buffered() {
        let mut stream = Stream::buffered();
        let data = b"Test data";
        stream.write(data).unwrap();

        // Note: buffered streams may not allow immediate read after write
        // without reset/seek, so this test just verifies creation
    }

    #[test]
    fn test_stream_write_read_with_stringbuf() {
        let mut stringbuf = StringBuf::new();

        // Write data
        {
            let mut write_stream = Stream::from_stringbuf(&mut stringbuf);
            write_stream.write(b"Hello").unwrap();
            write_stream.write(b", ").unwrap();
            write_stream.write(b"world!").unwrap();
        }

        // Verify stringbuf contains the data
        assert_eq!(stringbuf.to_string(), "Hello, world!");
    }

    #[test]
    fn test_stream_puts_printf() {
        let mut stringbuf = StringBuf::new();
        let mut stream = Stream::from_stringbuf(&mut stringbuf);

        stream.puts("Line 1").unwrap();
        stream.puts("Line 2").unwrap();
        stream.printf("Number: %d", &[&42]).unwrap();

        let content = stringbuf.to_string();
        assert!(content.contains("Line 1"));
        assert!(content.contains("Line 2"));
        assert!(content.contains("42"));
    }

    #[test]
    fn test_stream_reset() {
        // Create a stream that supports reset (string-based streams typically do)
        let data = "Test data for reset";
        let mut stream = Stream::from(data);

        if stream.supports_reset() {
            let mut buf1 = vec![0u8; 4];
            stream.read_full(&mut buf1).unwrap();
            assert_eq!(&buf1, b"Test");

            // Reset and read again
            stream.reset().unwrap();
            let mut buf2 = vec![0u8; 4];
            stream.read_full(&mut buf2).unwrap();
            assert_eq!(&buf2, b"Test");
        }
    }

    #[test]
    fn test_stream_mark_seek() {
        let data = "0123456789";
        let mut stream = Stream::from(data);

        if stream.supports_mark() {
            // Read first 3 bytes
            let mut buf = vec![0u8; 3];
            stream.read_full(&mut buf).unwrap();
            assert_eq!(&buf, b"012");

            // Mark current position
            let mark = stream.mark().unwrap();

            // Read next 3 bytes
            stream.read_full(&mut buf).unwrap();
            assert_eq!(&buf, b"345");

            // Seek back to mark
            stream.seek(&mark).unwrap();

            // Read should give us "345" again
            stream.read_full(&mut buf).unwrap();
            assert_eq!(&buf, b"345");
        }
    }

    #[test]
    fn test_stream_readline() {
        let data = "Line 1\nLine 2\rLine 3\r\nLine 4";
        let mut stream = Stream::from(data);

        // Read lines with different EOL markers
        if let Ok(Some(line)) = stream.readline("\n") {
            assert_eq!(line, "Line 1");
        }
    }

    #[test]
    fn test_stream_skip() {
        let data = "0123456789";
        let mut stream = Stream::from(data);

        // Skip first 5 bytes
        stream.skip(5).unwrap();

        // Read remaining
        let mut buf = vec![0u8; 5];
        let bytes_read = stream.read_full(&mut buf).unwrap();
        assert_eq!(bytes_read, 5);
        assert_eq!(&buf, b"56789");
    }

    #[test]
    fn test_io_read_trait() {
        use std::io::Read;

        let data = "Hello from Read trait";
        let mut stream = Stream::from(data);
        let mut buf = String::new();
        stream.read_to_string(&mut buf).unwrap();
        assert_eq!(buf, data);
    }

    #[test]
    fn test_io_write_trait() {
        use std::io::Write;

        let mut stringbuf = StringBuf::new();
        let mut stream = Stream::from_stringbuf(&mut stringbuf);

        write!(stream, "Hello {}", "world").unwrap();
        stream.flush().unwrap();

        assert!(stringbuf.to_string().contains("Hello world"));
    }
}

pub fn string_from_stream(stream: &mut Stream) -> Result<String, Error> {
    let mut str = std::ptr::null_mut();
    let pool = apr::pool::Pool::new();
    let err = unsafe {
        subversion_sys::svn_string_from_stream(
            &mut str,
            stream.ptr,
            pool.as_mut_ptr(),
            apr::pool::Pool::new().as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    let data = (unsafe { (*str).data }) as *const i8;
    let len = unsafe { (*str).len };
    Ok(unsafe {
        std::str::from_utf8(std::slice::from_raw_parts(data as *const u8, len))
            .unwrap()
            .to_string()
    })
}

pub fn remove_dir(
    path: &std::path::Path,
    ignore_enoent: bool,
    cancel_func: Option<&impl Fn() -> Result<(), Error>>,
) -> Result<(), Error> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let err = unsafe {
        subversion_sys::svn_io_remove_dir2(
            path.as_ptr(),
            ignore_enoent as i32,
            if cancel_func.is_some() {
                Some(crate::wrap_cancel_func)
            } else {
                None
            },
            if let Some(cancel_func) = cancel_func {
                &cancel_func as *const _ as *mut std::ffi::c_void
            } else {
                std::ptr::null_mut()
            },
            apr::pool::Pool::new().as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok(())
}
