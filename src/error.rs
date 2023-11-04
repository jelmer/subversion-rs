use crate::generated;
use generated::svn_error_t;

// Errors are a bit special; they own their own pool, so don't need to use PooledPtr
pub struct Error(*mut svn_error_t);

impl Error {
    pub fn new(status: apr::Status, child: Option<Error>, msg: &str) -> Self {
        let msg = std::ffi::CString::new(msg).unwrap();
        let child = child.map(|e| e.0).unwrap_or(std::ptr::null_mut());
        let err = unsafe { generated::svn_error_create(status as i32, child, msg.as_ptr()) };
        Self(err)
    }

    pub fn apr_err(&self) -> i32 {
        unsafe { (*self.0).apr_err }
    }

    pub fn as_mut_ptr(&mut self) -> *mut svn_error_t {
        self.0
    }

    pub fn as_ptr(&self) -> *const svn_error_t {
        self.0
    }

    pub fn from_raw(err: *mut svn_error_t) -> Result<(), Self> {
        if err.is_null() {
            Ok(())
        } else {
            Err(Self(err))
        }
    }

    pub fn line(&self) -> i64 {
        unsafe { (*self.0).line }
    }

    pub fn file(&self) -> Option<&str> {
        unsafe {
            let file = (*self.0).file;
            if file.is_null() {
                None
            } else {
                Some(std::ffi::CStr::from_ptr(file).to_str().unwrap())
            }
        }
    }

    pub fn child<'a>(&'a self) -> Option<Self> {
        unsafe {
            let child = (*self.0).child;
            if child.is_null() {
                None
            } else {
                Some(Error(child))
            }
        }
    }

    pub fn message<'a>(&'a self) -> &'a str {
        unsafe {
            let message = (*self.0).message;
            std::ffi::CStr::from_ptr(message).to_str().unwrap()
        }
    }

    pub fn find_cause(&self, status: apr::Status) -> Option<Error> {
        unsafe {
            let err = generated::svn_error_find_cause(self.0, status as i32);
            if err.is_null() {
                None
            } else {
                Some(Error(err))
            }
        }
    }

    pub fn purge_tracing(&self) -> Self {
        unsafe { Self(generated::svn_error_purge_tracing(self.0)) }
    }
}

impl Clone for Error {
    fn clone(&self) -> Self {
        unsafe { Self(generated::svn_error_dup(self.0)) }
    }
}

impl Drop for Error {
    fn drop(&mut self) {
        unsafe { generated::svn_error_clear(self.0) }
    }
}

impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(
            f,
            "{}:{}: {}",
            self.file().unwrap_or("<unspecified>"),
            self.line(),
            self.message()
        )?;
        let mut n = self.child();
        while let Some(err) = n {
            writeln!(
                f,
                "{}:{}: {}",
                err.file().unwrap_or("<unspecified>"),
                err.line(),
                err.message()
            )?;
            n = err.child();
        }
        Ok(())
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(f, "{}", self.message())?;
        Ok(())
    }
}

impl std::error::Error for Error {}
