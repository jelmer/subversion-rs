use subversion_sys::svn_error_t;

/// Categorizes the kind of error that occurred based on SVN error code ranges.
///
/// This enum provides a way to programmatically distinguish between different
/// error categories without parsing error messages. The categories correspond
/// to SVN's internal error code organization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCategory {
    /// Malformed input or argument errors (125000-129999)
    BadInput,
    /// XML parsing/generation errors (130000-134999)
    Xml,
    /// I/O errors (135000-139999)
    Io,
    /// Stream-related errors (140000-144999)
    Stream,
    /// Node (file/directory) errors (145000-149999)
    Node,
    /// Entry-related errors (150000-154999)
    Entry,
    /// Working copy errors (155000-159999)
    WorkingCopy,
    /// Filesystem backend errors (160000-164999)
    Filesystem,
    /// Repository errors (165000-169999)
    Repository,
    /// Repository access layer errors (170000-174999)
    RepositoryAccess,
    /// DAV protocol errors (175000-179999)
    RaDav,
    /// Local repository access errors (180000-184999)
    RaLocal,
    /// Diff algorithm errors (185000-189999)
    Svndiff,
    /// Apache module errors (190000-194999)
    ApacheMod,
    /// Client operation errors (195000-199999)
    Client,
    /// Miscellaneous errors including cancellation (200000-204999)
    Misc,
    /// Command-line client errors (205000-209999)
    CommandLine,
    /// SVN protocol errors (210000-214999)
    RaSvn,
    /// Authentication errors (215000-219999)
    Authentication,
    /// Authorization errors (220000-224999)
    Authorization,
    /// Diff operation errors (225000-229999)
    Diff,
    /// Serf/HTTP errors (230000-234999)
    RaSerf,
    /// Internal malfunction errors (235000-239999)
    Malfunction,
    /// X.509 certificate errors (240000-244999)
    X509,
    /// Unknown or APR error
    Other,
}

// Errors are a bit special; they own their own pool, so don't need to use PooledPtr
/// Represents a Subversion error.
///
/// SVN errors can form chains where each error points to a child error that provides
/// more context. The lifetime parameter tracks ownership of the error chain:
///
/// - `Error<'static>` owns its error pointer and will free the entire chain on drop
/// - `Error<'a>` borrows from another error's chain and shares the pointer without owning it
///
/// # Examples
///
/// Creating a simple error:
/// ```
/// use subversion::Error;
///
/// let err = Error::from_message("Something went wrong");
/// ```
///
/// Checking error details:
/// ```
/// # use subversion::Error;
/// # let err = Error::from_message("Something went wrong");
/// println!("Error code: {}", err.code());
/// println!("Error message: {}", err.message());
/// println!("Error category: {:?}", err.category());
/// ```
///
/// Traversing an error chain:
/// ```
/// # use subversion::Error;
/// # let err = Error::from_message("Something went wrong");
/// let mut current = Some(&err);
/// while let Some(e) = current {
///     println!("Error: {}", e.message());
///     current = e.child().as_ref();
/// }
/// ```
pub struct Error<'a> {
    ptr: *mut svn_error_t,
    owns_ptr: bool,
    _phantom: std::marker::PhantomData<&'a ()>,
}

unsafe impl Send for Error<'_> {}

impl Error<'static> {
    /// Creates a new error with the given status, optional child error, and message.
    pub fn new(status: apr::Status, child: Option<Error<'static>>, msg: &str) -> Self {
        let msg = std::ffi::CString::new(msg).unwrap();
        let child = child
            .map(|mut e| unsafe { e.detach() })
            .unwrap_or(std::ptr::null_mut());
        let err = unsafe { subversion_sys::svn_error_create(status as i32, child, msg.as_ptr()) };
        Self {
            ptr: err,
            owns_ptr: true,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Creates a new error with a raw APR/SVN status code.
    ///
    /// Use this when you need SVN-specific error codes (like `SVN_ERR_CANCELLED`)
    /// that cannot be represented by `apr::Status`.
    pub fn with_raw_status(status: i32, child: Option<Error<'static>>, msg: &str) -> Self {
        let msg = std::ffi::CString::new(msg).unwrap();
        let child = child
            .map(|mut e| unsafe { e.detach() })
            .unwrap_or(std::ptr::null_mut());
        let err = unsafe { subversion_sys::svn_error_create(status, child, msg.as_ptr()) };
        Self {
            ptr: err,
            owns_ptr: true,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Creates a new error from a string message.
    pub fn from_message(msg: &str) -> Error<'static> {
        Self::new(apr::Status::from(1), None, msg)
    }

    /// Creates an error from a raw SVN error pointer, or Ok if null.
    pub fn from_raw(err: *mut svn_error_t) -> Result<(), Error<'static>> {
        if err.is_null() {
            Ok(())
        } else {
            Err(Error {
                ptr: err,
                owns_ptr: true,
                _phantom: std::marker::PhantomData,
            })
        }
    }
}

impl<'a> Error<'a> {
    /// Wraps a raw SVN error pointer without taking ownership.
    ///
    /// The caller remains responsible for freeing `err`; the returned `Error`
    /// will NOT call `svn_error_clear` on drop.
    ///
    /// # Safety
    ///
    /// `err` must be a valid, non-null pointer that outlives the returned `Error<'a>`.
    pub(crate) unsafe fn from_ptr_borrowed(err: *mut svn_error_t) -> Error<'a> {
        debug_assert!(!err.is_null());
        Error {
            ptr: err,
            owns_ptr: false,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<'a> Error<'a> {
    /// Gets the APR error status code.
    ///
    /// Note: SVN-specific error codes (like `SVN_ERR_CANCELLED`) are mapped to
    /// `apr::Status::General` because they fall outside the standard APR status range.
    /// Use [`raw_apr_err()`](Self::raw_apr_err) when you need to distinguish SVN error codes.
    pub fn apr_err(&self) -> apr::Status {
        unsafe { (*self.ptr).apr_err }.into()
    }

    /// Gets the raw APR/SVN error status code as an integer.
    ///
    /// Unlike [`apr_err()`](Self::apr_err), this preserves the full error code
    /// including SVN-specific codes (e.g. `SVN_ERR_CANCELLED = 200015`).
    pub fn raw_apr_err(&self) -> i32 {
        unsafe { (*self.ptr).apr_err }
    }

    /// Gets the mutable raw pointer to the error.
    pub fn as_mut_ptr(&mut self) -> *mut svn_error_t {
        self.ptr
    }

    /// Gets the raw pointer to the error.
    pub fn as_ptr(&self) -> *const svn_error_t {
        self.ptr
    }

    /// Gets the line number where the error occurred.
    pub fn line(&self) -> i64 {
        unsafe { (*self.ptr).line.into() }
    }

    /// Gets the file name where the error occurred.
    pub fn file(&self) -> Option<&str> {
        unsafe {
            let file = (*self.ptr).file;
            if file.is_null() {
                None
            } else {
                Some(std::ffi::CStr::from_ptr(file).to_str().unwrap())
            }
        }
    }

    /// Gets the file and line location where the error occurred.
    pub fn location(&self) -> Option<(&str, i64)> {
        self.file().map(|f| (f, self.line()))
    }

    /// Gets the child error, if any.
    ///
    /// The returned error has the same lifetime as this error (both are part of the same error chain).
    /// The returned error does not own its pointer - the parent error owns the entire chain.
    pub fn child(&self) -> Option<Error<'a>> {
        unsafe {
            let child = (*self.ptr).child;
            if child.is_null() {
                None
            } else {
                Some(Error {
                    ptr: child,
                    owns_ptr: false,
                    _phantom: std::marker::PhantomData,
                })
            }
        }
    }

    /// Gets the error message.
    pub fn message(&self) -> Option<&str> {
        unsafe {
            let message = (*self.ptr).message;
            if message.is_null() {
                None
            } else {
                Some(std::ffi::CStr::from_ptr(message).to_str().unwrap())
            }
        }
    }

    /// Finds an error in the chain with the given status code.
    ///
    /// The returned error borrows from this error's chain and does not own its pointer.
    pub fn find_cause(&self, status: apr::Status) -> Option<Error<'a>> {
        unsafe {
            let err = subversion_sys::svn_error_find_cause(self.ptr, status as i32);
            if err.is_null() {
                None
            } else {
                Some(Error {
                    ptr: err,
                    owns_ptr: false,
                    _phantom: std::marker::PhantomData,
                })
            }
        }
    }

    /// Removes tracing information from the error.
    ///
    /// The returned error borrows from this error's chain and does not own its pointer.
    pub fn purge_tracing(&self) -> Error<'_> {
        unsafe {
            Error {
                ptr: subversion_sys::svn_error_purge_tracing(self.ptr),
                owns_ptr: false,
                _phantom: std::marker::PhantomData,
            }
        }
    }

    /// Detaches the error, returning the raw pointer and preventing cleanup.
    ///
    /// # Safety
    ///
    /// The caller assumes responsibility for managing the returned pointer's lifetime
    /// and ensuring it is properly freed using Subversion's error handling functions.
    pub unsafe fn detach(&mut self) -> *mut svn_error_t {
        let err = self.ptr;
        self.ptr = std::ptr::null_mut();
        err
    }

    /// Converts the error into a raw pointer, consuming self without cleanup.
    ///
    /// # Safety
    ///
    /// The caller assumes responsibility for managing the returned pointer's lifetime
    /// and ensuring it is properly freed using Subversion's error handling functions.
    pub unsafe fn into_raw(self) -> *mut svn_error_t {
        let err = self.ptr;
        std::mem::forget(self);
        err
    }

    /// Gets the best available error message from the error chain.
    pub fn best_message(&self) -> String {
        let mut buf = [0; 1024];
        unsafe {
            let ret = subversion_sys::svn_err_best_message(self.ptr, buf.as_mut_ptr(), buf.len());
            std::ffi::CStr::from_ptr(ret).to_string_lossy().into_owned()
        }
    }

    /// Collect all messages from the error chain
    pub fn full_message(&self) -> String {
        let mut messages = Vec::new();
        let mut current = self.ptr;

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

    /// Returns the error category based on the SVN error code.
    ///
    /// This allows programmatic handling of different error types without
    /// parsing error messages.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use subversion::error::ErrorCategory;
    /// # fn example() -> Result<(), subversion::Error> {
    /// let mut ctx = subversion::client::Context::new()?;
    /// match ctx.checkout("https://svn.example.com/repo", "/tmp/wc", None, true) {
    ///     Ok(_) => println!("Success"),
    ///     Err(e) => match e.category() {
    ///         ErrorCategory::Authentication => println!("Authentication required"),
    ///         ErrorCategory::Authorization => println!("Permission denied"),
    ///         ErrorCategory::Io => println!("I/O error occurred"),
    ///         _ => println!("Other error: {}", e),
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn category(&self) -> ErrorCategory {
        use subversion_sys::*;
        // Get the raw apr_status_t value directly, not the apr::Status enum discriminant
        let code = unsafe { (*self.ptr).apr_err as u32 };
        let category_size = SVN_ERR_CATEGORY_SIZE;

        match code {
            c if c >= SVN_ERR_BAD_CATEGORY_START
                && c < SVN_ERR_BAD_CATEGORY_START + category_size =>
            {
                ErrorCategory::BadInput
            }
            c if c >= SVN_ERR_XML_CATEGORY_START
                && c < SVN_ERR_XML_CATEGORY_START + category_size =>
            {
                ErrorCategory::Xml
            }
            c if c >= SVN_ERR_IO_CATEGORY_START
                && c < SVN_ERR_IO_CATEGORY_START + category_size =>
            {
                ErrorCategory::Io
            }
            c if c >= SVN_ERR_STREAM_CATEGORY_START
                && c < SVN_ERR_STREAM_CATEGORY_START + category_size =>
            {
                ErrorCategory::Stream
            }
            c if c >= SVN_ERR_NODE_CATEGORY_START
                && c < SVN_ERR_NODE_CATEGORY_START + category_size =>
            {
                ErrorCategory::Node
            }
            c if c >= SVN_ERR_ENTRY_CATEGORY_START
                && c < SVN_ERR_ENTRY_CATEGORY_START + category_size =>
            {
                ErrorCategory::Entry
            }
            c if c >= SVN_ERR_WC_CATEGORY_START
                && c < SVN_ERR_WC_CATEGORY_START + category_size =>
            {
                ErrorCategory::WorkingCopy
            }
            c if c >= SVN_ERR_FS_CATEGORY_START
                && c < SVN_ERR_FS_CATEGORY_START + category_size =>
            {
                ErrorCategory::Filesystem
            }
            c if c >= SVN_ERR_REPOS_CATEGORY_START
                && c < SVN_ERR_REPOS_CATEGORY_START + category_size =>
            {
                ErrorCategory::Repository
            }
            c if c >= SVN_ERR_RA_CATEGORY_START
                && c < SVN_ERR_RA_CATEGORY_START + category_size =>
            {
                ErrorCategory::RepositoryAccess
            }
            c if c >= SVN_ERR_RA_DAV_CATEGORY_START
                && c < SVN_ERR_RA_DAV_CATEGORY_START + category_size =>
            {
                ErrorCategory::RaDav
            }
            c if c >= SVN_ERR_RA_LOCAL_CATEGORY_START
                && c < SVN_ERR_RA_LOCAL_CATEGORY_START + category_size =>
            {
                ErrorCategory::RaLocal
            }
            c if c >= SVN_ERR_SVNDIFF_CATEGORY_START
                && c < SVN_ERR_SVNDIFF_CATEGORY_START + category_size =>
            {
                ErrorCategory::Svndiff
            }
            c if c >= SVN_ERR_APMOD_CATEGORY_START
                && c < SVN_ERR_APMOD_CATEGORY_START + category_size =>
            {
                ErrorCategory::ApacheMod
            }
            c if c >= SVN_ERR_CLIENT_CATEGORY_START
                && c < SVN_ERR_CLIENT_CATEGORY_START + category_size =>
            {
                ErrorCategory::Client
            }
            c if c >= SVN_ERR_MISC_CATEGORY_START
                && c < SVN_ERR_MISC_CATEGORY_START + category_size =>
            {
                ErrorCategory::Misc
            }
            c if c >= SVN_ERR_CL_CATEGORY_START
                && c < SVN_ERR_CL_CATEGORY_START + category_size =>
            {
                ErrorCategory::CommandLine
            }
            c if c >= SVN_ERR_RA_SVN_CATEGORY_START
                && c < SVN_ERR_RA_SVN_CATEGORY_START + category_size =>
            {
                ErrorCategory::RaSvn
            }
            c if c >= SVN_ERR_AUTHN_CATEGORY_START
                && c < SVN_ERR_AUTHN_CATEGORY_START + category_size =>
            {
                ErrorCategory::Authentication
            }
            c if c >= SVN_ERR_AUTHZ_CATEGORY_START
                && c < SVN_ERR_AUTHZ_CATEGORY_START + category_size =>
            {
                ErrorCategory::Authorization
            }
            c if c >= SVN_ERR_DIFF_CATEGORY_START
                && c < SVN_ERR_DIFF_CATEGORY_START + category_size =>
            {
                ErrorCategory::Diff
            }
            c if c >= SVN_ERR_RA_SERF_CATEGORY_START
                && c < SVN_ERR_RA_SERF_CATEGORY_START + category_size =>
            {
                ErrorCategory::RaSerf
            }
            c if c >= SVN_ERR_MALFUNC_CATEGORY_START
                && c < SVN_ERR_MALFUNC_CATEGORY_START + category_size =>
            {
                ErrorCategory::Malfunction
            }
            c if c >= SVN_ERR_X509_CATEGORY_START
                && c < SVN_ERR_X509_CATEGORY_START + category_size =>
            {
                ErrorCategory::X509
            }
            _ => ErrorCategory::Other,
        }
    }
}

/// Gets the symbolic name for an error status code.
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

/// Gets a human-readable error string for a status code.
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

impl Clone for Error<'static> {
    fn clone(&self) -> Self {
        unsafe {
            Error {
                ptr: subversion_sys::svn_error_dup(self.ptr),
                owns_ptr: true,
                _phantom: std::marker::PhantomData,
            }
        }
    }
}

impl Drop for Error<'_> {
    fn drop(&mut self) {
        // Only free if we own the pointer and it's non-null
        if self.owns_ptr && !self.ptr.is_null() {
            unsafe { subversion_sys::svn_error_clear(self.ptr) }
        }
    }
}

impl std::fmt::Debug for Error<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(
            f,
            "{}:{}: {}",
            self.file().unwrap_or("<unspecified>"),
            self.line(),
            self.message().unwrap_or("<no message>")
        )?;
        let mut n = self.child();
        while let Some(err) = n {
            writeln!(
                f,
                "{}:{}: {}",
                err.file().unwrap_or("<unspecified>"),
                err.line(),
                err.message().unwrap_or("<no message>")
            )?;
            n = err.child();
        }
        Ok(())
    }
}

impl std::fmt::Display for Error<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.full_message())
    }
}

impl std::error::Error for Error<'_> {}

impl From<std::io::Error> for Error<'static> {
    fn from(err: std::io::Error) -> Self {
        Error::new(apr::Status::from(err.kind()), None, &err.to_string())
    }
}

impl From<Error<'_>> for std::io::Error {
    fn from(err: Error) -> Self {
        let errno = err.apr_err().raw_os_error();
        errno.map_or(
            std::io::Error::other(err.message().unwrap_or("Unknown error")),
            std::io::Error::from_raw_os_error,
        )
    }
}

impl From<std::ffi::NulError> for Error<'static> {
    fn from(err: std::ffi::NulError) -> Self {
        Error::from_message(&format!("Null byte in string: {}", err))
    }
}

impl From<std::str::Utf8Error> for Error<'static> {
    fn from(err: std::str::Utf8Error) -> Self {
        Error::from_message(&format!("UTF-8 encoding error: {}", err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_chain_formatting() {
        // Create a chain of errors
        let child_err = Error::from_message("Child error");
        let parent_err = Error::new(apr::Status::from(1), Some(child_err), "Parent error");

        let full_msg = parent_err.full_message();
        assert!(full_msg.contains("Parent error"));
        assert!(full_msg.contains("Child error"));
        assert!(full_msg.contains(": ")); // Check for separator
    }

    #[test]
    fn test_single_error_message() {
        let err = Error::from_message("Single error");
        assert_eq!(err.message(), Some("Single error"));

        let full_msg = err.full_message();
        assert!(full_msg.contains("Single error"));
    }

    #[test]
    fn test_error_display() {
        let err = Error::from_message("Display test error");
        let display_str = format!("{}", err);
        assert!(display_str.contains("Display test error"));
    }

    #[test]
    fn test_error_from_raw_null() {
        Error::from_raw(std::ptr::null_mut()).unwrap();
    }

    #[test]
    fn test_error_category() {
        use subversion_sys::*;

        // Test various error category codes - create errors directly via C API
        let io_err_ptr = unsafe {
            subversion_sys::svn_error_create(
                SVN_ERR_IO_CATEGORY_START as i32,
                std::ptr::null_mut(),
                b"I/O error\0".as_ptr() as *const i8,
            )
        };
        let io_err = Error {
            ptr: io_err_ptr,
            owns_ptr: true,
            _phantom: std::marker::PhantomData,
        };
        assert_eq!(io_err.category(), ErrorCategory::Io);

        let auth_err_ptr = unsafe {
            subversion_sys::svn_error_create(
                SVN_ERR_AUTHN_CATEGORY_START as i32,
                std::ptr::null_mut(),
                b"Auth error\0".as_ptr() as *const i8,
            )
        };
        let auth_err = Error {
            ptr: auth_err_ptr,
            owns_ptr: true,
            _phantom: std::marker::PhantomData,
        };
        assert_eq!(auth_err.category(), ErrorCategory::Authentication);

        let authz_err_ptr = unsafe {
            subversion_sys::svn_error_create(
                SVN_ERR_AUTHZ_CATEGORY_START as i32,
                std::ptr::null_mut(),
                b"Authz error\0".as_ptr() as *const i8,
            )
        };
        let authz_err = Error {
            ptr: authz_err_ptr,
            owns_ptr: true,
            _phantom: std::marker::PhantomData,
        };
        assert_eq!(authz_err.category(), ErrorCategory::Authorization);

        let wc_err_ptr = unsafe {
            subversion_sys::svn_error_create(
                SVN_ERR_WC_CATEGORY_START as i32,
                std::ptr::null_mut(),
                b"WC error\0".as_ptr() as *const i8,
            )
        };
        let wc_err = Error {
            ptr: wc_err_ptr,
            owns_ptr: true,
            _phantom: std::marker::PhantomData,
        };
        assert_eq!(wc_err.category(), ErrorCategory::WorkingCopy);

        let repos_err_ptr = unsafe {
            subversion_sys::svn_error_create(
                SVN_ERR_REPOS_CATEGORY_START as i32,
                std::ptr::null_mut(),
                b"Repos error\0".as_ptr() as *const i8,
            )
        };
        let repos_err = Error {
            ptr: repos_err_ptr,
            owns_ptr: true,
            _phantom: std::marker::PhantomData,
        };
        assert_eq!(repos_err.category(), ErrorCategory::Repository);

        let misc_err_ptr = unsafe {
            subversion_sys::svn_error_create(
                SVN_ERR_MISC_CATEGORY_START as i32,
                std::ptr::null_mut(),
                b"Misc error\0".as_ptr() as *const i8,
            )
        };
        let misc_err = Error {
            ptr: misc_err_ptr,
            owns_ptr: true,
            _phantom: std::marker::PhantomData,
        };
        assert_eq!(misc_err.category(), ErrorCategory::Misc);
    }

    #[test]
    fn test_error_location_returns_value() {
        // Test that Error::location() returns actual location info when available
        use subversion_sys::*;

        // Create an error with location information
        // Use a static string to avoid memory management issues with CString
        static TEST_FILE: &[u8] = b"test_file.c\0";

        let err_ptr = unsafe {
            let err = svn_error_create(
                SVN_ERR_IO_CATEGORY_START as i32,
                std::ptr::null_mut(),
                b"Test error\0".as_ptr() as *const i8,
            );
            // SVN errors typically have file/line information set by internal macros
            // We simulate this by setting them directly using a static string
            (*err).file = TEST_FILE.as_ptr() as *const i8;
            (*err).line = 42;
            err
        };

        let err = Error {
            ptr: err_ptr,
            owns_ptr: true,
            _phantom: std::marker::PhantomData,
        };

        // Verify location() returns the correct file and line
        let location = err.location();
        assert!(
            location.is_some(),
            "Error with file/line should have location"
        );
        let (file, line) = location.unwrap();
        assert_eq!(file, "test_file.c", "File name should match");
        assert_eq!(line, 42, "Line number should match");
    }

    #[test]
    fn test_error_category_boundary_conditions() {
        // Test all error category ranges with boundary conditions to catch mutations
        // that modify range checks (>=, <, +, -, &&, ||)
        use subversion_sys::*;

        // Helper to create an error with a specific error code
        let make_error = |code: u32| -> Error<'static> {
            let err_ptr = unsafe {
                svn_error_create(
                    code as i32,
                    std::ptr::null_mut(),
                    b"Test\0".as_ptr() as *const i8,
                )
            };
            Error {
                ptr: err_ptr,
                owns_ptr: true,
                _phantom: std::marker::PhantomData,
            }
        };

        let category_size = SVN_ERR_CATEGORY_SIZE;

        // Test BadInput category (first category)
        assert_eq!(
            make_error(SVN_ERR_BAD_CATEGORY_START).category(),
            ErrorCategory::BadInput,
            "Start of BadInput range"
        );
        assert_eq!(
            make_error(SVN_ERR_BAD_CATEGORY_START + category_size - 1).category(),
            ErrorCategory::BadInput,
            "End of BadInput range"
        );
        assert_eq!(
            make_error(SVN_ERR_BAD_CATEGORY_START - 1).category(),
            ErrorCategory::Other,
            "Just before BadInput range"
        );

        // Test Xml category
        assert_eq!(
            make_error(SVN_ERR_XML_CATEGORY_START).category(),
            ErrorCategory::Xml,
            "Start of Xml range"
        );
        assert_eq!(
            make_error(SVN_ERR_XML_CATEGORY_START + category_size - 1).category(),
            ErrorCategory::Xml,
            "End of Xml range"
        );
        assert_eq!(
            make_error(SVN_ERR_XML_CATEGORY_START + category_size).category(),
            ErrorCategory::Io,
            "Just after Xml range should be Io"
        );

        // Test Io category
        assert_eq!(
            make_error(SVN_ERR_IO_CATEGORY_START).category(),
            ErrorCategory::Io,
            "Start of Io range"
        );
        assert_eq!(
            make_error(SVN_ERR_IO_CATEGORY_START + category_size - 1).category(),
            ErrorCategory::Io,
            "End of Io range"
        );

        // Test Stream category
        assert_eq!(
            make_error(SVN_ERR_STREAM_CATEGORY_START).category(),
            ErrorCategory::Stream,
            "Start of Stream range"
        );
        assert_eq!(
            make_error(SVN_ERR_STREAM_CATEGORY_START + category_size - 1).category(),
            ErrorCategory::Stream,
            "End of Stream range"
        );

        // Test Node category
        assert_eq!(
            make_error(SVN_ERR_NODE_CATEGORY_START).category(),
            ErrorCategory::Node,
            "Start of Node range"
        );
        assert_eq!(
            make_error(SVN_ERR_NODE_CATEGORY_START + category_size - 1).category(),
            ErrorCategory::Node,
            "End of Node range"
        );

        // Test Entry category
        assert_eq!(
            make_error(SVN_ERR_ENTRY_CATEGORY_START).category(),
            ErrorCategory::Entry,
            "Start of Entry range"
        );
        assert_eq!(
            make_error(SVN_ERR_ENTRY_CATEGORY_START + category_size - 1).category(),
            ErrorCategory::Entry,
            "End of Entry range"
        );

        // Test WorkingCopy category
        assert_eq!(
            make_error(SVN_ERR_WC_CATEGORY_START).category(),
            ErrorCategory::WorkingCopy,
            "Start of WorkingCopy range"
        );
        assert_eq!(
            make_error(SVN_ERR_WC_CATEGORY_START + category_size - 1).category(),
            ErrorCategory::WorkingCopy,
            "End of WorkingCopy range"
        );

        // Test Filesystem category
        assert_eq!(
            make_error(SVN_ERR_FS_CATEGORY_START).category(),
            ErrorCategory::Filesystem,
            "Start of Filesystem range"
        );
        assert_eq!(
            make_error(SVN_ERR_FS_CATEGORY_START + category_size - 1).category(),
            ErrorCategory::Filesystem,
            "End of Filesystem range"
        );

        // Test Repository category
        assert_eq!(
            make_error(SVN_ERR_REPOS_CATEGORY_START).category(),
            ErrorCategory::Repository,
            "Start of Repository range"
        );
        assert_eq!(
            make_error(SVN_ERR_REPOS_CATEGORY_START + category_size - 1).category(),
            ErrorCategory::Repository,
            "End of Repository range"
        );

        // Test RepositoryAccess category
        assert_eq!(
            make_error(SVN_ERR_RA_CATEGORY_START).category(),
            ErrorCategory::RepositoryAccess,
            "Start of RepositoryAccess range"
        );
        assert_eq!(
            make_error(SVN_ERR_RA_CATEGORY_START + category_size - 1).category(),
            ErrorCategory::RepositoryAccess,
            "End of RepositoryAccess range"
        );

        // Test RaDav category
        assert_eq!(
            make_error(SVN_ERR_RA_DAV_CATEGORY_START).category(),
            ErrorCategory::RaDav,
            "Start of RaDav range"
        );
        assert_eq!(
            make_error(SVN_ERR_RA_DAV_CATEGORY_START + category_size - 1).category(),
            ErrorCategory::RaDav,
            "End of RaDav range"
        );

        // Test RaLocal category
        assert_eq!(
            make_error(SVN_ERR_RA_LOCAL_CATEGORY_START).category(),
            ErrorCategory::RaLocal,
            "Start of RaLocal range"
        );
        assert_eq!(
            make_error(SVN_ERR_RA_LOCAL_CATEGORY_START + category_size - 1).category(),
            ErrorCategory::RaLocal,
            "End of RaLocal range"
        );

        // Test Svndiff category
        assert_eq!(
            make_error(SVN_ERR_SVNDIFF_CATEGORY_START).category(),
            ErrorCategory::Svndiff,
            "Start of Svndiff range"
        );
        assert_eq!(
            make_error(SVN_ERR_SVNDIFF_CATEGORY_START + category_size - 1).category(),
            ErrorCategory::Svndiff,
            "End of Svndiff range"
        );

        // Test ApacheMod category
        assert_eq!(
            make_error(SVN_ERR_APMOD_CATEGORY_START).category(),
            ErrorCategory::ApacheMod,
            "Start of ApacheMod range"
        );
        assert_eq!(
            make_error(SVN_ERR_APMOD_CATEGORY_START + category_size - 1).category(),
            ErrorCategory::ApacheMod,
            "End of ApacheMod range"
        );

        // Test Client category
        assert_eq!(
            make_error(SVN_ERR_CLIENT_CATEGORY_START).category(),
            ErrorCategory::Client,
            "Start of Client range"
        );
        assert_eq!(
            make_error(SVN_ERR_CLIENT_CATEGORY_START + category_size - 1).category(),
            ErrorCategory::Client,
            "End of Client range"
        );

        // Test Misc category
        assert_eq!(
            make_error(SVN_ERR_MISC_CATEGORY_START).category(),
            ErrorCategory::Misc,
            "Start of Misc range"
        );
        assert_eq!(
            make_error(SVN_ERR_MISC_CATEGORY_START + category_size - 1).category(),
            ErrorCategory::Misc,
            "End of Misc range"
        );

        // Test CommandLine category
        assert_eq!(
            make_error(SVN_ERR_CL_CATEGORY_START).category(),
            ErrorCategory::CommandLine,
            "Start of CommandLine range"
        );
        assert_eq!(
            make_error(SVN_ERR_CL_CATEGORY_START + category_size - 1).category(),
            ErrorCategory::CommandLine,
            "End of CommandLine range"
        );

        // Test RaSvn category
        assert_eq!(
            make_error(SVN_ERR_RA_SVN_CATEGORY_START).category(),
            ErrorCategory::RaSvn,
            "Start of RaSvn range"
        );
        assert_eq!(
            make_error(SVN_ERR_RA_SVN_CATEGORY_START + category_size - 1).category(),
            ErrorCategory::RaSvn,
            "End of RaSvn range"
        );

        // Test Authentication category
        assert_eq!(
            make_error(SVN_ERR_AUTHN_CATEGORY_START).category(),
            ErrorCategory::Authentication,
            "Start of Authentication range"
        );
        assert_eq!(
            make_error(SVN_ERR_AUTHN_CATEGORY_START + category_size - 1).category(),
            ErrorCategory::Authentication,
            "End of Authentication range"
        );

        // Test Authorization category
        assert_eq!(
            make_error(SVN_ERR_AUTHZ_CATEGORY_START).category(),
            ErrorCategory::Authorization,
            "Start of Authorization range"
        );
        assert_eq!(
            make_error(SVN_ERR_AUTHZ_CATEGORY_START + category_size - 1).category(),
            ErrorCategory::Authorization,
            "End of Authorization range"
        );

        // Test Diff category
        assert_eq!(
            make_error(SVN_ERR_DIFF_CATEGORY_START).category(),
            ErrorCategory::Diff,
            "Start of Diff range"
        );
        assert_eq!(
            make_error(SVN_ERR_DIFF_CATEGORY_START + category_size - 1).category(),
            ErrorCategory::Diff,
            "End of Diff range"
        );

        // Test RaSerf category
        assert_eq!(
            make_error(SVN_ERR_RA_SERF_CATEGORY_START).category(),
            ErrorCategory::RaSerf,
            "Start of RaSerf range"
        );
        assert_eq!(
            make_error(SVN_ERR_RA_SERF_CATEGORY_START + category_size - 1).category(),
            ErrorCategory::RaSerf,
            "End of RaSerf range"
        );

        // Test Malfunction category
        assert_eq!(
            make_error(SVN_ERR_MALFUNC_CATEGORY_START).category(),
            ErrorCategory::Malfunction,
            "Start of Malfunction range"
        );
        assert_eq!(
            make_error(SVN_ERR_MALFUNC_CATEGORY_START + category_size - 1).category(),
            ErrorCategory::Malfunction,
            "End of Malfunction range"
        );

        // Test X509 category (last category)
        assert_eq!(
            make_error(SVN_ERR_X509_CATEGORY_START).category(),
            ErrorCategory::X509,
            "Start of X509 range"
        );
        assert_eq!(
            make_error(SVN_ERR_X509_CATEGORY_START + category_size - 1).category(),
            ErrorCategory::X509,
            "End of X509 range"
        );
        assert_eq!(
            make_error(SVN_ERR_X509_CATEGORY_START + category_size).category(),
            ErrorCategory::Other,
            "Just after X509 range"
        );

        // Test Other category (out of all ranges)
        assert_eq!(
            make_error(0).category(),
            ErrorCategory::Other,
            "Zero should be Other"
        );
        assert_eq!(
            make_error(1000).category(),
            ErrorCategory::Other,
            "Small values should be Other"
        );
        assert_eq!(
            make_error(300000).category(),
            ErrorCategory::Other,
            "Values beyond all categories should be Other"
        );
    }

    #[test]
    fn test_error_best_message_returns_actual_message() {
        // Test that best_message() returns the actual error message, not "xyzzy" or empty string
        use subversion_sys::*;

        let err_ptr = unsafe {
            svn_error_create(
                SVN_ERR_IO_CATEGORY_START as i32,
                std::ptr::null_mut(),
                b"Specific error message\0".as_ptr() as *const i8,
            )
        };
        let err = Error {
            ptr: err_ptr,
            owns_ptr: true,
            _phantom: std::marker::PhantomData,
        };

        let msg = err.best_message();
        assert!(!msg.is_empty(), "best_message should not be empty");
        assert_ne!(msg, "xyzzy", "best_message should not be 'xyzzy'");
        assert_eq!(
            msg, "Specific error message",
            "best_message should return exact message, got '{}'",
            msg
        );
    }

    #[test]
    fn test_error_child_returns_none_when_no_child() {
        // Test that child() returns None when there is no child error
        use subversion_sys::*;

        let err_ptr = unsafe {
            svn_error_create(
                SVN_ERR_IO_CATEGORY_START as i32,
                std::ptr::null_mut(),
                b"Error without child\0".as_ptr() as *const i8,
            )
        };

        let err = Error {
            ptr: err_ptr,
            owns_ptr: true,
            _phantom: std::marker::PhantomData,
        };

        // Test that child() returns None when there is no child
        let child = err.child();
        assert!(
            child.is_none(),
            "child() should return None when no child exists"
        );
    }

    #[test]
    fn test_error_find_cause_returns_none_for_non_matching_status() {
        // Test that find_cause() returns None when no error matches the requested status
        use subversion_sys::*;

        // Create an error with a specific status
        let err_ptr = unsafe {
            svn_error_create(
                SVN_ERR_IO_CATEGORY_START as i32,
                std::ptr::null_mut(),
                b"Test error\0".as_ptr() as *const i8,
            )
        };
        let err = Error {
            ptr: err_ptr,
            owns_ptr: true,
            _phantom: std::marker::PhantomData,
        };

        // Create another error with a different status to use for searching
        let different_err_ptr = unsafe {
            svn_error_create(
                (SVN_ERR_CLIENT_CATEGORY_START + 100) as i32,
                std::ptr::null_mut(),
                b"Different error\0".as_ptr() as *const i8,
            )
        };
        let different_err = Error {
            ptr: different_err_ptr,
            owns_ptr: true,
            _phantom: std::marker::PhantomData,
        };
        let different_status = different_err.apr_err();
        // Detach so it doesn't get cleaned up before we use the status
        std::mem::forget(different_err);

        // Search for a status that won't be found in the first error
        let found = err.find_cause(different_status);

        assert!(
            found.is_none(),
            "find_cause() should return None when status doesn't match any error in chain"
        );

        // Clean up the different_err
        unsafe {
            subversion_sys::svn_error_clear(different_err_ptr);
        }
    }

    #[test]
    fn test_error_child_returns_actual_child() {
        // Test that child() returns the actual child error when present, not None
        use subversion_sys::*;

        let child_err_ptr = unsafe {
            svn_error_create(
                SVN_ERR_IO_CATEGORY_START as i32,
                std::ptr::null_mut(),
                b"Child error\0".as_ptr() as *const i8,
            )
        };

        let parent_err_ptr = unsafe {
            svn_error_create(
                SVN_ERR_CLIENT_CATEGORY_START as i32,
                child_err_ptr,
                b"Parent error\0".as_ptr() as *const i8,
            )
        };

        let parent_err = Error {
            ptr: parent_err_ptr,
            owns_ptr: true,
            _phantom: std::marker::PhantomData,
        };

        // Test that child() returns Some when there is a child
        let child = parent_err.child();
        assert!(
            child.is_some(),
            "child() should return Some when child exists"
        );

        let child_err = child.unwrap();
        assert_eq!(
            child_err.category(),
            ErrorCategory::Io,
            "Child error should have Io category"
        );
        assert!(
            child_err.message().unwrap().contains("Child error"),
            "Child error should have correct message"
        );
    }

    #[test]
    fn test_error_find_cause_returns_matching_error() {
        // Test that find_cause() returns the error with matching status, not None
        // We create an error chain and verify find_cause can locate specific errors
        let child_err = Error::from_message("Child error");
        let parent_status = apr::Status::from(12345);
        let parent_err = Error::new(parent_status, Some(child_err), "Parent error");

        // find_cause should find itself when searching for its own status
        let found = parent_err.find_cause(parent_status);
        assert!(
            found.is_some(),
            "find_cause() should find error with matching status"
        );

        let found_err = found.unwrap();
        assert_eq!(
            found_err.apr_err(),
            parent_status,
            "Found error should have correct status"
        );
    }

    #[test]
    fn test_error_as_ptr_returns_actual_pointer() {
        // Test that as_ptr() returns the actual pointer, not Default::default() (null)
        use subversion_sys::*;

        let err_ptr = unsafe {
            svn_error_create(
                SVN_ERR_IO_CATEGORY_START as i32,
                std::ptr::null_mut(),
                b"Test\0".as_ptr() as *const i8,
            )
        };

        let err = Error {
            ptr: err_ptr,
            owns_ptr: true,
            _phantom: std::marker::PhantomData,
        };

        let ptr = err.as_ptr();
        assert!(!ptr.is_null(), "as_ptr() should return non-null pointer");
        assert_eq!(
            ptr, err_ptr,
            "as_ptr() should return the actual error pointer"
        );
    }

    #[test]
    fn test_error_as_mut_ptr_returns_actual_pointer() {
        // Test that as_mut_ptr() returns the actual pointer, not Default::default() (null)
        use subversion_sys::*;

        let err_ptr = unsafe {
            svn_error_create(
                SVN_ERR_IO_CATEGORY_START as i32,
                std::ptr::null_mut(),
                b"Test\0".as_ptr() as *const i8,
            )
        };

        let mut err = Error {
            ptr: err_ptr,
            owns_ptr: true,
            _phantom: std::marker::PhantomData,
        };

        let ptr = err.as_mut_ptr();
        assert!(
            !ptr.is_null(),
            "as_mut_ptr() should return non-null pointer"
        );
        assert_eq!(
            ptr, err_ptr,
            "as_mut_ptr() should return the actual error pointer"
        );
    }

    #[test]
    fn test_symbolic_name_returns_actual_names() {
        // Test that symbolic_name() returns actual error names for errors we create,
        // not "xyzzy", "", or always None

        // Create an actual error and get its status code
        let err = Error::from_message("Test error");
        let status = err.apr_err();

        // Get the symbolic name for this error
        let name = symbolic_name(status);

        // For an actual error we created, symbolic_name should return a valid name or None
        // We can't assert Some because not all error codes have symbolic names,
        // but if it returns Some, it must be valid
        if let Some(name_str) = name {
            assert!(
                !name_str.is_empty(),
                "Symbolic name should not be empty if returned"
            );
            assert_ne!(name_str, "xyzzy", "Symbolic name should not be 'xyzzy'");
            assert!(
                name_str.starts_with("SVN_"),
                "Symbolic name should start with SVN_, got: {}",
                name_str
            );
        }

        // Test that the function doesn't panic with various inputs
        let _ = symbolic_name(0.into());
        let _ = symbolic_name(999999.into());
    }

    #[test]
    fn test_strerror_returns_actual_error_strings() {
        // Test that strerror() returns actual error strings for errors we create,
        // not "xyzzy", "", or always None

        // Create an actual error and get its status code
        let err = Error::from_message("Test error");
        let status = err.apr_err();

        // Get the error string for this error
        let err_str = strerror(status);

        // strerror MUST return Some for our created error, not None
        // This catches the mutation that always returns None
        assert!(
            err_str.is_some(),
            "strerror() must return Some for a valid SVN error code, got None"
        );

        let err_msg = err_str.unwrap();
        assert!(
            !err_msg.is_empty(),
            "Error string should not be empty if returned"
        );
        assert_ne!(err_msg, "xyzzy", "Error string should not be 'xyzzy'");
        assert!(
            err_msg.len() > 2,
            "Error string should be substantive, got: {}",
            err_msg
        );

        // Test that the function doesn't panic with various inputs
        // (these may or may not return Some, so we just check they don't panic)
        let _ = strerror(0.into());
        let _ = strerror(999999.into());
    }

    #[test]
    fn test_error_category_off_by_one_and_midrange() {
        // Test off-by-one boundary conditions and mid-range values to catch operator mutations
        // This catches mutations like >= -> <, < -> ==, + -> -, && -> ||
        use subversion_sys::*;

        let make_error = |code: u32| -> Error<'static> {
            let err_ptr = unsafe {
                svn_error_create(
                    code as i32,
                    std::ptr::null_mut(),
                    b"Test\0".as_ptr() as *const i8,
                )
            };
            Error {
                ptr: err_ptr,
                owns_ptr: true,
                _phantom: std::marker::PhantomData,
            }
        };

        let category_size = SVN_ERR_CATEGORY_SIZE;

        // For each category, test: START-1, START, START+1, MID, END-1, END, END+1
        let categories = vec![
            (
                SVN_ERR_BAD_CATEGORY_START,
                ErrorCategory::BadInput,
                "BadInput",
            ),
            (SVN_ERR_XML_CATEGORY_START, ErrorCategory::Xml, "Xml"),
            (SVN_ERR_IO_CATEGORY_START, ErrorCategory::Io, "Io"),
            (
                SVN_ERR_STREAM_CATEGORY_START,
                ErrorCategory::Stream,
                "Stream",
            ),
            (SVN_ERR_NODE_CATEGORY_START, ErrorCategory::Node, "Node"),
            (SVN_ERR_ENTRY_CATEGORY_START, ErrorCategory::Entry, "Entry"),
            (
                SVN_ERR_WC_CATEGORY_START,
                ErrorCategory::WorkingCopy,
                "WorkingCopy",
            ),
            (
                SVN_ERR_FS_CATEGORY_START,
                ErrorCategory::Filesystem,
                "Filesystem",
            ),
            (
                SVN_ERR_REPOS_CATEGORY_START,
                ErrorCategory::Repository,
                "Repository",
            ),
            (
                SVN_ERR_RA_CATEGORY_START,
                ErrorCategory::RepositoryAccess,
                "RepositoryAccess",
            ),
            (SVN_ERR_RA_DAV_CATEGORY_START, ErrorCategory::RaDav, "RaDav"),
            (
                SVN_ERR_RA_LOCAL_CATEGORY_START,
                ErrorCategory::RaLocal,
                "RaLocal",
            ),
            (
                SVN_ERR_SVNDIFF_CATEGORY_START,
                ErrorCategory::Svndiff,
                "Svndiff",
            ),
            (
                SVN_ERR_APMOD_CATEGORY_START,
                ErrorCategory::ApacheMod,
                "ApacheMod",
            ),
            (
                SVN_ERR_CLIENT_CATEGORY_START,
                ErrorCategory::Client,
                "Client",
            ),
            (SVN_ERR_MISC_CATEGORY_START, ErrorCategory::Misc, "Misc"),
            (
                SVN_ERR_CL_CATEGORY_START,
                ErrorCategory::CommandLine,
                "CommandLine",
            ),
            (SVN_ERR_RA_SVN_CATEGORY_START, ErrorCategory::RaSvn, "RaSvn"),
            (
                SVN_ERR_AUTHN_CATEGORY_START,
                ErrorCategory::Authentication,
                "Authentication",
            ),
            (
                SVN_ERR_AUTHZ_CATEGORY_START,
                ErrorCategory::Authorization,
                "Authorization",
            ),
            (SVN_ERR_DIFF_CATEGORY_START, ErrorCategory::Diff, "Diff"),
            (
                SVN_ERR_RA_SERF_CATEGORY_START,
                ErrorCategory::RaSerf,
                "RaSerf",
            ),
            (
                SVN_ERR_MALFUNC_CATEGORY_START,
                ErrorCategory::Malfunction,
                "Malfunction",
            ),
            (SVN_ERR_X509_CATEGORY_START, ErrorCategory::X509, "X509"),
        ];

        for (start, expected_cat, name) in categories {
            // Test START (should be in category)
            assert_eq!(
                make_error(start).category(),
                expected_cat,
                "{}: START should be in category",
                name
            );

            // Test START + 1 (should be in category, catches >= -> > mutation)
            assert_eq!(
                make_error(start + 1).category(),
                expected_cat,
                "{}: START+1 should be in category",
                name
            );

            // Test mid-range (should be in category)
            let mid = start + category_size / 2;
            assert_eq!(
                make_error(mid).category(),
                expected_cat,
                "{}: MID should be in category",
                name
            );

            // Test END - 2 (should be in category)
            assert_eq!(
                make_error(start + category_size - 2).category(),
                expected_cat,
                "{}: END-2 should be in category",
                name
            );

            // Test END - 1 (should be in category, catches < -> <= mutation)
            assert_eq!(
                make_error(start + category_size - 1).category(),
                expected_cat,
                "{}: END-1 (last valid) should be in category",
                name
            );

            // Test END (should NOT be in category, catches < -> <= mutation)
            assert_ne!(
                make_error(start + category_size).category(),
                expected_cat,
                "{}: END should NOT be in category",
                name
            );

            // Test START - 1 (should NOT be in category for most, catches >= -> > mutation)
            // Skip for first category since START-1 might underflow
            if start > 1000 {
                assert_ne!(
                    make_error(start - 1).category(),
                    expected_cat,
                    "{}: START-1 should NOT be in category",
                    name
                );
            }
        }

        // Test value way below all categories
        assert_eq!(
            make_error(100).category(),
            ErrorCategory::Other,
            "Value below all categories should be Other"
        );

        // Test value way above all categories
        assert_eq!(
            make_error(500000).category(),
            ErrorCategory::Other,
            "Value above all categories should be Other"
        );

        // Test between categories (just after BadInput, should be Xml or Other)
        let between = SVN_ERR_BAD_CATEGORY_START + category_size;
        let between_cat = make_error(between).category();
        assert_ne!(
            between_cat,
            ErrorCategory::BadInput,
            "Value just after BadInput should not be BadInput"
        );
    }

    #[test]
    fn test_raw_apr_err_preserves_svn_error_codes() {
        // SVN error codes like SVN_ERR_CANCELLED (200015) are not standard APR
        // status codes and get mapped to General by apr::Status::from().
        // raw_apr_err() must preserve the original code.
        let cancelled_code = subversion_sys::svn_errno_t_SVN_ERR_CANCELLED as i32;
        let err = Error::with_raw_status(cancelled_code, None, "cancelled");

        assert_eq!(err.raw_apr_err(), cancelled_code);
        // apr_err() loses the distinction — both map to General
        assert_eq!(err.apr_err(), apr::Status::General);
    }

    #[test]
    fn test_with_raw_status_creates_distinguishable_errors() {
        let cancelled_code = subversion_sys::svn_errno_t_SVN_ERR_CANCELLED as i32;
        let fs_not_found_code = subversion_sys::svn_errno_t_SVN_ERR_FS_NOT_FOUND as i32;

        let err1 = Error::with_raw_status(cancelled_code, None, "cancelled");
        let err2 = Error::with_raw_status(fs_not_found_code, None, "not found");

        // apr_err() would return General for both — indistinguishable
        assert_eq!(err1.apr_err(), err2.apr_err());
        // raw_apr_err() preserves the difference
        assert_ne!(err1.raw_apr_err(), err2.raw_apr_err());
        assert_eq!(err1.raw_apr_err(), cancelled_code);
        assert_eq!(err2.raw_apr_err(), fs_not_found_code);
    }

    #[test]
    fn test_with_raw_status_message_and_child() {
        let child = Error::from_message("child error");
        let parent = Error::with_raw_status(200015, Some(child), "parent error");

        assert_eq!(parent.message(), Some("parent error"));
        let full = parent.full_message();
        assert!(full.contains("parent error"));
        assert!(full.contains("child error"));
    }
}
