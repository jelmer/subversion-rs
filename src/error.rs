use subversion_sys::svn_error_t;

// Errors are a bit special; they own their own pool, so don't need to use PooledPtr
pub struct Error(*mut svn_error_t);
unsafe impl Send for Error {}

impl Error {
    pub fn new(status: apr::Status, child: Option<Error>, msg: &str) -> Self {
        let msg = std::ffi::CString::new(msg).unwrap();
        let child = child.map(|e| e.0).unwrap_or(std::ptr::null_mut());
        let err = unsafe { subversion_sys::svn_error_create(status as i32, child, msg.as_ptr()) };
        Self(err)
    }

    pub fn from_str(msg: &str) -> Self {
        Self::new(apr::Status::from(1), None, msg)
    }

    pub fn apr_err(&self) -> apr::Status {
        unsafe { (*self.0).apr_err }.into()
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

    pub fn location(&self) -> Option<(&str, i64)> {
        self.file().map(|f| (f, self.line()))
    }

    pub fn child(&self) -> Option<Self> {
        unsafe {
            let child = (*self.0).child;
            if child.is_null() {
                None
            } else {
                Some(Error(child))
            }
        }
    }

    pub fn message(&self) -> &str {
        unsafe {
            let message = (*self.0).message;
            std::ffi::CStr::from_ptr(message).to_str().unwrap()
        }
    }

    pub fn find_cause(&self, status: apr::Status) -> Option<Error> {
        unsafe {
            let err = subversion_sys::svn_error_find_cause(self.0, status as i32);
            if err.is_null() {
                None
            } else {
                Some(Error(err))
            }
        }
    }

    pub fn purge_tracing(&self) -> Self {
        unsafe { Self(subversion_sys::svn_error_purge_tracing(self.0)) }
    }

    pub unsafe fn detach(&mut self) -> *mut svn_error_t {
        let err = self.0;
        self.0 = std::ptr::null_mut();
        err
    }

    pub unsafe fn into_raw(self) -> *mut svn_error_t {
        let err = self.0;
        std::mem::forget(self);
        err
    }

    pub fn best_message(&self) -> String {
        let mut buf = [0; 1024];
        unsafe {
            let ret = subversion_sys::svn_err_best_message(self.0, buf.as_mut_ptr(), buf.len());
            std::ffi::CStr::from_ptr(ret).to_string_lossy().into_owned()
        }
    }

    /// Collect all messages from the error chain
    pub fn full_message(&self) -> String {
        let mut messages = Vec::new();
        let mut current = self.0;

        unsafe {
            while !current.is_null() {
                let msg = (*current).message;
                if !msg.is_null() {
                    let msg_str = std::ffi::CStr::from_ptr(msg).to_string_lossy();
                    if !msg_str.is_empty() {
                        messages.push(msg_str.into_owned());
                    }
                }
                current = (*current).child;
            }
        }

        if messages.is_empty() {
            self.best_message()
        } else {
            messages.join(": ")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_chain_formatting() {
        // Create a chain of errors
        let child_err = Error::from_str("Child error");
        let parent_err = Error::new(
            apr::Status::from(1),
            Some(child_err),
            "Parent error"
        );
        
        let full_msg = parent_err.full_message();
        assert!(full_msg.contains("Parent error"));
        assert!(full_msg.contains("Child error"));
        assert!(full_msg.contains(": ")); // Check for separator
    }

    #[test]
    fn test_single_error_message() {
        let err = Error::from_str("Single error");
        assert_eq!(err.message(), "Single error");
        
        let full_msg = err.full_message();
        assert!(full_msg.contains("Single error"));
    }

    #[test]
    fn test_error_display() {
        let err = Error::from_str("Display test error");
        let display_str = format!("{}", err);
        assert!(display_str.contains("Display test error"));
    }

    #[test]
    fn test_error_from_raw() {
        // Test with null pointer
        let result = Error::from_raw(std::ptr::null_mut());
        assert!(result.is_ok());
        
        // Test with actual error would require creating a real svn_error_t
        // which is complex, so we skip that for now
    }
}

pub fn symbolic_name(status: apr::Status) -> Option<&'static str> {
    unsafe {
        let name = subversion_sys::svn_error_symbolic_name(status as i32);
        if name.is_null() {
            None
        } else {
            Some(std::ffi::CStr::from_ptr(name).to_str().unwrap())
        }
    }
}

pub fn strerror(status: apr::Status) -> Option<&'static str> {
    let mut buf = [0; 1024];
    unsafe {
        let name = subversion_sys::svn_strerror(status as i32, buf.as_mut_ptr(), buf.len());
        if name.is_null() {
            None
        } else {
            Some(std::ffi::CStr::from_ptr(name).to_str().unwrap())
        }
    }
}

impl Clone for Error {
    fn clone(&self) -> Self {
        unsafe { Self(subversion_sys::svn_error_dup(self.0)) }
    }
}

impl Drop for Error {
    fn drop(&mut self) {
        unsafe { subversion_sys::svn_error_clear(self.0) }
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
        write!(f, "{}", self.full_message())
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::new(apr::Status::from(err.kind()), None, &err.to_string())
    }
}

impl From<Error> for std::io::Error {
    fn from(err: Error) -> Self {
        let errno = err.apr_err().raw_os_error();
        errno.map_or(
            std::io::Error::new(std::io::ErrorKind::Other, err.message()),
            std::io::Error::from_raw_os_error,
        )
    }
}
