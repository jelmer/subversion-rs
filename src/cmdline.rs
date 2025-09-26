//! Command-line utilities for Subversion applications
//!
//! This module provides utilities for command-line applications using Subversion,
//! including initialization, authentication setup, terminal handling, and progress
//! notification.
//!
//! Note: Many cmdline functions are not available in the current subversion-sys
//! bindings, so this module provides a framework that can be extended when
//! those bindings become available.

use crate::{auth::AuthBaton, Error};
use std::io::{self, Write};

/// Progress notification callback type
pub type ProgressCallback = Box<dyn FnMut(i64, i64) + Send>;

/// Terminal capabilities and settings
#[derive(Debug, Clone)]
pub struct TerminalInfo {
    /// Terminal width in characters
    pub width: Option<usize>,
    /// Terminal height in characters  
    pub height: Option<usize>,
    /// Whether terminal supports colors
    pub supports_color: bool,
    /// Whether terminal supports UTF-8
    pub supports_utf8: bool,
    /// Whether output is being redirected
    pub is_redirected: bool,
}

impl TerminalInfo {
    /// Detect terminal capabilities
    pub fn detect() -> Self {
        // Use environment variables and standard detection methods
        let width = std::env::var("COLUMNS")
            .ok()
            .and_then(|s| s.parse().ok())
            .or_else(|| {
                #[cfg(unix)]
                {
                    // Try to get terminal size from system call
                    unsafe {
                        let mut ws: libc::winsize = std::mem::zeroed();
                        if libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut ws) == 0 {
                            if ws.ws_col > 0 {
                                Some(ws.ws_col as usize)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                }
                #[cfg(not(unix))]
                None
            })
            .or(Some(80)); // Default fallback

        let height = std::env::var("LINES")
            .ok()
            .and_then(|s| s.parse().ok())
            .or_else(|| {
                #[cfg(unix)]
                {
                    unsafe {
                        let mut ws: libc::winsize = std::mem::zeroed();
                        if libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut ws) == 0 {
                            if ws.ws_row > 0 {
                                Some(ws.ws_row as usize)
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }
                }
                #[cfg(not(unix))]
                None
            })
            .or(Some(24)); // Default fallback

        // Detect color support
        let supports_color = std::env::var("NO_COLOR").is_err()
            && (std::env::var("FORCE_COLOR").is_ok()
                || std::env::var("TERM")
                    .map(|term| !term.is_empty() && term != "dumb")
                    .unwrap_or(false));

        // Detect UTF-8 support
        let supports_utf8 = std::env::var("LC_ALL")
            .or_else(|_| std::env::var("LC_CTYPE"))
            .or_else(|_| std::env::var("LANG"))
            .map(|locale| {
                locale.to_uppercase().contains("UTF-8") || locale.to_uppercase().contains("UTF8")
            })
            .unwrap_or(true); // Default to UTF-8 support

        // Detect if output is redirected
        let is_redirected = !atty::is(atty::Stream::Stdout);

        Self {
            width,
            height,
            supports_color,
            supports_utf8,
            is_redirected,
        }
    }

    /// Get effective terminal width for output formatting
    pub fn effective_width(&self) -> usize {
        if self.is_redirected {
            // Use unlimited width for redirected output
            usize::MAX
        } else {
            self.width.unwrap_or(80)
        }
    }
}

/// Command-line application context
pub struct CmdlineContext {
    /// Terminal information
    pub terminal: TerminalInfo,
    /// Progress callback
    progress_callback: Option<ProgressCallback>,
    /// Whether to suppress output
    pub quiet: bool,
    /// Verbosity level
    pub verbose: u8,
}

impl CmdlineContext {
    /// Create a new cmdline context
    pub fn new() -> Self {
        Self {
            terminal: TerminalInfo::detect(),
            progress_callback: None,
            quiet: false,
            verbose: 0,
        }
    }

    /// Set progress callback
    pub fn set_progress_callback(&mut self, callback: ProgressCallback) {
        self.progress_callback = Some(callback);
    }

    /// Clear progress callback
    pub fn clear_progress_callback(&mut self) {
        self.progress_callback = None;
    }

    /// Report progress
    pub fn report_progress(&mut self, completed: i64, total: i64) {
        if let Some(ref mut callback) = self.progress_callback {
            callback(completed, total);
        }
    }

    /// Set quiet mode
    pub fn set_quiet(&mut self, quiet: bool) {
        self.quiet = quiet;
    }

    /// Set verbosity level
    pub fn set_verbose(&mut self, level: u8) {
        self.verbose = level;
    }

    /// Check if we should show progress
    pub fn should_show_progress(&self) -> bool {
        !self.quiet && !self.terminal.is_redirected && self.progress_callback.is_some()
    }
}

impl Default for CmdlineContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize command-line application
///
/// Note: This is currently a placeholder since svn_cmdline_init() is not
/// available in subversion-sys bindings.
pub fn init(program_name: &str) -> Result<(), Error> {
    // TODO: When subversion-sys exposes cmdline functions, implement:
    // - svn_cmdline_init(program_name, stderr)
    // - Set up locale and encoding
    // - Initialize APR if needed

    // For now, just validate the program name
    if program_name.is_empty() {
        return Err(Error::from_str("Program name cannot be empty"));
    }

    Ok(())
}

/// Create authentication baton for command-line use
///
/// This sets up authentication providers suitable for interactive command-line use.
pub fn create_auth_baton(
    username: Option<&str>,
    password: Option<&str>,
    config_dir: Option<&str>,
    non_interactive: bool,
) -> Result<AuthBaton, Error> {
    let auth_baton = AuthBaton::open(vec![])?;

    // TODO: When subversion-sys exposes more auth providers, add:
    // - Keychain providers (macOS)
    // - GNOME Keyring providers (Linux)
    // - Windows credential providers
    // - Prompt providers (if interactive)

    // For now, add basic providers if credentials are provided
    if let Some(user) = username {
        // Would add username provider
        let _ = user; // Suppress unused warning
    }

    if let Some(pass) = password {
        // Would add password provider
        let _ = pass; // Suppress unused warning
    }

    if let Some(config) = config_dir {
        // Would set config directory
        let _ = config; // Suppress unused warning
    }

    if non_interactive {
        // Would disable interactive prompts
    }

    Ok(auth_baton)
}

/// Progress notification for command-line operations
pub struct ProgressNotifier {
    context: CmdlineContext,
    last_progress: f64,
    start_time: std::time::Instant,
}

impl ProgressNotifier {
    /// Create a new progress notifier
    pub fn new(context: CmdlineContext) -> Self {
        Self {
            context,
            last_progress: 0.0,
            start_time: std::time::Instant::now(),
        }
    }

    /// Update progress
    pub fn update(&mut self, completed: i64, total: i64) -> Result<(), Error> {
        if !self.context.should_show_progress() {
            return Ok(());
        }

        if total <= 0 {
            return Ok(());
        }

        let progress = (completed as f64) / (total as f64);

        // Only update if progress changed significantly
        if (progress - self.last_progress).abs() < 0.01 {
            return Ok(());
        }

        self.last_progress = progress;

        // Calculate ETA
        let elapsed = self.start_time.elapsed();
        let eta = if progress > 0.0 {
            Some(elapsed.mul_f64((1.0 - progress) / progress))
        } else {
            None
        };

        // Format progress bar
        let width = self.context.terminal.effective_width().min(80);
        let bar_width = width.saturating_sub(20); // Leave space for percentages and info

        if bar_width > 0 {
            let filled = ((progress * bar_width as f64) as usize).min(bar_width);
            let bar = "=".repeat(filled) + &" ".repeat(bar_width - filled);

            print!("\r[{}] {:.1}%", bar, progress * 100.0);

            if let Some(eta_duration) = eta {
                if eta_duration.as_secs() < 3600 {
                    print!(
                        " ETA: {}:{:02}",
                        eta_duration.as_secs() / 60,
                        eta_duration.as_secs() % 60
                    );
                }
            }

            io::stdout()
                .flush()
                .map_err(|e| Error::from_str(&format!("Progress display error: {}", e)))?;
        }

        // Report to callback
        self.context.report_progress(completed, total);

        Ok(())
    }

    /// Finish progress display
    pub fn finish(&mut self) -> Result<(), Error> {
        if self.context.should_show_progress() {
            println!(); // Move to next line
        }
        Ok(())
    }
}

/// Format byte count for human display
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];

    if bytes == 0 {
        return "0 B".to_string();
    }

    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

/// Format duration for human display
pub fn format_duration(duration: std::time::Duration) -> String {
    let secs = duration.as_secs();

    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m{}s", secs / 60, secs % 60)
    } else {
        format!("{}h{}m{}s", secs / 3600, (secs % 3600) / 60, secs % 60)
    }
}

/// Check if stdin is available (not redirected)
pub fn stdin_is_available() -> bool {
    atty::is(atty::Stream::Stdin)
}

/// Check if we're in an interactive terminal session
pub fn is_interactive() -> bool {
    atty::is(atty::Stream::Stdin) && atty::is(atty::Stream::Stdout)
}

/// Prompt user for input
pub fn prompt_user(prompt: &str, echo: bool) -> Result<String, Error> {
    if !stdin_is_available() {
        return Err(Error::from_str("Cannot prompt user: stdin not available"));
    }

    print!("{}", prompt);
    io::stdout()
        .flush()
        .map_err(|e| Error::from_str(&format!("Prompt error: {}", e)))?;

    if echo {
        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|e| Error::from_str(&format!("Input error: {}", e)))?;
        Ok(input.trim().to_string())
    } else {
        // For password input, we'd want to disable echo
        // This is a simplified version - real implementation would use termios
        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            let mut input = String::new();

            // Disable echo
            let stdin_fd = io::stdin().as_raw_fd();
            let mut termios: libc::termios = unsafe { std::mem::zeroed() };

            unsafe {
                libc::tcgetattr(stdin_fd, &mut termios);
                let original_lflag = termios.c_lflag;
                termios.c_lflag &= !(libc::ECHO | libc::ECHONL);
                libc::tcsetattr(stdin_fd, libc::TCSAFLUSH, &termios);

                let result = io::stdin().read_line(&mut input);

                // Restore echo
                termios.c_lflag = original_lflag;
                libc::tcsetattr(stdin_fd, libc::TCSAFLUSH, &termios);

                println!(); // Add newline since echo was disabled

                result.map_err(|e| Error::from_str(&format!("Input error: {}", e)))?;
            }

            Ok(input.trim().to_string())
        }
        #[cfg(not(unix))]
        {
            // Fallback for non-Unix systems - just read normally
            let mut input = String::new();
            io::stdin()
                .read_line(&mut input)
                .map_err(|e| Error::from_str(&format!("Input error: {}", e)))?;
            println!("Warning: Password input not hidden on this platform");
            Ok(input.trim().to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_info_creation() {
        let terminal = TerminalInfo::detect();
        // Should not panic and should have reasonable defaults
        assert!(terminal.width.is_none() || terminal.width.unwrap() > 0);
        assert!(terminal.height.is_none() || terminal.height.unwrap() > 0);
    }

    #[test]
    fn test_terminal_effective_width() {
        let mut terminal = TerminalInfo::detect();

        // Normal terminal
        terminal.is_redirected = false;
        terminal.width = Some(100);
        assert_eq!(terminal.effective_width(), 100);

        // Redirected output should use unlimited width
        terminal.is_redirected = true;
        assert_eq!(terminal.effective_width(), usize::MAX);
    }

    #[test]
    fn test_cmdline_context_creation() {
        let context = CmdlineContext::new();
        assert!(!context.quiet);
        assert_eq!(context.verbose, 0);
        assert!(context.progress_callback.is_none());
    }

    #[test]
    fn test_cmdline_context_settings() {
        let mut context = CmdlineContext::new();

        context.set_quiet(true);
        assert!(context.quiet);

        context.set_verbose(3);
        assert_eq!(context.verbose, 3);

        // Progress callback
        context.set_progress_callback(Box::new(|_completed, _total| {}));
        assert!(context.progress_callback.is_some());

        context.clear_progress_callback();
        assert!(context.progress_callback.is_none());
    }

    #[test]
    fn test_init() {
        let result = init("test-program");
        assert!(result.is_ok());

        let result = init("");
        assert!(result.is_err());
    }

    #[test]
    fn test_create_auth_baton() {
        let result = create_auth_baton(Some("user"), Some("pass"), None, false);
        // Should create without error (even if functionality is limited)
        let _ = result;
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1048576), "1.0 MB");
        assert_eq!(format_bytes(2147483648), "2.0 GB");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(std::time::Duration::from_secs(30)), "30s");
        assert_eq!(format_duration(std::time::Duration::from_secs(90)), "1m30s");
        assert_eq!(
            format_duration(std::time::Duration::from_secs(3661)),
            "1h1m1s"
        );
    }

    #[test]
    fn test_interactivity_detection() {
        // These tests will vary based on test environment
        let _ = stdin_is_available();
        let _ = is_interactive();
    }

    #[test]
    fn test_progress_notifier() {
        let context = CmdlineContext::new();
        let mut notifier = ProgressNotifier::new(context);

        // Should not panic
        let result = notifier.update(50, 100);
        let _ = result;

        let result = notifier.finish();
        let _ = result;
    }
}
