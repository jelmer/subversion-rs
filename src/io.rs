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

    pub fn buffered() -> Self {
        let pool = apr::Pool::new();
        let stream = unsafe { subversion_sys::svn_stream_buffered(pool.as_mut_ptr()) };
        Self {
            ptr: stream,
            pool,
            _phantom: PhantomData,
        }
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

    pub fn supports_reset(&mut self) -> bool {
        unsafe { subversion_sys::svn_stream_supports_reset(self.ptr) != 0 }
    }

    pub fn reset(&mut self) -> Result<(), Error> {
        let err = unsafe { subversion_sys::svn_stream_reset(self.ptr) };
        Error::from_raw(err)?;
        Ok(())
    }

    pub fn data_available(&mut self) -> Result<bool, Error> {
        let mut data_available = 0;
        let err =
            unsafe { subversion_sys::svn_stream_data_available(self.ptr, &mut data_available) };
        Error::from_raw(err)?;
        Ok(data_available != 0)
    }

    pub fn puts(&mut self, s: &str) -> Result<(), Error> {
        let s = std::ffi::CString::new(s).unwrap();
        let err = unsafe { subversion_sys::svn_stream_puts(self.ptr, s.as_ptr()) };

        Error::from_raw(err)?;
        Ok(())
    }

    pub fn readline(&mut self, eol: &str) -> Result<String, Error> {
        let eol = std::ffi::CString::new(eol).unwrap();
        let mut line = std::ptr::null_mut();
        let pool = apr::pool::Pool::new();
        let mut eof = 0;
        let err = unsafe {
            subversion_sys::svn_stream_readline(
                self.ptr,
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
            subversion_sys::svn_stream_write(self.ptr, buf.as_ptr() as *const i8, &mut len)
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
            subversion_sys::svn_stream_read(self.ptr, buf.as_mut_ptr() as *mut i8, &mut len)
        };
        Error::from_raw(err)?;
        Ok(len)
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_stream_create_and_write() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");

        // Create and write to a stream
        let mut stream = Stream::open_writable(&file_path).unwrap();
        let data = b"Hello, world!";
        assert!(stream.write(data).is_ok());
        assert!(stream.close().is_ok());

        // Verify file was created
        assert!(file_path.exists());
    }

    #[test]
    fn test_stream_read() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");

        // Write some data first
        std::fs::write(&file_path, b"Test data").unwrap();

        // Read using Stream
        let mut stream = Stream::open_readonly(&file_path).unwrap();
        let mut buf = vec![0u8; 9];
        let bytes_read = stream.read(&mut buf).unwrap();
        assert_eq!(bytes_read, 9);
        assert_eq!(&buf[..bytes_read], b"Test data");
    }

    #[test]
    fn test_stream_from_bytes() {
        let data = b"Test string data";
        let stream = Stream::from(data.as_ref());
        // Should be able to create stream from bytes
        assert!(!stream.as_ptr().is_null());
    }

    #[test]
    fn test_stream_stdin() {
        // Just test that we can create stdin stream without panic
        let result = Stream::stdin(false);
        assert!(result.is_ok());
    }

    #[test]
    fn test_stream_stdout() {
        // Just test that we can create stdout stream without panic
        let result = Stream::stdout();
        assert!(result.is_ok());
    }

    #[test]
    fn test_stream_stderr() {
        // Just test that we can create stderr stream without panic
        let result = Stream::stderr();
        assert!(result.is_ok());
    }

    #[test]
    fn test_stream_mark_and_seek() {
        let data = b"Test data for seeking";
        let mut stream = Stream::from(data.as_ref());

        // Check if stream supports marking
        if stream.supports_mark() {
            // Create a mark
            let mark = stream.mark();
            assert!(mark.is_ok());

            // Read some data
            let mut buf = vec![0u8; 4];
            let _ = stream.read(&mut buf);

            // Seek back to mark
            if let Ok(mark) = mark {
                assert!(stream.seek(&mark).is_ok());
            }
        }
    }

    #[test]
    fn test_stream_reset() {
        let data = b"Reset test data";
        let mut stream = Stream::from(data.as_ref());

        // Check if stream supports reset
        if stream.supports_reset() {
            // Read some data
            let mut buf = vec![0u8; 5];
            let _ = stream.read(&mut buf);

            // Reset stream
            assert!(stream.reset().is_ok());
        }
    }

    #[test]
    fn test_remove_dir() {
        let dir = tempdir().unwrap();
        let test_dir = dir.path().join("test_dir");
        std::fs::create_dir(&test_dir).unwrap();

        // Remove the directory
        let result = remove_dir(&test_dir, false, None::<&fn() -> Result<(), Error>>);
        assert!(result.is_ok());

        // Directory should no longer exist
        assert!(!test_dir.exists());
    }
}
