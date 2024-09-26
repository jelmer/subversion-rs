use crate::generated::svn_io_dirent2_t;
use crate::Error;
use std::ffi::OsStr;

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

impl From<FileDel> for crate::generated::svn_io_file_del_t {
    fn from(file_del: FileDel) -> Self {
        match file_del {
            FileDel::None => crate::generated::svn_io_file_del_t_svn_io_file_del_none,
            FileDel::OnClose => crate::generated::svn_io_file_del_t_svn_io_file_del_on_close,
            FileDel::OnPoolCleanup => {
                crate::generated::svn_io_file_del_t_svn_io_file_del_on_pool_cleanup
            }
        }
    }
}

impl From<crate::generated::svn_io_file_del_t> for FileDel {
    fn from(file_del: crate::generated::svn_io_file_del_t) -> Self {
        match file_del {
            crate::generated::svn_io_file_del_t_svn_io_file_del_none => FileDel::None,
            crate::generated::svn_io_file_del_t_svn_io_file_del_on_close => FileDel::OnClose,
            crate::generated::svn_io_file_del_t_svn_io_file_del_on_pool_cleanup => {
                FileDel::OnPoolCleanup
            }
            _ => unreachable!(),
        }
    }
}

pub struct Mark(apr::pool::PooledPtr<crate::generated::svn_stream_mark_t>);

impl Mark {
    pub fn as_mut_ptr(&mut self) -> *mut crate::generated::svn_stream_mark_t {
        self.0.as_mut_ptr()
    }

    pub fn as_ptr(&self) -> *const crate::generated::svn_stream_mark_t {
        self.0.as_ptr()
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

    pub fn open_unique(
        dirpath: &std::path::Path,
        when: FileDel,
    ) -> Result<(std::path::PathBuf, Self), Error> {
        let dirpath = std::ffi::CString::new(dirpath.to_str().unwrap()).unwrap();
        let mut stream = std::ptr::null_mut();
        let mut path = std::ptr::null();
        let pool = apr::pool::Pool::new();
        let err = unsafe {
            crate::generated::svn_stream_open_unique(
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
            Self(unsafe { apr::pool::PooledPtr::in_pool(std::rc::Rc::new(pool), stream) }),
        ))
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

    pub fn checksummed(
        &mut self,
        checksum_kind: crate::ChecksumKind,
        read_all: bool,
    ) -> (crate::io::Stream, crate::Checksum, crate::Checksum) {
        let mut read_checksum = std::ptr::null_mut();
        let mut write_checksum = std::ptr::null_mut();
        let stream = unsafe {
            crate::generated::svn_stream_checksummed2(
                self.0.as_mut_ptr(),
                &mut read_checksum,
                &mut write_checksum,
                checksum_kind.into(),
                read_all as i32,
                apr::pool::Pool::new().as_mut_ptr(),
            )
        };
        let pool = std::rc::Rc::new(apr::pool::Pool::new());
        (
            crate::io::Stream(unsafe { apr::pool::PooledPtr::in_pool(pool.clone(), stream) }),
            crate::Checksum(unsafe { apr::pool::PooledPtr::in_pool(pool.clone(), read_checksum) }),
            crate::Checksum(unsafe { apr::pool::PooledPtr::in_pool(pool, write_checksum) }),
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

    pub fn contents_checksum(
        &mut self,
        checksum_kind: crate::ChecksumKind,
    ) -> Result<crate::Checksum, Error> {
        let mut checksum = std::ptr::null_mut();
        let pool = apr::pool::Pool::new();
        let err = unsafe {
            crate::generated::svn_stream_contents_checksum(
                &mut checksum,
                self.0.as_mut_ptr(),
                checksum_kind.into(),
                pool.as_mut_ptr(),
                apr::pool::Pool::new().as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(crate::Checksum(unsafe {
            apr::pool::PooledPtr::in_pool(std::rc::Rc::new(apr::pool::Pool::new()), checksum)
        }))
    }

    pub fn read_full(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        let mut len = buf.len();
        let err = unsafe {
            crate::generated::svn_stream_read_full(
                self.0.as_mut_ptr(),
                buf.as_mut_ptr() as *mut i8,
                &mut len,
            )
        };
        Error::from_raw(err)?;
        Ok(len)
    }

    pub fn supports_partial_read(&mut self) -> bool {
        unsafe { crate::generated::svn_stream_supports_partial_read(self.0.as_mut_ptr()) != 0 }
    }

    pub fn skip(&mut self, len: usize) -> Result<(), Error> {
        let err = unsafe { crate::generated::svn_stream_skip(self.0.as_mut_ptr(), len) };
        Error::from_raw(err)?;
        Ok(())
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        let mut len = buf.len();
        let err = unsafe {
            crate::generated::svn_stream_read2(
                self.0.as_mut_ptr(),
                buf.as_mut_ptr() as *mut i8,
                &mut len,
            )
        };
        Error::from_raw(err)?;
        Ok(len)
    }

    pub fn write(&mut self, buf: &[u8]) -> Result<(), Error> {
        let mut len = buf.len();
        let err = unsafe {
            crate::generated::svn_stream_write(
                self.0.as_mut_ptr(),
                buf.as_ptr() as *const i8,
                &mut len,
            )
        };
        Error::from_raw(err)?;
        Ok(())
    }

    pub fn close(&mut self) -> Result<(), Error> {
        let err = unsafe { crate::generated::svn_stream_close(self.0.as_mut_ptr()) };
        Error::from_raw(err)?;
        Ok(())
    }

    pub fn supports_mark(&mut self) -> bool {
        unsafe { crate::generated::svn_stream_supports_mark(self.0.as_mut_ptr()) != 0 }
    }

    pub fn mark(&mut self) -> Result<Mark, Error> {
        let mut mark = std::ptr::null_mut();
        let pool = apr::pool::Pool::new();
        let err = unsafe {
            crate::generated::svn_stream_mark(self.0.as_mut_ptr(), &mut mark, pool.as_mut_ptr())
        };
        Error::from_raw(err)?;
        Ok(Mark(unsafe {
            apr::pool::PooledPtr::in_pool(std::rc::Rc::new(pool), mark)
        }))
    }

    pub fn seek(&mut self, mark: &Mark) -> Result<(), Error> {
        let err = unsafe { crate::generated::svn_stream_seek(self.0.as_mut_ptr(), mark.as_ptr()) };
        Error::from_raw(err)?;
        Ok(())
    }

    pub fn supports_reset(&mut self) -> bool {
        unsafe { crate::generated::svn_stream_supports_reset(self.0.as_mut_ptr()) != 0 }
    }

    pub fn reset(&mut self) -> Result<(), Error> {
        let err = unsafe { crate::generated::svn_stream_reset(self.0.as_mut_ptr()) };
        Error::from_raw(err)?;
        Ok(())
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

    pub fn readline(&mut self, eol: &str) -> Result<String, Error> {
        let eol = std::ffi::CString::new(eol).unwrap();
        let mut line = std::ptr::null_mut();
        let pool = apr::pool::Pool::new();
        let mut eof = 0;
        let err = unsafe {
            crate::generated::svn_stream_readline(
                self.0.as_mut_ptr(),
                &mut line,
                eol.as_ptr(),
                &mut eof,
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        let data = (unsafe { (*line).data }) as *const i8;
        let len = unsafe { (*line).len };
        Ok(unsafe {
            std::str::from_utf8(std::slice::from_raw_parts(data as *const u8, len))
                .unwrap()
                .to_string()
        })
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
        crate::generated::svn_io_create_unique_link(
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
        crate::generated::svn_io_read_link(
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
    let err = unsafe { crate::generated::svn_io_temp_dir(&mut path, pool.as_mut_ptr()) };
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
        crate::generated::svn_io_copy_file(
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
        crate::generated::svn_io_copy_perms(
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
        crate::generated::svn_io_copy_link(
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
        crate::generated::svn_io_copy_dir_recursively(
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
        crate::generated::svn_io_make_dir_recursively(
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
        crate::generated::svn_io_dir_empty(
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
        crate::generated::svn_io_append_file(
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
        crate::generated::svn_io_set_file_read_only(
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
        crate::generated::svn_io_set_file_read_write(
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
        crate::generated::svn_io_is_file_executable(
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
        crate::generated::svn_io_file_affected_time(
            &mut affected_time,
            path.as_ptr(),
            apr::pool::Pool::new().as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok(apr::time::Time::from(affected_time).into())
}

pub fn set_file_affected_time(
    path: &std::path::Path,
    affected_time: apr::time::Time,
) -> Result<(), Error> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let affected_time = affected_time.into();
    let err = unsafe {
        crate::generated::svn_io_set_file_affected_time(
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
        crate::generated::svn_io_sleep_for_timestamps(
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
        crate::generated::svn_io_filesizes_different_p(
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
        crate::generated::svn_io_filesizes_three_different_p(
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
        crate::generated::svn_io_file_checksum2(
            &mut checksum,
            file.as_ptr(),
            checksum_kind.into(),
            pool.as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok(crate::Checksum(unsafe {
        apr::pool::PooledPtr::in_pool(std::rc::Rc::new(pool), checksum)
    }))
}

pub fn files_contents_same_p(
    file1: &std::path::Path,
    file2: &std::path::Path,
) -> Result<bool, Error> {
    let file1 = std::ffi::CString::new(file1.to_str().unwrap()).unwrap();
    let file2 = std::ffi::CString::new(file2.to_str().unwrap()).unwrap();
    let mut same = 0;
    let err = unsafe {
        crate::generated::svn_io_files_contents_same_p(
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
        crate::generated::svn_io_files_contents_three_same_p(
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
        crate::generated::svn_io_file_create(
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
        crate::generated::svn_io_file_create_bytes(
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
        crate::generated::svn_io_file_create_empty(
            path.as_ptr(),
            apr::pool::Pool::new().as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok(())
}

pub fn file_lock(path: &std::path::Path, exclusive: bool, nonblocking: bool) -> Result<(), Error> {
    let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
    let err = unsafe {
        crate::generated::svn_io_file_lock2(
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
        crate::generated::svn_io_dir_file_copy(
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
        crate::generated::svn_stream_copy3(
            from.0.as_mut_ptr(),
            to.0.as_mut_ptr(),
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
        crate::generated::svn_stream_contents_same(
            &mut same,
            stream1.0.as_mut_ptr(),
            stream2.0.as_mut_ptr(),
            apr::pool::Pool::new().as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok(same != 0)
}

pub fn string_from_stream(stream: &mut Stream) -> Result<String, Error> {
    let mut str = std::ptr::null_mut();
    let pool = apr::pool::Pool::new();
    let err = unsafe {
        crate::generated::svn_string_from_stream(
            &mut str,
            stream.0.as_mut_ptr(),
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
        crate::generated::svn_io_remove_dir2(
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
