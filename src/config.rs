use crate::{svn_result, Error};
use std::ffi::{c_void, CString};
use std::path::Path;
use std::ptr;

/// Configuration options that can be set/retrieved
#[derive(Debug, Clone)]
pub enum ConfigOption<'a> {
    // Auth section
    /// Whether to store passwords.
    StorePasswords(bool),
    /// Whether to store plaintext passwords.
    StorePlaintextPasswords(bool),
    /// Whether to store authentication credentials.
    StoreAuthCreds(bool),
    /// Whether to store SSL client certificate passphrases.
    StoreSslClientCertPP(bool),
    /// Whether to store SSL client certificate passphrases in plaintext.
    StoreSslClientCertPPPlaintext(bool),
    /// Password store types to use.
    PasswordStores(&'a str),
    /// KWallet wallet name.
    KwalletWallet(&'a str),
    /// KWallet SVN application name.
    KwalletSvnApplicationName(&'a str),
    /// Whether to prompt for SSL client certificate file.
    SslClientCertFilePrompt(bool),

    // Helpers section
    /// Editor command.
    EditorCmd(&'a str),
    /// Diff command.
    DiffCmd(&'a str),
    /// Three-way diff command.
    Diff3Cmd(&'a str),
    /// Diff file extensions.
    DiffExtensions(&'a str),
    /// Merge tool command.
    MergeToolCmd(&'a str),

    // Miscellany section
    /// Global ignore patterns.
    GlobalIgnores(&'a str),
    /// Log message encoding.
    LogEncoding(&'a str),
    /// Whether to use commit times for timestamps.
    UseCommitTimes(bool),
    /// Whether to avoid unlocking files.
    NoUnlock(bool),
    /// MIME types file path.
    MimeTypesFile(&'a str),
    /// Preserved conflict file extension.
    PreservedConflictFileExt(&'a str),
    /// Whether to enable automatic properties.
    EnableAutoProps(bool),
    /// Whether to handle conflicts interactively.
    InteractiveConflicts(bool),
    /// Memory cache size.
    MemoryCacheSize(i64),
    /// Whether to ignore content type in diffs.
    DiffIgnoreContentType(bool),

    // Working copy section
    /// Whether to use exclusive locking.
    ExclusiveLocking(bool),
    /// Exclusive locking clients.
    ExclusiveLockingClients(&'a str),
    /// Busy timeout value.
    BusyTimeout(i64),

    // Proxy section (from servers file)
    /// HTTP proxy host.
    HttpProxy(&'a str),
    /// HTTP proxy port.
    HttpProxyPort(i64),
    /// HTTP proxy username.
    HttpProxyUsername(&'a str),
    /// HTTP proxy password.
    HttpProxyPassword(&'a str),
    /// HTTP proxy exceptions.
    HttpProxyExceptions(&'a str),
    /// HTTP timeout value.
    HttpTimeout(i64),
    /// Whether to use HTTP compression.
    HttpCompression(bool),
    /// Maximum HTTP connections.
    HttpMaxConnections(i64),
    /// Whether to use chunked HTTP requests.
    HttpChunkedRequests(bool),

    // SSL section (from servers file)
    /// SSL certificate authority files.
    SslAuthorityFiles(&'a str),
    /// Whether to trust the default CA.
    SslTrustDefaultCa(bool),
    /// SSL client certificate file.
    SslClientCertFile(&'a str),
    /// SSL client certificate password.
    SslClientCertPassword(&'a str),

    // Generic string/int/bool options for sections not covered above
    /// Generic string configuration option.
    String {
        /// Configuration section name.
        section: &'a str,
        /// Option name.
        option: &'a str,
        /// String value.
        value: &'a str,
    },
    /// Generic integer configuration option.
    Int {
        /// Configuration section name.
        section: &'a str,
        /// Option name.
        option: &'a str,
        /// Integer value.
        value: i64,
    },
    /// Generic boolean configuration option.
    Bool {
        /// Configuration section name.
        section: &'a str,
        /// Option name.
        option: &'a str,
        /// Boolean value.
        value: bool,
    },
}

/// Configuration container wrapping svn_config_t
pub struct Config {
    ptr: *mut subversion_sys::svn_config_t,
    _pool: apr::Pool,
}

/// Configuration hash wrapping apr_hash_t (as expected by repository access APIs)
pub struct ConfigHash {
    ptr: *mut subversion_sys::apr_hash_t,
    _pool: apr::Pool,
}

/// Internal helper for accessing hash with C string keys
struct StringKeyHash {
    ptr: *mut subversion_sys::apr_hash_t,
}

impl StringKeyHash {
    unsafe fn from_ptr(ptr: *mut subversion_sys::apr_hash_t) -> Self {
        Self { ptr }
    }

    /// Get a value using a null-terminated C string key
    fn get(&self, key: &str) -> Result<*mut c_void, Error> {
        let c_key = CString::new(key)?;
        unsafe {
            let value = apr_sys::apr_hash_get(
                self.ptr,
                c_key.as_ptr() as *const c_void,
                apr_sys::APR_HASH_KEY_STRING as subversion_sys::apr_ssize_t,
            );
            Ok(value)
        }
    }
}

impl Config {
    /// Create from raw pointer and pool
    pub(crate) unsafe fn from_ptr_and_pool(
        ptr: *mut subversion_sys::svn_config_t,
        pool: apr::Pool,
    ) -> Self {
        Self { ptr, _pool: pool }
    }

    /// Get a configuration option value
    pub fn get(&self, option: ConfigOption) -> Result<ConfigValue, Error> {
        match option {
            // Auth options
            ConfigOption::StorePasswords(_) => {
                self.get_bool_value("auth", "store-passwords", Some(true))
            }
            ConfigOption::StorePlaintextPasswords(_) => {
                self.get_bool_value("auth", "store-plaintext-passwords", None)
            }
            ConfigOption::StoreAuthCreds(_) => {
                self.get_bool_value("auth", "store-auth-creds", Some(true))
            }
            ConfigOption::StoreSslClientCertPP(_) => {
                self.get_bool_value("auth", "store-ssl-client-cert-pp", Some(true))
            }
            ConfigOption::StoreSslClientCertPPPlaintext(_) => {
                self.get_bool_value("auth", "store-ssl-client-cert-pp-plaintext", None)
            }
            ConfigOption::PasswordStores(_) => self.get_string_value("auth", "password-stores"),
            ConfigOption::KwalletWallet(_) => self.get_string_value("auth", "kwallet-wallet"),
            ConfigOption::KwalletSvnApplicationName(_) => {
                self.get_string_value("auth", "kwallet-svn-application-name")
            }
            ConfigOption::SslClientCertFilePrompt(_) => {
                self.get_bool_value("auth", "ssl-client-cert-file-prompt", Some(false))
            }

            // Helpers options
            ConfigOption::EditorCmd(_) => self.get_string_value("helpers", "editor-cmd"),
            ConfigOption::DiffCmd(_) => self.get_string_value("helpers", "diff-cmd"),
            ConfigOption::Diff3Cmd(_) => self.get_string_value("helpers", "diff3-cmd"),
            ConfigOption::DiffExtensions(_) => self.get_string_value("helpers", "diff-extensions"),
            ConfigOption::MergeToolCmd(_) => self.get_string_value("helpers", "merge-tool-cmd"),

            // Miscellany options
            ConfigOption::GlobalIgnores(_) => self.get_string_value("miscellany", "global-ignores"),
            ConfigOption::LogEncoding(_) => self.get_string_value("miscellany", "log-encoding"),
            ConfigOption::UseCommitTimes(_) => {
                self.get_bool_value("miscellany", "use-commit-times", Some(false))
            }
            ConfigOption::NoUnlock(_) => {
                self.get_bool_value("miscellany", "no-unlock", Some(false))
            }
            ConfigOption::MimeTypesFile(_) => {
                self.get_string_value("miscellany", "mime-types-file")
            }
            ConfigOption::PreservedConflictFileExt(_) => {
                self.get_string_value("miscellany", "preserved-conflict-file-exts")
            }
            ConfigOption::EnableAutoProps(_) => {
                self.get_bool_value("miscellany", "enable-auto-props", Some(false))
            }
            ConfigOption::InteractiveConflicts(_) => {
                self.get_bool_value("miscellany", "interactive-conflicts", Some(true))
            }
            ConfigOption::MemoryCacheSize(_) => {
                self.get_int_value("miscellany", "memory-cache-size", Some(16777216))
            }
            ConfigOption::DiffIgnoreContentType(_) => {
                self.get_bool_value("miscellany", "diff-ignore-content-type", Some(false))
            }

            // Working copy options
            ConfigOption::ExclusiveLocking(_) => {
                self.get_bool_value("working-copy", "exclusive-locking", Some(false))
            }
            ConfigOption::ExclusiveLockingClients(_) => {
                self.get_string_value("working-copy", "exclusive-locking-clients")
            }
            ConfigOption::BusyTimeout(_) => {
                self.get_int_value("working-copy", "busy-timeout", Some(10000))
            }

            // Proxy options
            ConfigOption::HttpProxy(_) => self.get_string_value("global", "http-proxy"),
            ConfigOption::HttpProxyPort(_) => {
                self.get_int_value("global", "http-proxy-port", Some(80))
            }
            ConfigOption::HttpProxyUsername(_) => {
                self.get_string_value("global", "http-proxy-username")
            }
            ConfigOption::HttpProxyPassword(_) => {
                self.get_string_value("global", "http-proxy-password")
            }
            ConfigOption::HttpProxyExceptions(_) => {
                self.get_string_value("global", "http-proxy-exceptions")
            }
            ConfigOption::HttpTimeout(_) => self.get_int_value("global", "http-timeout", Some(0)),
            ConfigOption::HttpCompression(_) => {
                self.get_bool_value("global", "http-compression", Some(true))
            }
            ConfigOption::HttpMaxConnections(_) => {
                self.get_int_value("global", "http-max-connections", Some(4))
            }
            ConfigOption::HttpChunkedRequests(_) => {
                self.get_bool_value("global", "http-chunked-requests", Some(true))
            }

            // SSL options
            ConfigOption::SslAuthorityFiles(_) => {
                self.get_string_value("global", "ssl-authority-files")
            }
            ConfigOption::SslTrustDefaultCa(_) => {
                self.get_bool_value("global", "ssl-trust-default-ca", Some(true))
            }
            ConfigOption::SslClientCertFile(_) => {
                self.get_string_value("global", "ssl-client-cert-file")
            }
            ConfigOption::SslClientCertPassword(_) => {
                self.get_string_value("global", "ssl-client-cert-password")
            }

            // Generic options
            ConfigOption::String {
                section, option, ..
            } => self.get_string_value(section, option),
            ConfigOption::Int {
                section, option, ..
            } => self.get_int_value(section, option, None),
            ConfigOption::Bool {
                section, option, ..
            } => self.get_bool_value(section, option, None),
        }
    }

    /// Set a configuration option
    pub fn set(&mut self, option: ConfigOption) -> Result<(), Error> {
        let (section, key, value) = match option {
            // Auth options
            ConfigOption::StorePasswords(v) => ("auth", "store-passwords", format!("{}", v)),
            ConfigOption::StorePlaintextPasswords(v) => {
                ("auth", "store-plaintext-passwords", format!("{}", v))
            }
            ConfigOption::StoreAuthCreds(v) => ("auth", "store-auth-creds", format!("{}", v)),
            ConfigOption::StoreSslClientCertPP(v) => {
                ("auth", "store-ssl-client-cert-pp", format!("{}", v))
            }
            ConfigOption::StoreSslClientCertPPPlaintext(v) => (
                "auth",
                "store-ssl-client-cert-pp-plaintext",
                format!("{}", v),
            ),
            ConfigOption::PasswordStores(v) => ("auth", "password-stores", v.to_string()),
            ConfigOption::KwalletWallet(v) => ("auth", "kwallet-wallet", v.to_string()),
            ConfigOption::KwalletSvnApplicationName(v) => {
                ("auth", "kwallet-svn-application-name", v.to_string())
            }
            ConfigOption::SslClientCertFilePrompt(v) => {
                ("auth", "ssl-client-cert-file-prompt", format!("{}", v))
            }

            // Helpers options
            ConfigOption::EditorCmd(v) => ("helpers", "editor-cmd", v.to_string()),
            ConfigOption::DiffCmd(v) => ("helpers", "diff-cmd", v.to_string()),
            ConfigOption::Diff3Cmd(v) => ("helpers", "diff3-cmd", v.to_string()),
            ConfigOption::DiffExtensions(v) => ("helpers", "diff-extensions", v.to_string()),
            ConfigOption::MergeToolCmd(v) => ("helpers", "merge-tool-cmd", v.to_string()),

            // Miscellany options
            ConfigOption::GlobalIgnores(v) => ("miscellany", "global-ignores", v.to_string()),
            ConfigOption::LogEncoding(v) => ("miscellany", "log-encoding", v.to_string()),
            ConfigOption::UseCommitTimes(v) => ("miscellany", "use-commit-times", format!("{}", v)),
            ConfigOption::NoUnlock(v) => ("miscellany", "no-unlock", format!("{}", v)),
            ConfigOption::MimeTypesFile(v) => ("miscellany", "mime-types-file", v.to_string()),
            ConfigOption::PreservedConflictFileExt(v) => {
                ("miscellany", "preserved-conflict-file-exts", v.to_string())
            }
            ConfigOption::EnableAutoProps(v) => {
                ("miscellany", "enable-auto-props", format!("{}", v))
            }
            ConfigOption::InteractiveConflicts(v) => {
                ("miscellany", "interactive-conflicts", format!("{}", v))
            }
            ConfigOption::MemoryCacheSize(v) => {
                ("miscellany", "memory-cache-size", format!("{}", v))
            }
            ConfigOption::DiffIgnoreContentType(v) => {
                ("miscellany", "diff-ignore-content-type", format!("{}", v))
            }

            // Working copy options
            ConfigOption::ExclusiveLocking(v) => {
                ("working-copy", "exclusive-locking", format!("{}", v))
            }
            ConfigOption::ExclusiveLockingClients(v) => {
                ("working-copy", "exclusive-locking-clients", v.to_string())
            }
            ConfigOption::BusyTimeout(v) => ("working-copy", "busy-timeout", format!("{}", v)),

            // Proxy options
            ConfigOption::HttpProxy(v) => ("global", "http-proxy", v.to_string()),
            ConfigOption::HttpProxyPort(v) => ("global", "http-proxy-port", format!("{}", v)),
            ConfigOption::HttpProxyUsername(v) => ("global", "http-proxy-username", v.to_string()),
            ConfigOption::HttpProxyPassword(v) => ("global", "http-proxy-password", v.to_string()),
            ConfigOption::HttpProxyExceptions(v) => {
                ("global", "http-proxy-exceptions", v.to_string())
            }
            ConfigOption::HttpTimeout(v) => ("global", "http-timeout", format!("{}", v)),
            ConfigOption::HttpCompression(v) => ("global", "http-compression", format!("{}", v)),
            ConfigOption::HttpMaxConnections(v) => {
                ("global", "http-max-connections", format!("{}", v))
            }
            ConfigOption::HttpChunkedRequests(v) => {
                ("global", "http-chunked-requests", format!("{}", v))
            }

            // SSL options
            ConfigOption::SslAuthorityFiles(v) => ("global", "ssl-authority-files", v.to_string()),
            ConfigOption::SslTrustDefaultCa(v) => {
                ("global", "ssl-trust-default-ca", format!("{}", v))
            }
            ConfigOption::SslClientCertFile(v) => ("global", "ssl-client-cert-file", v.to_string()),
            ConfigOption::SslClientCertPassword(v) => {
                ("global", "ssl-client-cert-password", v.to_string())
            }

            // Generic options
            ConfigOption::String {
                section,
                option,
                value,
            } => (section, option, value.to_string()),
            ConfigOption::Int {
                section,
                option,
                value,
            } => (section, option, format!("{}", value)),
            ConfigOption::Bool {
                section,
                option,
                value,
            } => (section, option, format!("{}", value)),
        };

        self.set_value(section, key, &value)
    }

    // Internal helper functions
    fn get_string_value(&self, section: &str, option: &str) -> Result<ConfigValue, Error> {
        let section_cstr = CString::new(section)?;
        let option_cstr = CString::new(option)?;

        unsafe {
            let mut value_ptr = ptr::null();
            subversion_sys::svn_config_get(
                self.ptr,
                &mut value_ptr,
                section_cstr.as_ptr(),
                option_cstr.as_ptr(),
                ptr::null(),
            );

            if value_ptr.is_null() {
                Ok(ConfigValue::None)
            } else {
                let value_cstr = std::ffi::CStr::from_ptr(value_ptr);
                Ok(ConfigValue::String(
                    value_cstr.to_string_lossy().into_owned(),
                ))
            }
        }
    }

    fn get_bool_value(
        &self,
        section: &str,
        option: &str,
        default: Option<bool>,
    ) -> Result<ConfigValue, Error> {
        let section_cstr = CString::new(section)?;
        let option_cstr = CString::new(option)?;

        unsafe {
            let mut result = 0;
            let err = subversion_sys::svn_config_get_bool(
                self.ptr,
                &mut result,
                section_cstr.as_ptr(),
                option_cstr.as_ptr(),
                default.unwrap_or(false) as i32,
            );
            svn_result(err)?;
            Ok(ConfigValue::Bool(result != 0))
        }
    }

    fn get_int_value(
        &self,
        section: &str,
        option: &str,
        default: Option<i64>,
    ) -> Result<ConfigValue, Error> {
        let section_cstr = CString::new(section)?;
        let option_cstr = CString::new(option)?;

        unsafe {
            let mut result = 0;
            let err = subversion_sys::svn_config_get_int64(
                self.ptr,
                &mut result,
                section_cstr.as_ptr(),
                option_cstr.as_ptr(),
                default.unwrap_or(0),
            );
            svn_result(err)?;
            Ok(ConfigValue::Int(result))
        }
    }

    fn set_value(&mut self, section: &str, option: &str, value: &str) -> Result<(), Error> {
        let section_cstr = CString::new(section)?;
        let option_cstr = CString::new(option)?;
        let value_cstr = CString::new(value)?;

        unsafe {
            subversion_sys::svn_config_set(
                self.ptr,
                section_cstr.as_ptr(),
                option_cstr.as_ptr(),
                value_cstr.as_ptr(),
            );
        }
        Ok(())
    }

    /// Returns a raw pointer to the configuration.
    pub fn as_ptr(&self) -> *const subversion_sys::svn_config_t {
        self.ptr
    }

    /// Returns a mutable raw pointer to the configuration.
    pub fn as_mut_ptr(&mut self) -> *mut subversion_sys::svn_config_t {
        self.ptr
    }
}

impl ConfigHash {
    /// Create from raw pointer and pool
    pub(crate) unsafe fn from_ptr_and_pool(
        ptr: *mut subversion_sys::apr_hash_t,
        pool: apr::Pool,
    ) -> Self {
        Self { ptr, _pool: pool }
    }

    #[allow(dead_code)]
    pub(crate) fn as_ptr(&self) -> *const subversion_sys::apr_hash_t {
        self.ptr
    }

    pub(crate) fn as_mut_ptr(&mut self) -> *mut subversion_sys::apr_hash_t {
        self.ptr
    }
}

/// Configuration value returned from get operations
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigValue {
    /// String configuration value.
    String(String),
    /// Integer configuration value.
    Int(i64),
    /// Boolean configuration value.
    Bool(bool),
    /// No value present.
    None,
}

impl ConfigValue {
    /// Returns the value as a string if it is one.
    pub fn as_string(&self) -> Option<&str> {
        match self {
            ConfigValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// Returns the value as an integer if it is one.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            ConfigValue::Int(i) => Some(*i),
            _ => None,
        }
    }

    /// Returns the value as a boolean if it is one.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ConfigValue::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

/// Get the configuration hash (for repository access APIs)
pub fn get_config_hash(config_dir: Option<&Path>) -> Result<ConfigHash, Error> {
    let pool = apr::Pool::new();

    let config_dir_cstr = if let Some(dir) = config_dir {
        Some(CString::new(dir.to_str().ok_or_else(|| {
            Error::from_str("Invalid config directory path")
        })?)?)
    } else {
        None
    };

    let config_dir_ptr = config_dir_cstr
        .as_ref()
        .map(|s| s.as_ptr())
        .unwrap_or(ptr::null());

    unsafe {
        let mut cfg_hash = ptr::null_mut();
        let err =
            subversion_sys::svn_config_get_config(&mut cfg_hash, config_dir_ptr, pool.as_mut_ptr());
        svn_result(err)?;
        Ok(ConfigHash::from_ptr_and_pool(cfg_hash, pool))
    }
}

/// Read configuration from the default locations
pub fn get_config(config_dir: Option<&Path>) -> Result<(Config, Config), Error> {
    let pool = apr::Pool::new();

    let config_dir_cstr = if let Some(dir) = config_dir {
        Some(CString::new(dir.to_str().ok_or_else(|| {
            Error::from_str("Invalid config directory path")
        })?)?)
    } else {
        None
    };

    let config_dir_ptr = config_dir_cstr
        .as_ref()
        .map(|s| s.as_ptr())
        .unwrap_or(ptr::null());

    unsafe {
        let mut cfg_hash = ptr::null_mut();
        let err =
            subversion_sys::svn_config_get_config(&mut cfg_hash, config_dir_ptr, pool.as_mut_ptr());
        svn_result(err)?;

        // Use our StringKeyHash wrapper to access the config hash
        let hash = StringKeyHash::from_ptr(cfg_hash);

        let config_ptr =
            hash.get("config").unwrap_or(ptr::null_mut()) as *mut subversion_sys::svn_config_t;
        let servers_ptr =
            hash.get("servers").unwrap_or(ptr::null_mut()) as *mut subversion_sys::svn_config_t;

        Ok((
            Config::from_ptr_and_pool(config_ptr, apr::Pool::new()),
            Config::from_ptr_and_pool(servers_ptr, apr::Pool::new()),
        ))
    }
}

/// Read a configuration file
pub fn read_config(file: &Path, must_exist: bool) -> Result<Config, Error> {
    let pool = apr::Pool::new();
    let file_cstr = CString::new(
        file.to_str()
            .ok_or_else(|| Error::from_str("Invalid file path"))?,
    )?;

    unsafe {
        let mut cfg = ptr::null_mut();
        let err = subversion_sys::svn_config_read3(
            &mut cfg,
            file_cstr.as_ptr(),
            must_exist as i32,
            0, // case_sensitive
            0, // expand
            pool.as_mut_ptr(),
        );
        svn_result(err)?;
        Ok(Config::from_ptr_and_pool(cfg, pool))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_option_enum() {
        // Test that the enum can be created and used
        let opt = ConfigOption::StorePasswords(true);
        match opt {
            ConfigOption::StorePasswords(v) => assert!(v),
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_config_value_conversions() {
        let val = ConfigValue::String("test".to_string());
        assert_eq!(val.as_string(), Some("test"));
        assert_eq!(val.as_int(), None);
        assert_eq!(val.as_bool(), None);

        let val = ConfigValue::Int(42);
        assert_eq!(val.as_string(), None);
        assert_eq!(val.as_int(), Some(42));
        assert_eq!(val.as_bool(), None);

        let val = ConfigValue::Bool(true);
        assert_eq!(val.as_string(), None);
        assert_eq!(val.as_int(), None);
        assert_eq!(val.as_bool(), Some(true));
    }
}
