use crate::generated::svn_io_dirent2_t;
use crate::Error;

pub struct Dirent(*const svn_io_dirent2_t);

impl From<*const svn_io_dirent2_t> for Dirent {
    fn from(ptr: *const svn_io_dirent2_t) -> Self {
        Self(ptr)
    }
}

pub struct Stream(apr::pool::PooledPtr<crate::generated::svn_stream_t>);

impl Stream {
    pub fn empty() -> Self {
        Self(
            apr::pool::PooledPtr::initialize(|pool| {
                let stream = unsafe { crate::generated::svn_stream_empty(pool.as_mut_ptr()) };
                Ok::<_, crate::Error>(stream)
            })
            .unwrap(),
        )
    }

    pub fn as_mut_ptr(&mut self) -> *mut crate::generated::svn_stream_t {
        self.0.as_mut_ptr()
    }

    pub fn as_ptr(&self) -> *const crate::generated::svn_stream_t {
        self.0.as_ptr()
    }

    pub fn open_readonly(path: &std::path::Path) -> Result<Self, Error> {
        Ok(Self(apr::pool::PooledPtr::initialize(|pool| {
            let mut stream = std::ptr::null_mut();
            let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
            let err = unsafe {
                crate::generated::svn_stream_open_readonly(
                    &mut stream,
                    path.as_ptr(),
                    pool.as_mut_ptr(),
                    apr::pool::Pool::new().as_mut_ptr(),
                )
            };
            Error::from_raw(err)?;
            Ok::<_, Error>(stream)
        })?))
    }

    pub fn open_writable(path: &std::path::Path) -> Result<Self, Error> {
        Ok(Self(apr::pool::PooledPtr::initialize(|pool| {
            let mut stream = std::ptr::null_mut();
            let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
            let err = unsafe {
                crate::generated::svn_stream_open_writable(
                    &mut stream,
                    path.as_ptr(),
                    pool.as_mut_ptr(),
                    apr::pool::Pool::new().as_mut_ptr(),
                )
            };
            Error::from_raw(err)?;
            Ok::<_, Error>(stream)
        })?))
    }

    pub fn stdin(buffered: bool) -> Result<Self, Error> {
        Ok(Self(apr::pool::PooledPtr::initialize(|pool| {
            let mut stream = std::ptr::null_mut();
            let err = unsafe {
                crate::generated::svn_stream_for_stdin2(
                    &mut stream,
                    buffered as i32,
                    pool.as_mut_ptr(),
                )
            };
            Error::from_raw(err)?;
            Ok::<_, Error>(stream)
        })?))
    }

    pub fn stderr() -> Result<Self, Error> {
        Ok(Self(apr::pool::PooledPtr::initialize(|pool| {
            let mut stream = std::ptr::null_mut();
            let err =
                unsafe { crate::generated::svn_stream_for_stderr(&mut stream, pool.as_mut_ptr()) };
            Error::from_raw(err)?;
            Ok::<_, Error>(stream)
        })?))
    }

    pub fn stdout() -> Result<Self, Error> {
        Ok(Self(apr::pool::PooledPtr::initialize(|pool| {
            let mut stream = std::ptr::null_mut();
            let err =
                unsafe { crate::generated::svn_stream_for_stdout(&mut stream, pool.as_mut_ptr()) };
            Error::from_raw(err)?;
            Ok::<_, Error>(stream)
        })?))
    }

    pub fn buffered() -> Self {
        Self(
            apr::pool::PooledPtr::initialize(|pool| {
                let stream = unsafe { crate::generated::svn_stream_buffered(pool.as_mut_ptr()) };
                Ok::<_, crate::Error>(stream)
            })
            .unwrap(),
        )
    }

    pub fn compressed(&mut self) -> Self {
        Self(
            apr::pool::PooledPtr::initialize(|pool| {
                let stream = unsafe {
                    crate::generated::svn_stream_compressed(self.0.as_mut_ptr(), pool.as_mut_ptr())
                };
                Ok::<_, crate::Error>(stream)
            })
            .unwrap(),
        )
    }

    pub fn supports_partial_read(&mut self) -> bool {
        unsafe { crate::generated::svn_stream_supports_partial_read(self.0.as_mut_ptr()) != 0 }
    }

    pub fn supports_mark(&mut self) -> bool {
        unsafe { crate::generated::svn_stream_supports_mark(self.0.as_mut_ptr()) != 0 }
    }

    pub fn supports_reset(&mut self) -> bool {
        unsafe { crate::generated::svn_stream_supports_reset(self.0.as_mut_ptr()) != 0 }
    }

    pub fn data_available(&mut self) -> Result<bool, Error> {
        let mut data_available = 0;
        let err = unsafe {
            crate::generated::svn_stream_data_available(self.0.as_mut_ptr(), &mut data_available)
        };
        Error::from_raw(err)?;
        Ok(data_available != 0)
    }

    pub fn puts(&mut self, s: &str) -> Result<(), Error> {
        let s = std::ffi::CString::new(s).unwrap();
        let err = unsafe { crate::generated::svn_stream_puts(self.0.as_mut_ptr(), s.as_ptr()) };

        Error::from_raw(err)?;
        Ok(())
    }
}

impl std::io::Write for Stream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut len = 0;
        let err = unsafe {
            crate::generated::svn_stream_write(
                self.0.as_mut_ptr(),
                buf.as_ptr() as *const i8,
                &mut len,
            )
        };
        Error::from_raw(err)?;
        Ok(len)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl std::io::Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut len = 0;
        let err = unsafe {
            crate::generated::svn_stream_read(
                self.0.as_mut_ptr(),
                buf.as_mut_ptr() as *mut i8,
                &mut len,
            )
        };
        Error::from_raw(err)?;
        Ok(len)
    }
}

pub fn tee(out1: &mut Stream, out2: &mut Stream) -> Result<Stream, Error> {
    Ok(Stream(apr::pool::PooledPtr::initialize(|pool| {
        let stream = unsafe {
            crate::generated::svn_stream_tee(
                out1.0.as_mut_ptr(),
                out2.0.as_mut_ptr(),
                pool.as_mut_ptr(),
            )
        };
        Ok::<_, crate::Error>(stream)
    })?))
}

impl From<&[u8]> for Stream {
    fn from(bytes: &[u8]) -> Self {
        Self(
            apr::pool::PooledPtr::initialize(|pool| {
                let buf = crate::generated::svn_string_t {
                    data: bytes.as_ptr() as *mut i8,
                    len: bytes.len(),
                };
                let stream =
                    unsafe { crate::generated::svn_stream_from_string(&buf, pool.as_mut_ptr()) };
                Ok::<_, crate::Error>(stream)
            })
            .unwrap(),
        )
    }
}

pub fn remove_file(path: &std::path::Path, ignore_enoent: bool) -> Result<(), Error> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let err = unsafe {
        crate::generated::svn_io_remove_file2(
            path.as_ptr(),
            ignore_enoent as i32,
            apr::pool::Pool::new().as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok(())
}

pub fn wrap_write(write: &mut dyn std::io::Write) -> Result<Stream, Error> {
    let write = Box::into_raw(Box::new(write));
    let mut stream = apr::pool::PooledPtr::initialize(|pool| {
        let stream = unsafe {
            crate::generated::svn_stream_create(write as *mut std::ffi::c_void, pool.as_mut_ptr())
        };
        Ok::<_, Error>(stream)
    })?;

    extern "C" fn write_fn(
        baton: *mut std::ffi::c_void,
        buffer: *const i8,
        len: *mut usize,
    ) -> *mut crate::generated::svn_error_t {
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

    extern "C" fn close_fn(baton: *mut std::ffi::c_void) -> *mut crate::generated::svn_error_t {
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
        crate::generated::svn_stream_set_write(stream.as_mut_ptr(), Some(write_fn));
        crate::generated::svn_stream_set_close(stream.as_mut_ptr(), Some(close_fn));
    }

    Ok(Stream(stream))
}
