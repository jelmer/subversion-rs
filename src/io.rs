use crate::generated::svn_io_dirent2_t;
use crate::Error;

pub struct Dirent(*const svn_io_dirent2_t);

impl From<*const svn_io_dirent2_t> for Dirent {
    fn from(ptr: *const svn_io_dirent2_t) -> Self {
        Self(ptr)
    }
}

pub struct Stream<'pool>(apr::pool::PooledPtr<'pool, crate::generated::svn_stream_t>);

impl<'pool> Stream<'pool> {
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
            Ok(stream)
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
            Ok(stream)
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
            Ok(stream)
        })?))
    }

    pub fn stderr() -> Result<Self, Error> {
        Ok(Self(apr::pool::PooledPtr::initialize(|pool| {
            let mut stream = std::ptr::null_mut();
            let err =
                unsafe { crate::generated::svn_stream_for_stderr(&mut stream, pool.as_mut_ptr()) };
            Error::from_raw(err)?;
            Ok(stream)
        })?))
    }

    pub fn stdout() -> Result<Self, Error> {
        Ok(Self(apr::pool::PooledPtr::initialize(|pool| {
            let mut stream = std::ptr::null_mut();
            let err =
                unsafe { crate::generated::svn_stream_for_stdout(&mut stream, pool.as_mut_ptr()) };
            Error::from_raw(err)?;
            Ok(stream)
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

pub fn tee<'pool>(
    out1: &'pool mut Stream,
    out2: &'pool mut Stream,
) -> Result<Stream<'pool>, Error> {
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

impl From<&[u8]> for Stream<'_> {
    fn from(bytes: &[u8]) -> Self {
        Self(
            apr::pool::PooledPtr::initialize(|pool| {
                let mut buf = crate::generated::svn_string_t {
                    data: bytes.as_ptr() as *mut i8,
                    len: bytes.len() as usize,
                };
                let stream = unsafe {
                    crate::generated::svn_stream_from_string(&mut buf, pool.as_mut_ptr())
                };
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
