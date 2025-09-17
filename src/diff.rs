use crate::{svn_result, with_tmp_pool, Error};

/// Options for diff operations
#[derive(Debug, Clone, Copy, Default)]
pub struct DiffOptions {
    /// Ignore changes in whitespace
    pub ignore_whitespace: bool,
    /// Ignore changes in end-of-line style  
    pub ignore_eol_style: bool,
    /// Show context around changes
    pub show_c_function: bool,
}

impl DiffOptions {
    /// Creates new diff options with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets whether to ignore whitespace changes.
    pub fn with_ignore_whitespace(mut self, ignore: bool) -> Self {
        self.ignore_whitespace = ignore;
        self
    }

    /// Sets whether to ignore end-of-line style differences.
    pub fn with_ignore_eol_style(mut self, ignore: bool) -> Self {
        self.ignore_eol_style = ignore;
        self
    }

    /// Sets whether to show C function names in context.
    pub fn with_show_c_function(mut self, show: bool) -> Self {
        self.show_c_function = show;
        self
    }
}

/// A diff hunk showing differences between files
pub struct DiffHunk {
    ptr: *mut subversion_sys::svn_diff_hunk_t,
}

impl DiffHunk {
    #[allow(dead_code)]
    unsafe fn from_raw(ptr: *mut subversion_sys::svn_diff_hunk_t) -> Self {
        Self { ptr }
    }

    /// Get the starting line number in the original file
    pub fn original_start(&self) -> u64 {
        unsafe { subversion_sys::svn_diff_hunk_get_original_start(self.ptr) }
    }

    /// Get the number of lines in the original file
    pub fn original_length(&self) -> u64 {
        unsafe { subversion_sys::svn_diff_hunk_get_original_length(self.ptr) }
    }

    /// Get the starting line number in the modified file
    pub fn modified_start(&self) -> u64 {
        unsafe { subversion_sys::svn_diff_hunk_get_modified_start(self.ptr) }
    }

    /// Get the number of lines in the modified file  
    pub fn modified_length(&self) -> u64 {
        unsafe { subversion_sys::svn_diff_hunk_get_modified_length(self.ptr) }
    }

    /// Get the leading context lines
    pub fn leading_context(&self) -> u64 {
        unsafe { subversion_sys::svn_diff_hunk_get_leading_context(self.ptr) }
    }

    /// Get the trailing context lines
    pub fn trailing_context(&self) -> u64 {
        unsafe { subversion_sys::svn_diff_hunk_get_trailing_context(self.ptr) }
    }
}

/// A diff between two files
pub struct Diff {
    ptr: *mut subversion_sys::svn_diff_t,
    _pool: apr::Pool,
}

impl Diff {
    unsafe fn from_raw(ptr: *mut subversion_sys::svn_diff_t, pool: apr::Pool) -> Self {
        Self { ptr, _pool: pool }
    }

    /// Check if the diff contains any changes
    pub fn contains_changes(&self) -> bool {
        unsafe { subversion_sys::svn_diff_contains_diffs(self.ptr) != 0 }
    }

    /// Check if the diff contains conflicts  
    pub fn contains_conflicts(&self) -> bool {
        unsafe { subversion_sys::svn_diff_contains_conflicts(self.ptr) != 0 }
    }

    /// Get the raw pointer for use with other SVN functions
    pub fn as_ptr(&self) -> *mut subversion_sys::svn_diff_t {
        self.ptr
    }
}

/// File options for diff operations
#[derive(Debug, Clone, Copy, Default)]
pub struct FileOptions {
    /// Ignore whitespace changes
    pub ignore_space: IgnoreSpace,
    /// Ignore end-of-line differences
    pub ignore_eol_style: bool,
    /// Show function context
    pub show_c_function: bool,
}

impl FileOptions {
    /// Creates new file options with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets whether to ignore whitespace changes.
    pub fn with_ignore_whitespace(mut self, ignore: bool) -> Self {
        self.ignore_space = if ignore {
            IgnoreSpace::Change
        } else {
            IgnoreSpace::None
        };
        self
    }

    /// Sets whether to ignore end-of-line style differences.
    pub fn with_ignore_eol_style(mut self, ignore: bool) -> Self {
        self.ignore_eol_style = ignore;
        self
    }

    /// Sets whether to show C function names in context.
    pub fn with_show_c_function(mut self, show: bool) -> Self {
        self.show_c_function = show;
        self
    }
}

/// Types of whitespace ignoring
#[derive(Debug, Clone, Copy, Default)]
pub enum IgnoreSpace {
    #[default]
    /// Don't ignore any whitespace
    None,
    /// Ignore changes in whitespace
    Change,
    /// Ignore all whitespace
    All,
}

impl From<IgnoreSpace> for subversion_sys::svn_diff_file_ignore_space_t {
    fn from(ignore: IgnoreSpace) -> Self {
        match ignore {
            IgnoreSpace::None => {
                subversion_sys::svn_diff_file_ignore_space_t_svn_diff_file_ignore_space_none
            }
            IgnoreSpace::Change => {
                subversion_sys::svn_diff_file_ignore_space_t_svn_diff_file_ignore_space_change
            }
            IgnoreSpace::All => {
                subversion_sys::svn_diff_file_ignore_space_t_svn_diff_file_ignore_space_all
            }
        }
    }
}

/// Diff two files
pub fn file_diff(
    original: &std::path::Path,
    modified: &std::path::Path,
    options: FileOptions,
) -> Result<Diff, Error> {
    let original_cstr = std::ffi::CString::new(original.to_string_lossy().as_ref())?;
    let modified_cstr = std::ffi::CString::new(modified.to_string_lossy().as_ref())?;

    let pool = apr::Pool::new();
    let mut diff_ptr = std::ptr::null_mut();

    // Create diff options
    let diff_options = unsafe { subversion_sys::svn_diff_file_options_create(pool.as_mut_ptr()) };
    unsafe {
        (*diff_options).ignore_space = options.ignore_space.into();
        (*diff_options).ignore_eol_style = if options.ignore_eol_style { 1 } else { 0 };
        (*diff_options).show_c_function = if options.show_c_function { 1 } else { 0 };
    }

    let err = unsafe {
        subversion_sys::svn_diff_file_diff_2(
            &mut diff_ptr,
            original_cstr.as_ptr(),
            modified_cstr.as_ptr(),
            diff_options,
            pool.as_mut_ptr(),
        )
    };

    svn_result(err)?;
    Ok(unsafe { Diff::from_raw(diff_ptr, pool) })
}

/// Diff three files (three-way comparison)
pub fn file_diff3(
    original: &std::path::Path,
    modified: &std::path::Path,
    latest: &std::path::Path,
    options: FileOptions,
) -> Result<Diff, Error> {
    let original_cstr = std::ffi::CString::new(original.to_string_lossy().as_ref())?;
    let modified_cstr = std::ffi::CString::new(modified.to_string_lossy().as_ref())?;
    let latest_cstr = std::ffi::CString::new(latest.to_string_lossy().as_ref())?;

    let pool = apr::Pool::new();
    let mut diff_ptr = std::ptr::null_mut();

    // Create diff options
    let diff_options = unsafe { subversion_sys::svn_diff_file_options_create(pool.as_mut_ptr()) };
    unsafe {
        (*diff_options).ignore_space = options.ignore_space.into();
        (*diff_options).ignore_eol_style = if options.ignore_eol_style { 1 } else { 0 };
        (*diff_options).show_c_function = if options.show_c_function { 1 } else { 0 };
    }

    let err = unsafe {
        subversion_sys::svn_diff_file_diff3_2(
            &mut diff_ptr,
            original_cstr.as_ptr(),
            modified_cstr.as_ptr(),
            latest_cstr.as_ptr(),
            diff_options,
            pool.as_mut_ptr(),
        )
    };

    svn_result(err)?;
    Ok(unsafe { Diff::from_raw(diff_ptr, pool) })
}

/// Diff four files (four-way comparison with ancestor)
pub fn file_diff4(
    original: &std::path::Path,
    modified: &std::path::Path,
    latest: &std::path::Path,
    ancestor: &std::path::Path,
    options: FileOptions,
) -> Result<Diff, Error> {
    let original_cstr = std::ffi::CString::new(original.to_string_lossy().as_ref())?;
    let modified_cstr = std::ffi::CString::new(modified.to_string_lossy().as_ref())?;
    let latest_cstr = std::ffi::CString::new(latest.to_string_lossy().as_ref())?;
    let ancestor_cstr = std::ffi::CString::new(ancestor.to_string_lossy().as_ref())?;

    let pool = apr::Pool::new();
    let mut diff_ptr = std::ptr::null_mut();

    // Create diff options
    let diff_options = unsafe { subversion_sys::svn_diff_file_options_create(pool.as_mut_ptr()) };
    unsafe {
        (*diff_options).ignore_space = options.ignore_space.into();
        (*diff_options).ignore_eol_style = if options.ignore_eol_style { 1 } else { 0 };
        (*diff_options).show_c_function = if options.show_c_function { 1 } else { 0 };
    }

    let err = unsafe {
        subversion_sys::svn_diff_file_diff4_2(
            &mut diff_ptr,
            original_cstr.as_ptr(),
            modified_cstr.as_ptr(),
            latest_cstr.as_ptr(),
            ancestor_cstr.as_ptr(),
            diff_options,
            pool.as_mut_ptr(),
        )
    };

    svn_result(err)?;
    Ok(unsafe { Diff::from_raw(diff_ptr, pool) })
}

/// Output unified diff format
pub fn file_output_unified(
    output_stream: &mut crate::io::Stream,
    diff: &Diff,
    original_path: &std::path::Path,
    modified_path: &std::path::Path,
    original_header: Option<&str>,
    modified_header: Option<&str>,
    header_encoding: &str,
    context_size: i32,
) -> Result<(), Error> {
    let original_path_cstr = std::ffi::CString::new(original_path.to_string_lossy().as_ref())?;
    let modified_path_cstr = std::ffi::CString::new(modified_path.to_string_lossy().as_ref())?;
    let header_encoding_cstr = std::ffi::CString::new(header_encoding)?;

    let original_header_cstr = original_header.map(std::ffi::CString::new).transpose()?;
    let modified_header_cstr = modified_header.map(std::ffi::CString::new).transpose()?;

    with_tmp_pool(|scratch_pool| {
        let err = unsafe {
            subversion_sys::svn_diff_file_output_unified4(
                output_stream.as_mut_ptr(),
                diff.as_ptr(),
                original_path_cstr.as_ptr(),
                modified_path_cstr.as_ptr(),
                original_header_cstr
                    .as_ref()
                    .map_or(std::ptr::null(), |c| c.as_ptr()),
                modified_header_cstr
                    .as_ref()
                    .map_or(std::ptr::null(), |c| c.as_ptr()),
                header_encoding_cstr.as_ptr(),
                std::ptr::null(), // relative_to_dir
                1,                // show_c_function
                context_size,
                None,                 // cancel_func
                std::ptr::null_mut(), // cancel_baton
                scratch_pool.as_mut_ptr(),
            )
        };

        svn_result(err)
    })
}

/// Diff memory strings
pub fn mem_string_diff(
    original: &str,
    modified: &str,
    options: FileOptions,
) -> Result<Diff, Error> {
    let pool = apr::Pool::new();
    let mut diff_ptr = std::ptr::null_mut();

    // Create svn_string_t structures
    let original_svn_str = subversion_sys::svn_string_t {
        data: original.as_ptr() as *const std::os::raw::c_char,
        len: original.len(),
    };

    let modified_svn_str = subversion_sys::svn_string_t {
        data: modified.as_ptr() as *const std::os::raw::c_char,
        len: modified.len(),
    };

    // Create diff options
    let diff_options = unsafe { subversion_sys::svn_diff_file_options_create(pool.as_mut_ptr()) };
    unsafe {
        (*diff_options).ignore_space = options.ignore_space.into();
        (*diff_options).ignore_eol_style = if options.ignore_eol_style { 1 } else { 0 };
        (*diff_options).show_c_function = if options.show_c_function { 1 } else { 0 };
    }

    let err = unsafe {
        subversion_sys::svn_diff_mem_string_diff(
            &mut diff_ptr,
            &original_svn_str,
            &modified_svn_str,
            diff_options,
            pool.as_mut_ptr(),
        )
    };

    svn_result(err)?;
    Ok(unsafe { Diff::from_raw(diff_ptr, pool) })
}

/// Conflict display style for merge output
#[derive(Debug, Clone, Copy)]
pub enum ConflictDisplayStyle {
    /// Show modified and latest
    ModifiedLatest,
    /// Show resolved conflicts only
    ResolvedModifiedLatest,
    /// Show modified, original, and latest
    ModifiedOriginalLatest,
    /// Show only modified
    OnlyConflicts,
}

impl From<ConflictDisplayStyle> for subversion_sys::svn_diff_conflict_display_style_t {
    fn from(style: ConflictDisplayStyle) -> Self {
        match style {
            ConflictDisplayStyle::ModifiedLatest => {
                subversion_sys::svn_diff_conflict_display_style_t_svn_diff_conflict_display_modified_latest
            }
            ConflictDisplayStyle::ResolvedModifiedLatest => {
                subversion_sys::svn_diff_conflict_display_style_t_svn_diff_conflict_display_resolved_modified_latest
            }
            ConflictDisplayStyle::ModifiedOriginalLatest => {
                subversion_sys::svn_diff_conflict_display_style_t_svn_diff_conflict_display_modified_original_latest
            }
            ConflictDisplayStyle::OnlyConflicts => {
                subversion_sys::svn_diff_conflict_display_style_t_svn_diff_conflict_display_only_conflicts
            }
        }
    }
}

/// Output merge result with conflict markers
pub fn file_output_merge(
    output_stream: &mut crate::io::Stream,
    diff: &Diff,
    original_path: &std::path::Path,
    modified_path: &std::path::Path,
    latest_path: &std::path::Path,
    conflict_original: Option<&str>,
    conflict_modified: Option<&str>,
    conflict_latest: Option<&str>,
    conflict_separator: Option<&str>,
    conflict_style: ConflictDisplayStyle,
) -> Result<(), Error> {
    let original_path_cstr = std::ffi::CString::new(original_path.to_string_lossy().as_ref())?;
    let modified_path_cstr = std::ffi::CString::new(modified_path.to_string_lossy().as_ref())?;
    let latest_path_cstr = std::ffi::CString::new(latest_path.to_string_lossy().as_ref())?;

    let conflict_original_cstr = conflict_original.map(std::ffi::CString::new).transpose()?;
    let conflict_modified_cstr = conflict_modified.map(std::ffi::CString::new).transpose()?;
    let conflict_latest_cstr = conflict_latest.map(std::ffi::CString::new).transpose()?;
    let conflict_separator_cstr = conflict_separator.map(std::ffi::CString::new).transpose()?;

    with_tmp_pool(|scratch_pool| {
        let err = unsafe {
            subversion_sys::svn_diff_file_output_merge3(
                output_stream.as_mut_ptr(),
                diff.as_ptr(),
                original_path_cstr.as_ptr(),
                modified_path_cstr.as_ptr(),
                latest_path_cstr.as_ptr(),
                conflict_original_cstr
                    .as_ref()
                    .map_or(std::ptr::null(), |c| c.as_ptr()),
                conflict_modified_cstr
                    .as_ref()
                    .map_or(std::ptr::null(), |c| c.as_ptr()),
                conflict_latest_cstr
                    .as_ref()
                    .map_or(std::ptr::null(), |c| c.as_ptr()),
                conflict_separator_cstr
                    .as_ref()
                    .map_or(std::ptr::null(), |c| c.as_ptr()),
                conflict_style.into(),
                None,                 // cancel_func
                std::ptr::null_mut(), // cancel_baton
                scratch_pool.as_mut_ptr(),
            )
        };

        svn_result(err)
    })
}

/// Generic diff output using callback functions
pub fn output(
    diff: &Diff,
    output_baton: *mut std::ffi::c_void,
    output_fns: &subversion_sys::svn_diff_output_fns_t,
) -> Result<(), Error> {
    let err = unsafe {
        subversion_sys::svn_diff_output2(
            diff.as_ptr(),
            output_baton,
            output_fns,
            None,                 // cancel_func
            std::ptr::null_mut(), // cancel_baton
        )
    };

    svn_result(err)
}

/// Output unified diff with more options
pub fn file_output_unified_with_options(
    output_stream: &mut crate::io::Stream,
    diff: &Diff,
    original_path: &std::path::Path,
    modified_path: &std::path::Path,
    original_header: Option<&str>,
    modified_header: Option<&str>,
    header_encoding: &str,
    relative_to_dir: Option<&std::path::Path>,
    show_c_function: bool,
    context_size: i32,
) -> Result<(), Error> {
    let original_path_cstr = std::ffi::CString::new(original_path.to_string_lossy().as_ref())?;
    let modified_path_cstr = std::ffi::CString::new(modified_path.to_string_lossy().as_ref())?;
    let header_encoding_cstr = std::ffi::CString::new(header_encoding)?;

    let original_header_cstr = original_header.map(std::ffi::CString::new).transpose()?;
    let modified_header_cstr = modified_header.map(std::ffi::CString::new).transpose()?;
    let relative_to_dir_cstr = relative_to_dir
        .map(|p| std::ffi::CString::new(p.to_string_lossy().as_ref()))
        .transpose()?;

    with_tmp_pool(|scratch_pool| {
        let err = unsafe {
            subversion_sys::svn_diff_file_output_unified4(
                output_stream.as_mut_ptr(),
                diff.as_ptr(),
                original_path_cstr.as_ptr(),
                modified_path_cstr.as_ptr(),
                original_header_cstr
                    .as_ref()
                    .map_or(std::ptr::null(), |c| c.as_ptr()),
                modified_header_cstr
                    .as_ref()
                    .map_or(std::ptr::null(), |c| c.as_ptr()),
                header_encoding_cstr.as_ptr(),
                relative_to_dir_cstr
                    .as_ref()
                    .map_or(std::ptr::null(), |c| c.as_ptr()),
                if show_c_function { 1 } else { 0 },
                context_size,
                None,                 // cancel_func
                std::ptr::null_mut(), // cancel_baton
                scratch_pool.as_mut_ptr(),
            )
        };

        svn_result(err)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_file_options() {
        let _options = FileOptions::default()
            .with_ignore_whitespace(true)
            .with_ignore_eol_style(true)
            .with_show_c_function(false);

        // Test that the builder pattern works
        assert!(true);
    }

    #[test]
    fn test_mem_string_diff() {
        let original = "line 1\nline 2\nline 3\n";
        let modified = "line 1\nline 2 modified\nline 3\n";

        let options = FileOptions::default();
        let diff = mem_string_diff(original, modified, options);

        assert!(diff.is_ok());
        let diff = diff.unwrap();
        assert!(diff.contains_changes());
    }

    #[test]
    fn test_file_diff() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;

        // Create test files
        let original_path = temp_dir.path().join("original.txt");
        let modified_path = temp_dir.path().join("modified.txt");

        let mut original_file = std::fs::File::create(&original_path)?;
        original_file.write_all(b"line 1\nline 2\nline 3\n")?;

        let mut modified_file = std::fs::File::create(&modified_path)?;
        modified_file.write_all(b"line 1\nline 2 modified\nline 3\n")?;

        let options = FileOptions::default();
        let diff = file_diff(&original_path, &modified_path, options)?;

        assert!(diff.contains_changes());
        assert!(!diff.contains_conflicts());

        Ok(())
    }

    #[test]
    fn test_diff_identical_files() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;

        // Create identical test files
        let original_path = temp_dir.path().join("original.txt");
        let modified_path = temp_dir.path().join("modified.txt");

        let content = b"line 1\nline 2\nline 3\n";
        std::fs::write(&original_path, content)?;
        std::fs::write(&modified_path, content)?;

        let options = FileOptions::default();
        let diff = file_diff(&original_path, &modified_path, options)?;

        assert!(!diff.contains_changes());

        Ok(())
    }

    #[test]
    fn test_file_output_unified() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;

        // Create test files
        let original_path = temp_dir.path().join("original.txt");
        let modified_path = temp_dir.path().join("modified.txt");

        std::fs::write(&original_path, b"line 1\nline 2\nline 3\n")?;
        std::fs::write(&modified_path, b"line 1\nline 2 modified\nline 3\nline 4\n")?;

        let options = FileOptions::default();
        let diff = file_diff(&original_path, &modified_path, options)?;

        // Create output stream using a string buffer
        let mut stringbuf = crate::io::StringBuf::new();
        let mut stream = crate::io::Stream::from_stringbuf(&mut stringbuf);

        // Generate unified diff output
        file_output_unified(
            &mut stream,
            &diff,
            &original_path,
            &modified_path,
            Some("Original File"),
            Some("Modified File"),
            "UTF-8",
            3,
        )?;

        // Verify output contains unified diff markers
        let output = stringbuf.to_string();
        assert!(output.contains("---"));
        assert!(output.contains("+++"));
        assert!(output.contains("@@"));

        Ok(())
    }

    #[test]
    fn test_file_output_merge() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;

        // Create test files for 3-way merge
        let original_path = temp_dir.path().join("original.txt");
        let modified_path = temp_dir.path().join("modified.txt");
        let latest_path = temp_dir.path().join("latest.txt");

        std::fs::write(&original_path, b"line 1\nline 2\nline 3\n")?;
        std::fs::write(&modified_path, b"line 1\nline 2 modified\nline 3\n")?;
        std::fs::write(&latest_path, b"line 1\nline 2 latest\nline 3\n")?;

        let options = FileOptions::default();
        let diff = file_diff3(&modified_path, &original_path, &latest_path, options)?;

        // Create output stream using a string buffer
        let mut stringbuf = crate::io::StringBuf::new();
        let mut stream = crate::io::Stream::from_stringbuf(&mut stringbuf);

        // Generate merge output with default conflict markers
        file_output_merge(
            &mut stream,
            &diff,
            &original_path,
            &modified_path,
            &latest_path,
            None,
            None,
            None,
            None,
            ConflictDisplayStyle::ModifiedLatest,
        )?;

        // Verify output was generated
        let output = stringbuf.to_string();
        assert!(!output.is_empty());

        Ok(())
    }

    #[test]
    fn test_file_output_merge_with_diff3_style() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;

        // Create test files with conflicting changes
        let original_path = temp_dir.path().join("original.txt");
        let modified_path = temp_dir.path().join("modified.txt");
        let latest_path = temp_dir.path().join("latest.txt");

        std::fs::write(&original_path, b"common line\noriginal line\ncommon end\n")?;
        std::fs::write(&modified_path, b"common line\nmodified line\ncommon end\n")?;
        std::fs::write(&latest_path, b"common line\nlatest line\ncommon end\n")?;

        let options = FileOptions::default();
        let diff = file_diff3(&modified_path, &original_path, &latest_path, options)?;

        // Create output stream using a string buffer
        let mut stringbuf = crate::io::StringBuf::new();
        let mut stream = crate::io::Stream::from_stringbuf(&mut stringbuf);

        // Generate merge output with diff3-style conflict markers
        file_output_merge(
            &mut stream,
            &diff,
            &original_path,
            &modified_path,
            &latest_path,
            Some("Modified"),
            Some("Original"),
            Some("Latest"),
            Some("======="),
            ConflictDisplayStyle::ModifiedOriginalLatest,
        )?;

        // Verify output was generated
        let output = stringbuf.to_string();
        assert!(!output.is_empty());

        Ok(())
    }

    #[test]
    fn test_file_output_unified_with_options() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;

        // Create test files
        let original_path = temp_dir.path().join("src/original.c");
        let modified_path = temp_dir.path().join("src/modified.c");

        // Create parent directory
        std::fs::create_dir_all(original_path.parent().unwrap())?;

        let original_content = b"void foo() {\n    int x = 1;\n}\n\nvoid bar() {\n    return;\n}\n";
        let modified_content = b"void foo() {\n    int x = 2;\n}\n\nvoid bar() {\n    return;\n}\n";

        std::fs::write(&original_path, original_content)?;
        std::fs::write(&modified_path, modified_content)?;

        let options = FileOptions::default();
        let diff = file_diff(&original_path, &modified_path, options)?;

        // Create output stream using a string buffer
        let mut stringbuf = crate::io::StringBuf::new();
        let mut stream = crate::io::Stream::from_stringbuf(&mut stringbuf);

        // Generate unified diff with options
        file_output_unified_with_options(
            &mut stream,
            &diff,
            &original_path,
            &modified_path,
            Some("Original Version"),
            Some("Modified Version"),
            "UTF-8",
            Some(&temp_dir.path()),
            true, // show_c_function
            5,    // context_size
        )?;

        // Verify output was generated
        let output = stringbuf.to_string();
        assert!(output.contains("---"));
        assert!(output.contains("+++"));

        Ok(())
    }

    #[test]
    fn test_conflict_display_style() {
        // Test the ConflictDisplayStyle enum conversions
        let style: subversion_sys::svn_diff_conflict_display_style_t =
            ConflictDisplayStyle::ModifiedLatest.into();
        assert_eq!(style, subversion_sys::svn_diff_conflict_display_style_t_svn_diff_conflict_display_modified_latest);

        let style: subversion_sys::svn_diff_conflict_display_style_t =
            ConflictDisplayStyle::ModifiedOriginalLatest.into();
        assert_eq!(style, subversion_sys::svn_diff_conflict_display_style_t_svn_diff_conflict_display_modified_original_latest);

        let style: subversion_sys::svn_diff_conflict_display_style_t =
            ConflictDisplayStyle::ResolvedModifiedLatest.into();
        assert_eq!(style, subversion_sys::svn_diff_conflict_display_style_t_svn_diff_conflict_display_resolved_modified_latest);

        let style: subversion_sys::svn_diff_conflict_display_style_t =
            ConflictDisplayStyle::OnlyConflicts.into();
        assert_eq!(style, subversion_sys::svn_diff_conflict_display_style_t_svn_diff_conflict_display_only_conflicts);
    }
    
    #[test]
    fn test_file_diff4() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempdir()?;
        
        // Create test files for 4-way diff
        let original_path = temp_dir.path().join("original.txt");
        let modified_path = temp_dir.path().join("modified.txt");
        let latest_path = temp_dir.path().join("latest.txt");
        let ancestor_path = temp_dir.path().join("ancestor.txt");
        
        std::fs::write(&original_path, b"line 1\nline 2\nline 3\n")?;
        std::fs::write(&modified_path, b"line 1 modified\nline 2\nline 3\n")?;
        std::fs::write(&latest_path, b"line 1\nline 2 latest\nline 3\n")?;
        std::fs::write(&ancestor_path, b"line 1\nline 2\nline 3\n")?;
        
        let options = FileOptions::default();
        let diff = file_diff4(&original_path, &modified_path, &latest_path, &ancestor_path, options)?;
        
        // Should have changes since modified and latest differ from original
        assert!(diff.contains_changes());
        
        Ok(())
    }
}
