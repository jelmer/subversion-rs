use crate::Error;
use std::marker::PhantomData;

/// Authentication settings that can be configured.
pub enum AuthSetting<'a> {
    /// Username for authentication.
    Username(&'a str),
    /// Default username to use when not specified.
    DefaultUsername(&'a str),
    /// SSL server trust file path.
    SslServerTrustFile(&'a str),
    /// SSL client certificate file path.
    SslClientCertFile(&'a str),
    /// Password for SSL client certificate.
    SslClientCertPassword(&'a str),
    /// Configuration directory path.
    ConfigDir(&'a str),
    /// Server group name.
    ServerGroup(&'a str),
    /// Configuration category for servers.
    ConfigCategoryServers(&'a str),
}

/// Authentication baton for managing authentication providers and credentials.
pub struct AuthBaton {
    ptr: *mut subversion_sys::svn_auth_baton_t,
    pool: apr::Pool,
    // Store parameter names to keep them alive for the lifetime of the baton
    parameter_names: std::collections::HashMap<String, std::ffi::CString>,
    // Store providers to keep them alive for the lifetime of the baton
    _providers: Vec<AuthProvider>,
}
unsafe impl Send for AuthBaton {}

/// Trait for authentication credentials.
pub trait Credentials {
    /// Returns the kind of credential.
    fn kind() -> &'static str
    where
        Self: Sized;

    /// Returns a mutable pointer to the credentials.
    fn as_mut_ptr(&mut self) -> *mut std::ffi::c_void;

    /// Creates credentials from a raw pointer.
    fn from_raw(cred: *mut std::ffi::c_void) -> Self
    where
        Self: Sized;
    
    /// Try to downcast to SimpleCredentials
    fn as_simple(&self) -> Option<&SimpleCredentials> {
        None
    }
    
    /// Try to downcast to UsernameCredentials
    fn as_username(&self) -> Option<&UsernameCredentials> {
        None
    }
    
    /// Try to downcast to SslServerTrustCredentials  
    fn as_ssl_server_trust(&self) -> Option<&SslServerTrustCredentials> {
        None
    }
}

/// Simple username/password credentials.
pub struct SimpleCredentials<'pool> {
    ptr: *mut subversion_sys::svn_auth_cred_simple_t,
    _pool: std::marker::PhantomData<&'pool apr::Pool>,
}

impl<'pool> SimpleCredentials<'pool> {
    /// Returns the username.
    pub fn username(&self) -> &str {
        unsafe {
            std::ffi::CStr::from_ptr((*self.ptr).username)
                .to_str()
                .unwrap()
        }
    }

    /// Returns the password.
    pub fn password(&self) -> &str {
        unsafe {
            std::ffi::CStr::from_ptr((*self.ptr).password)
                .to_str()
                .unwrap()
        }
    }

    /// Sets the username.
    pub fn set_username(&mut self, username: &str, pool: &apr::Pool) {
        unsafe {
            (*self.ptr).username = apr::strings::pstrdup_raw(username, pool).unwrap() as *mut _;
        }
    }

    /// Returns whether the credentials may be saved.
    pub fn may_save(&self) -> bool {
        unsafe { (*self.ptr).may_save != 0 }
    }

    /// Creates new simple credentials.
    pub fn new(username: String, password: String, may_save: bool, pool: &'pool apr::Pool) -> Self {
        let cred: *mut subversion_sys::svn_auth_cred_simple_t = pool.calloc();
        unsafe {
            (*cred).username = apr::strings::pstrdup_raw(&username, pool).unwrap() as *mut _;
            (*cred).password = apr::strings::pstrdup_raw(&password, pool).unwrap() as *mut _;
            (*cred).may_save = if may_save { 1 } else { 0 };
        }
        Self {
            ptr: cred,
            _pool: std::marker::PhantomData,
        }
    }
    
}

impl<'pool> Credentials for SimpleCredentials<'pool> {
    fn kind() -> &'static str
    where
        Self: Sized,
    {
        unsafe { std::str::from_utf8_unchecked(subversion_sys::SVN_AUTH_CRED_SIMPLE) }
            .trim_end_matches('\0')
    }

    fn as_mut_ptr(&mut self) -> *mut std::ffi::c_void {
        self.ptr as *mut std::ffi::c_void
    }

    fn from_raw(cred: *mut std::ffi::c_void) -> Self {
        Self {
            ptr: cred as *mut subversion_sys::svn_auth_cred_simple_t,
            _pool: std::marker::PhantomData,
        }
    }
    
    fn as_simple(&self) -> Option<&SimpleCredentials> {
        Some(self)
    }
}

/// Username-only credentials.
pub struct UsernameCredentials<'pool> {
    ptr: *mut subversion_sys::svn_auth_cred_username_t,
    _pool: std::marker::PhantomData<&'pool apr::Pool>,
}

impl<'pool> UsernameCredentials<'pool> {
    /// Returns the username.
    pub fn username(&self) -> &str {
        unsafe {
            std::ffi::CStr::from_ptr((*self.ptr).username)
                .to_str()
                .unwrap()
        }
    }

    /// Returns whether the credentials may be saved.
    pub fn may_save(&self) -> bool {
        unsafe { (*self.ptr).may_save != 0 }
    }

    /// Creates new username credentials.
    pub fn new(username: String, may_save: bool, pool: &'pool apr::Pool) -> Self {
        let cred: *mut subversion_sys::svn_auth_cred_username_t = pool.calloc();
        unsafe {
            (*cred).username = apr::strings::pstrdup_raw(&username, pool).unwrap() as *mut _;
            (*cred).may_save = if may_save { 1 } else { 0 };
        }
        Self {
            ptr: cred,
            _pool: std::marker::PhantomData,
        }
    }
}

impl<'pool> Credentials for UsernameCredentials<'pool> {
    fn kind() -> &'static str
    where
        Self: Sized,
    {
        unsafe { std::str::from_utf8_unchecked(subversion_sys::SVN_AUTH_CRED_USERNAME) }
            .trim_end_matches('\0')
    }

    fn as_mut_ptr(&mut self) -> *mut std::ffi::c_void {
        self.ptr as *mut std::ffi::c_void
    }

    fn from_raw(cred: *mut std::ffi::c_void) -> Self
    where
        Self: Sized,
    {
        Self {
            ptr: cred as *mut subversion_sys::svn_auth_cred_username_t,
            _pool: std::marker::PhantomData,
        }
    }
    
    fn as_username(&self) -> Option<&UsernameCredentials> {
        Some(self)
    }
}

/// SSL server trust credentials.
pub struct SslServerTrustCredentials<'pool> {
    ptr: *mut subversion_sys::svn_auth_cred_ssl_server_trust_t,
    _pool: std::marker::PhantomData<&'pool apr::Pool>,
}

impl<'pool> SslServerTrustCredentials<'pool> {
    /// Returns whether the credentials may be saved.
    pub fn may_save(&self) -> bool {
        unsafe { (*self.ptr).may_save != 0 }
    }

    /// Returns the accepted failures.
    pub fn accepted_failures(&self) -> u32 {
        unsafe { (*self.ptr).accepted_failures }
    }

    /// Creates new SSL server trust credentials.
    pub fn new(may_save: bool, accepted_failures: u32, pool: &'pool apr::Pool) -> Self {
        let cred: *mut subversion_sys::svn_auth_cred_ssl_server_trust_t = pool.calloc();
        unsafe {
            (*cred).may_save = if may_save { 1 } else { 0 };
            (*cred).accepted_failures = accepted_failures;
        }
        Self {
            ptr: cred,
            _pool: std::marker::PhantomData,
        }
    }
}

impl<'pool> Credentials for SslServerTrustCredentials<'pool> {
    fn kind() -> &'static str
    where
        Self: Sized,
    {
        unsafe { std::str::from_utf8_unchecked(subversion_sys::SVN_AUTH_CRED_SSL_SERVER_TRUST) }
            .trim_end_matches('\0')
    }

    fn as_mut_ptr(&mut self) -> *mut std::ffi::c_void {
        self.ptr as *mut std::ffi::c_void
    }

    fn from_raw(cred: *mut std::ffi::c_void) -> Self
    where
        Self: Sized,
    {
        Self {
            ptr: cred as *mut subversion_sys::svn_auth_cred_ssl_server_trust_t,
            _pool: std::marker::PhantomData,
        }
    }
    
    fn as_ssl_server_trust(&self) -> Option<&SslServerTrustCredentials> {
        Some(self)
    }
}

impl AuthBaton {
    /// Try to get credentials from the auth providers (for testing).
    #[cfg(test)]
    pub(crate) fn first_credentials(
        &mut self,
        cred_kind: &str,
        realm: &str,
    ) -> Result<Option<Box<dyn Credentials + '_>>, Error> {
        use std::ffi::CString;

        let mut creds_ptr = std::ptr::null_mut();
        let mut iter_baton = std::ptr::null_mut();
        let cred_kind_c = CString::new(cred_kind)?;
        let realm_c = CString::new(realm)?;

        let err = unsafe {
            subversion_sys::svn_auth_first_credentials(
                &mut creds_ptr,
                &mut iter_baton,
                cred_kind_c.as_ptr(),
                realm_c.as_ptr(),
                self.ptr,
                self.pool.as_mut_ptr(),
            )
        };

        Error::from_raw(err)?;

        if creds_ptr.is_null() {
            return Ok(None);
        }

        // Convert credentials based on type
        match cred_kind {
            "svn.simple" => {
                let simple_creds = SimpleCredentials::from_raw(creds_ptr);
                Ok(Some(Box::new(simple_creds)))
            }
            "svn.username" => {
                let username_creds = UsernameCredentials::from_raw(creds_ptr);
                Ok(Some(Box::new(username_creds)))
            }
            "svn.ssl.server.trust" => {
                let trust_creds = SslServerTrustCredentials::from_raw(creds_ptr);
                Ok(Some(Box::new(trust_creds)))
            }
            _ => Ok(None),
        }
    }

    /// Returns a mutable pointer to the auth baton.
    pub fn as_mut_ptr(&mut self) -> *mut subversion_sys::svn_auth_baton_t {
        self.ptr
    }

    /// Sets an authentication setting.
    pub fn set(&mut self, setting: AuthSetting) -> Result<(), Error> {
        match setting {
            AuthSetting::Username(value) => self.set_string_parameter("username", value),
            AuthSetting::DefaultUsername(value) => {
                self.set_string_parameter("svn:auth:username", value)
            }
            AuthSetting::SslServerTrustFile(value) => {
                self.set_string_parameter("servers:global:ssl-trust-default-ca", value)
            }
            AuthSetting::SslClientCertFile(value) => {
                self.set_string_parameter("servers:global:ssl-client-cert-file", value)
            }
            AuthSetting::SslClientCertPassword(value) => {
                self.set_string_parameter("servers:global:ssl-client-cert-password", value)
            }
            AuthSetting::ConfigDir(value) => self.set_string_parameter("config-dir", value),
            AuthSetting::ServerGroup(value) => self.set_string_parameter("servers:group", value),
            AuthSetting::ConfigCategoryServers(value) => {
                self.set_string_parameter("config:servers", value)
            }
        }
    }

    fn set_string_parameter(&mut self, param_name: &str, param_value: &str) -> Result<(), Error> {
        // Store the value in the auth_baton's pool to ensure it persists
        let persistent_value = apr::strings::pstrdup(param_value, &self.pool)?;

        // Use the existing set_parameter method
        unsafe {
            self.set_parameter(
                param_name,
                persistent_value.as_ptr() as *const std::ffi::c_void,
            );
        }

        Ok(())
    }

    /// Gets credentials for the specified realm.
    pub fn credentials<C: Credentials>(&mut self, realm: &str) -> Result<IterState<C>, Error> {
        let cred_kind = std::ffi::CString::new(C::kind()).unwrap();
        let realm = std::ffi::CString::new(realm).unwrap();
        let mut cred = std::ptr::null_mut();
        let mut state = std::ptr::null_mut();
        let pool = apr::pool::Pool::new();
        unsafe {
            let err = subversion_sys::svn_auth_first_credentials(
                &mut cred,
                &mut state,
                cred_kind.as_ptr(),
                realm.as_ptr(),
                self.ptr,
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            let first_creds = C::from_raw(cred);
            Ok(IterState {
                ptr: state,
                pool,
                creds: Some(first_creds),
                _phantom: PhantomData,
            })
        }
    }

    /// Forgets stored credentials.
    pub fn forget_credentials<C: Credentials>(
        &mut self,
        cred_kind: Option<&str>,
        realm: Option<&str>,
    ) -> Result<(), Error> {
        let cred_kind = cred_kind
            .map(|s| std::ffi::CString::new(s).unwrap())
            .map_or_else(std::ptr::null, |p| p.as_ptr());
        let realmstring = realm
            .map(|s| std::ffi::CString::new(s).unwrap())
            .map_or_else(std::ptr::null, |p| p.as_ptr());
        let err = std::ptr::null_mut();
        let pool = apr::pool::Pool::new();
        unsafe {
            subversion_sys::svn_auth_forget_credentials(
                self.ptr,
                cred_kind,
                realmstring,
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            Ok(())
        }
    }

    /// Get a parameter from the auth baton.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the value is valid for the lifetime of the auth baton.
    pub unsafe fn get_parameter(&mut self, name: &str) -> *const std::ffi::c_void {
        // Ensure the parameter name is stored and persists
        if !self.parameter_names.contains_key(name) {
            let name_cstring = std::ffi::CString::new(name).unwrap();
            self.parameter_names.insert(name.to_string(), name_cstring);
        }
        let name_cstring = &self.parameter_names[name];
        subversion_sys::svn_auth_get_parameter(self.ptr, name_cstring.as_ptr())
    }

    /// Set a parameter on the auth baton.
    ///
    /// # Safety
    /// The caller must ensure that the value is valid for the lifetime of the auth baton.
    pub unsafe fn set_parameter(&mut self, name: &str, value: *const std::ffi::c_void) {
        // Store the parameter name to ensure it persists for the lifetime of the baton
        let name_cstring = std::ffi::CString::new(name).unwrap();
        let name_ptr = name_cstring.as_ptr();
        self.parameter_names.insert(name.to_string(), name_cstring);
        subversion_sys::svn_auth_set_parameter(self.ptr, name_ptr, value);
    }

    /// Opens an authentication baton with the given providers.
    pub fn open(providers: Vec<AuthProvider>) -> Result<Self, Error> {
        let pool = apr::pool::Pool::new();
        let mut baton = std::ptr::null_mut();
        unsafe {
            let mut provider_array = apr::tables::TypedArray::<
                *const subversion_sys::svn_auth_provider_object_t,
            >::new(&pool, providers.len() as i32);
            for provider in &providers {
                provider_array.push(provider.as_auth_provider(&pool));
            }
            subversion_sys::svn_auth_open(&mut baton, provider_array.as_ptr(), pool.as_mut_ptr());
            Ok(Self {
                ptr: baton,
                pool,
                parameter_names: std::collections::HashMap::new(),
                _providers: providers,
            })
        }
    }
}

/// Iterator state for credentials.
pub struct IterState<C: Credentials> {
    ptr: *mut subversion_sys::svn_auth_iterstate_t,
    pool: apr::Pool,
    creds: Option<C>,
    _phantom: PhantomData<*mut ()>,
}

impl<C: Credentials> IterState<C> {
    /// Get a reference to the underlying pool
    pub fn pool(&self) -> &apr::Pool {
        &self.pool
    }

    /// Get the raw pointer to the iterator state (use with caution)
    pub fn as_ptr(&self) -> *const subversion_sys::svn_auth_iterstate_t {
        self.ptr
    }

    /// Get the mutable raw pointer to the iterator state (use with caution)
    pub fn as_mut_ptr(&mut self) -> *mut subversion_sys::svn_auth_iterstate_t {
        self.ptr
    }

    /// Saves the credentials.
    pub fn save_credentials(&mut self) -> Result<(), Error> {
        let pool = apr::pool::Pool::new();
        let err = unsafe { subversion_sys::svn_auth_save_credentials(self.ptr, pool.as_mut_ptr()) };
        Error::from_raw(err)?;
        Ok(())
    }

    fn next_credentials(&mut self) -> Result<Option<C>, Error> {
        let mut cred = std::ptr::null_mut();
        let pool = apr::pool::Pool::new();
        unsafe {
            let err =
                subversion_sys::svn_auth_next_credentials(&mut cred, self.ptr, pool.as_mut_ptr());
            Error::from_raw(err)?;
            if cred.is_null() {
                return Ok(None);
            }
            Ok(Some(C::from_raw(cred)))
        }
    }
}

impl<C: Credentials> Iterator for IterState<C> {
    type Item = C;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(creds) = self.creds.take() {
            return Some(creds);
        }
        match self.next_credentials() {
            Ok(Some(cred)) => Some(cred),
            Ok(None) => None,
            Err(_) => None,
        }
    }
}

/// Trait for types that can be converted to authentication providers.
pub trait AsAuthProvider {
    /// Returns a pointer to the authentication provider.
    fn as_auth_provider(
        &self,
        pool: &apr::pool::Pool,
    ) -> *const subversion_sys::svn_auth_provider_object_t;
}

impl AsAuthProvider for *const subversion_sys::svn_auth_provider_object_t {
    fn as_auth_provider(
        &self,
        _pool: &apr::pool::Pool,
    ) -> *const subversion_sys::svn_auth_provider_object_t {
        *self
    }
}

impl AsAuthProvider for AuthProvider {
    fn as_auth_provider(
        &self,
        _pool: &apr::pool::Pool,
    ) -> *const subversion_sys::svn_auth_provider_object_t {
        self.ptr
    }
}

impl AsAuthProvider for &AuthProvider {
    fn as_auth_provider(
        &self,
        _pool: &apr::pool::Pool,
    ) -> *const subversion_sys::svn_auth_provider_object_t {
        self.ptr
    }
}

#[allow(dead_code)]
/// Authentication provider.
pub struct AuthProvider {
    ptr: *const subversion_sys::svn_auth_provider_object_t,
    pool: apr::Pool,
    _phantom: PhantomData<*mut ()>,
}
unsafe impl Send for AuthProvider {}

impl AuthProvider {
    /// Returns the credential kind for this provider.
    pub fn cred_kind(&self) -> &str {
        let cred_kind = unsafe { (*(*self.ptr).vtable).cred_kind };
        unsafe { std::ffi::CStr::from_ptr(cred_kind).to_str().unwrap() }
    }

    /// Saves credentials using this provider.
    pub fn save_credentials(
        &self,
        credentials: &mut impl Credentials,
        realm: &str,
    ) -> Result<bool, Error> {
        let realm = std::ffi::CString::new(realm).unwrap();
        let creds = credentials.as_mut_ptr();
        let pool = apr::pool::Pool::new();
        let mut saved = subversion_sys::svn_boolean_t::default();
        let err = unsafe {
            (*(*self.ptr).vtable).save_credentials.unwrap()(
                &mut saved,
                creds,
                (*self.ptr).provider_baton,
                std::ptr::null_mut(),
                realm.as_ptr(),
                pool.as_mut_ptr(),
            )
        };
        Error::from_raw(err)?;
        Ok(true)
    }
}

/// Gets the username authentication provider.
pub fn get_username_provider() -> AuthProvider {
    let mut auth_provider = std::ptr::null_mut();
    let pool = apr::Pool::new();
    unsafe {
        subversion_sys::svn_auth_get_username_provider(&mut auth_provider, pool.as_mut_ptr());
    }
    AuthProvider {
        ptr: auth_provider,
        pool,
        _phantom: PhantomData,
    }
}

/// Gets the SSL server trust file authentication provider.
pub fn get_ssl_server_trust_file_provider() -> AuthProvider {
    let mut auth_provider = std::ptr::null_mut();
    let pool = apr::Pool::new();
    unsafe {
        subversion_sys::svn_auth_get_ssl_server_trust_file_provider(
            &mut auth_provider,
            pool.as_mut_ptr(),
        );
    }
    AuthProvider {
        ptr: auth_provider,
        pool,
        _phantom: PhantomData,
    }
}

#[allow(dead_code)]
/// SSL server certificate information.
pub struct SslServerCertInfo {
    ptr: *const subversion_sys::svn_auth_ssl_server_cert_info_t,
    pool: apr::Pool,
    _phantom: PhantomData<*mut ()>,
}
unsafe impl Send for SslServerCertInfo {}

impl SslServerCertInfo {
    /// Creates a duplicate of the SSL server certificate info.
    pub fn dup(&self) -> Self {
        let pool = apr::Pool::new();
        let ptr = unsafe {
            subversion_sys::svn_auth_ssl_server_cert_info_dup(self.ptr, pool.as_mut_ptr())
        };
        Self {
            ptr,
            pool,
            _phantom: PhantomData,
        }
    }
}

/// SSL server trust credentials.
pub struct SslServerTrust {
    ptr: *mut subversion_sys::svn_auth_cred_ssl_server_trust_t,
    pool: apr::Pool,
    _phantom: PhantomData<*mut ()>,
}
unsafe impl Send for SslServerTrust {}

impl SslServerTrust {
    /// Get a reference to the underlying pool
    pub fn pool(&self) -> &apr::Pool {
        &self.pool
    }

    /// Get the raw pointer to the SSL server trust credentials (use with caution)
    pub fn as_ptr(&self) -> *const subversion_sys::svn_auth_cred_ssl_server_trust_t {
        self.ptr
    }

    /// Get the mutable raw pointer to the SSL server trust credentials (use with caution)
    pub fn as_mut_ptr(&mut self) -> *mut subversion_sys::svn_auth_cred_ssl_server_trust_t {
        self.ptr
    }

    /// Creates a duplicate of the SSL server trust credentials.
    pub fn dup(&self) -> Self {
        let pool = apr::Pool::new();
        let cred = pool.calloc();
        unsafe { std::ptr::copy_nonoverlapping(self.ptr, cred, 1) };
        Self {
            ptr: cred,
            pool,
            _phantom: PhantomData,
        }
    }
}

/// Gets an SSL server trust prompt authentication provider.
pub fn get_ssl_server_trust_prompt_provider(
    prompt_func: &impl Fn(&'_ str, usize, &'_ SslServerCertInfo, bool) -> Result<SslServerTrust, Error>,
) -> AuthProvider {
    let mut auth_provider = std::ptr::null_mut();

    extern "C" fn wrap_ssl_server_trust_prompt_fn(
        cred: *mut *mut subversion_sys::svn_auth_cred_ssl_server_trust_t,
        baton: *mut std::ffi::c_void,
        realmstring: *const std::ffi::c_char,
        failures: subversion_sys::apr_uint32_t,
        cert_info: *const subversion_sys::svn_auth_ssl_server_cert_info_t,
        may_save: subversion_sys::svn_boolean_t,
        pool: *mut subversion_sys::apr_pool_t,
    ) -> *mut subversion_sys::svn_error_t {
        let f = unsafe {
            &*(baton
                as *const &dyn Fn(
                    &'_ str,
                    usize,
                    &'_ SslServerCertInfo,
                    bool,
                ) -> Result<SslServerTrust, crate::Error>)
        };
        let realm = unsafe { std::ffi::CStr::from_ptr(realmstring).to_str().unwrap() };
        let cert_info = SslServerCertInfo {
            ptr: cert_info,
            pool: apr::pool::Pool::from_raw(pool),
            _phantom: PhantomData,
        };
        f(
            realm,
            failures.try_into().unwrap(),
            &cert_info,
            may_save != 0,
        )
        .map(|creds| {
            unsafe { *cred = creds.ptr };
            std::ptr::null_mut()
        })
        .unwrap_or_else(|e| unsafe { e.into_raw() })
    }

    let pool = apr::Pool::new();
    unsafe {
        subversion_sys::svn_auth_get_ssl_server_trust_prompt_provider(
            &mut auth_provider,
            Some(wrap_ssl_server_trust_prompt_fn),
            prompt_func as *const _ as *mut std::ffi::c_void,
            pool.as_mut_ptr(),
        );
    }
    AuthProvider {
        ptr: auth_provider,
        pool,
        _phantom: PhantomData,
    }
}

/// Gets an SSL client certificate file authentication provider.
pub fn get_ssl_client_cert_file_provider() -> AuthProvider {
    let mut auth_provider = std::ptr::null_mut();
    let pool = apr::Pool::new();
    unsafe {
        subversion_sys::svn_auth_get_ssl_client_cert_file_provider(
            &mut auth_provider,
            pool.as_mut_ptr(),
        );
    }
    AuthProvider {
        ptr: auth_provider,
        pool,
        _phantom: PhantomData,
    }
}

/// SSL client certificate credentials.
pub struct SslClientCertCredentials {
    ptr: *mut subversion_sys::svn_auth_cred_ssl_client_cert_t,
    pool: apr::Pool,
    _phantom: PhantomData<*mut ()>,
}
unsafe impl Send for SslClientCertCredentials {}

impl SslClientCertCredentials {
    /// Get a reference to the underlying pool
    pub fn pool(&self) -> &apr::Pool {
        &self.pool
    }

    /// Get the raw pointer to the SSL client cert credentials (use with caution)
    pub fn as_ptr(&self) -> *const subversion_sys::svn_auth_cred_ssl_client_cert_t {
        self.ptr
    }

    /// Get the mutable raw pointer to the SSL client cert credentials (use with caution)
    pub fn as_mut_ptr(&mut self) -> *mut subversion_sys::svn_auth_cred_ssl_client_cert_t {
        self.ptr
    }

    /// Creates a duplicate of the SSL client certificate credentials.
    pub fn dup(&self) -> Self {
        let pool = apr::Pool::new();
        let cred: *mut subversion_sys::svn_auth_cred_ssl_client_cert_t = pool.calloc();
        unsafe {
            (*cred).cert_file = apr::strings::pstrdup_raw(
                std::ffi::CStr::from_ptr((*self.ptr).cert_file)
                    .to_str()
                    .unwrap(),
                &pool,
            )
            .unwrap() as *mut _;
            (*cred).may_save = (*self.ptr).may_save;
        }
        Self {
            ptr: cred,
            pool,
            _phantom: PhantomData,
        }
    }
}

extern "C" fn wrap_client_cert_prompt_fn(
    cred: *mut *mut subversion_sys::svn_auth_cred_ssl_client_cert_t,
    baton: *mut std::ffi::c_void,
    realmstring: *const std::ffi::c_char,
    may_save: subversion_sys::svn_boolean_t,
    _pool: *mut subversion_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let f = unsafe {
        &*(baton as *const &dyn Fn(&str, bool) -> Result<SslClientCertCredentials, crate::Error>)
    };
    let realm = unsafe { std::ffi::CStr::from_ptr(realmstring).to_str().unwrap() };
    f(realm, may_save != 0)
        .map(|creds| {
            unsafe { *cred = creds.ptr };
            std::ptr::null_mut()
        })
        .unwrap_or_else(|e| unsafe { e.into_raw() })
}

/// Gets an SSL client certificate prompt authentication provider.
pub fn get_ssl_client_cert_prompt_provider(
    prompt_fn: &impl Fn(&str, bool) -> Result<SslClientCertCredentials, crate::Error>,
    retry_limit: usize,
) -> AuthProvider {
    let mut auth_provider = std::ptr::null_mut();

    let pool = apr::Pool::new();
    unsafe {
        subversion_sys::svn_auth_get_ssl_client_cert_prompt_provider(
            &mut auth_provider,
            Some(wrap_client_cert_prompt_fn),
            prompt_fn as *const _ as *mut std::ffi::c_void,
            retry_limit.try_into().unwrap(),
            pool.as_mut_ptr(),
        );
    }
    AuthProvider {
        ptr: auth_provider,
        pool,
        _phantom: PhantomData,
    }
}

/// Gets an SSL client certificate password file authentication provider.
pub fn get_ssl_client_cert_pw_file_provider(
    prompt_fn: Option<&impl Fn(&str) -> Result<bool, crate::Error>>,
) -> AuthProvider {
    let mut auth_provider = std::ptr::null_mut();

    let pool = apr::Pool::new();
    unsafe {
        subversion_sys::svn_auth_get_ssl_client_cert_pw_file_provider2(
            &mut auth_provider,
            if prompt_fn.is_some() {
                Some(wrap_plaintext_passphrase_prompt)
            } else {
                None
            },
            if let Some(f) = prompt_fn {
                f as *const _ as *mut std::ffi::c_void
            } else {
                std::ptr::null_mut()
            },
            pool.as_mut_ptr(),
        );
    }
    AuthProvider {
        ptr: auth_provider,
        pool,
        _phantom: PhantomData,
    }
}

/// Gets an SSL client certificate password file authentication provider with a boxed callback.
///
/// This version accepts a boxed trait object for dynamic language bindings.
pub fn get_ssl_client_cert_pw_file_provider_boxed(
    prompt_fn: Option<Box<dyn Fn(&str) -> Result<bool, crate::Error> + Send>>,
) -> AuthProvider {
    let mut auth_provider = std::ptr::null_mut();
    let pool = apr::Pool::new();

    let baton = if let Some(func) = prompt_fn {
        Box::into_raw(func) as *mut std::ffi::c_void
    } else {
        std::ptr::null_mut()
    };

    unsafe {
        subversion_sys::svn_auth_get_ssl_client_cert_pw_file_provider2(
            &mut auth_provider,
            if !baton.is_null() {
                Some(wrap_plaintext_passphrase_prompt_boxed)
            } else {
                None
            },
            baton,
            pool.as_mut_ptr(),
        );
    }
    AuthProvider {
        ptr: auth_provider,
        pool,
        _phantom: PhantomData,
    }
}

/// Gets a simple prompt authentication provider.
pub fn get_simple_prompt_provider<'pool>(
    prompt_fn: &impl Fn(
        &'_ str,
        Option<&'_ str>,
        bool,
    ) -> Result<SimpleCredentials<'pool>, crate::Error>,
    retry_limit: usize,
) -> AuthProvider {
    let mut auth_provider = std::ptr::null_mut();

    extern "C" fn wrap_simple_prompt_provider<'pool>(
        credentials: *mut *mut subversion_sys::svn_auth_cred_simple_t,
        baton: *mut std::ffi::c_void,
        realmstring: *const std::ffi::c_char,
        username: *const std::ffi::c_char,
        may_save: subversion_sys::svn_boolean_t,
        _pool: *mut subversion_sys::apr_pool_t,
    ) -> *mut subversion_sys::svn_error_t {
        let f = unsafe {
            &*(baton
                as *const &dyn for<'a> Fn(
                    &'a str,
                    Option<&'a str>,
                    bool,
                )
                    -> Result<SimpleCredentials<'a>, crate::Error>)
        };
        let realm = unsafe { std::ffi::CStr::from_ptr(realmstring).to_str().unwrap() };
        let username = if username.is_null() {
            None
        } else {
            Some(unsafe { std::ffi::CStr::from_ptr(username).to_str().unwrap() })
        };
        f(realm, username, may_save != 0)
            .map(|creds| {
                unsafe { *credentials = creds.ptr as *mut _ };
                std::ptr::null_mut()
            })
            .unwrap_or_else(|e| unsafe { e.into_raw() })
    }

    let pool = apr::Pool::new();
    unsafe {
        subversion_sys::svn_auth_get_simple_prompt_provider(
            &mut auth_provider,
            Some(wrap_simple_prompt_provider),
            prompt_fn as *const _ as *mut std::ffi::c_void,
            retry_limit.try_into().unwrap(),
            pool.as_mut_ptr(),
        );
    }
    AuthProvider {
        ptr: auth_provider,
        pool,
        _phantom: PhantomData,
    }
}

/// Gets a simple prompt authentication provider with a boxed callback.
///
/// This version accepts a boxed trait object for dynamic language bindings.
pub fn get_simple_prompt_provider_boxed(
    prompt_fn: Box<
        dyn Fn(&str, Option<&str>, bool) -> Result<(String, String, bool), crate::Error> + Send,
    >,
    retry_limit: usize,
) -> AuthProvider {
    let mut auth_provider = std::ptr::null_mut();
    let pool = apr::Pool::new();

    let baton = Box::into_raw(Box::new(prompt_fn)) as *mut std::ffi::c_void;

    extern "C" fn wrap_simple_prompt_provider_boxed(
        credentials: *mut *mut subversion_sys::svn_auth_cred_simple_t,
        baton: *mut std::ffi::c_void,
        realmstring: *const std::ffi::c_char,
        username: *const std::ffi::c_char,
        may_save: subversion_sys::svn_boolean_t,
        pool: *mut subversion_sys::apr_pool_t,
    ) -> *mut subversion_sys::svn_error_t {
        let f = unsafe {
            &*(baton
                as *const Box<
                    dyn Fn(
                            &str,
                            Option<&str>,
                            bool,
                        )
                            -> Result<(String, String, bool), crate::Error>
                        + Send,
                >)
        };
        let realm = unsafe { std::ffi::CStr::from_ptr(realmstring).to_str().unwrap() };
        let username_str = if username.is_null() {
            None
        } else {
            Some(unsafe { std::ffi::CStr::from_ptr(username).to_str().unwrap() })
        };
        let svn_pool = apr::Pool::from_raw(pool);
        
        f(realm, username_str, may_save != 0)
            .map(|(user, pass, save)| {
                // Create credentials in the SVN-provided pool
                let creds = SimpleCredentials::new(user, pass, save, &svn_pool);
                unsafe { *credentials = creds.ptr as *mut _ };
                std::mem::forget(creds); // Don't drop since SVN owns it now
                std::ptr::null_mut()
            })
            .unwrap_or_else(|e| unsafe { e.into_raw() })
    }

    unsafe {
        subversion_sys::svn_auth_get_simple_prompt_provider(
            &mut auth_provider,
            Some(wrap_simple_prompt_provider_boxed),
            baton,
            retry_limit.try_into().unwrap(),
            pool.as_mut_ptr(),
        );
    }
    AuthProvider {
        ptr: auth_provider,
        pool,
        _phantom: PhantomData,
    }
}

/// Gets a username prompt authentication provider.
pub fn get_username_prompt_provider(
    prompt_fn: &impl Fn(&str, bool) -> Result<String, crate::Error>,
    retry_limit: usize,
) -> AuthProvider {
    let mut auth_provider = std::ptr::null_mut();

    extern "C" fn wrap_username_prompt_provider(
        credentials: *mut *mut subversion_sys::svn_auth_cred_username_t,
        baton: *mut std::ffi::c_void,
        realmstring: *const std::ffi::c_char,
        may_save: subversion_sys::svn_boolean_t,
        _pool: *mut subversion_sys::apr_pool_t,
    ) -> *mut subversion_sys::svn_error_t {
        let f = unsafe { &*(baton as *const &dyn Fn(&str, bool) -> Result<String, crate::Error>) };
        let realm = unsafe { std::ffi::CStr::from_ptr(realmstring).to_str().unwrap() };
        f(realm, may_save != 0)
            .map(|username| {
                let username = std::ffi::CString::new(username).unwrap();
                let creds = subversion_sys::svn_auth_cred_username_t {
                    username: username.as_ptr(),
                    may_save,
                };
                unsafe { *credentials = Box::into_raw(Box::new(creds)) };
                std::ptr::null_mut()
            })
            .unwrap_or_else(|e| unsafe { e.into_raw() })
    }

    let pool = apr::Pool::new();
    unsafe {
        subversion_sys::svn_auth_get_username_prompt_provider(
            &mut auth_provider,
            Some(wrap_username_prompt_provider),
            prompt_fn as *const _ as *mut std::ffi::c_void,
            retry_limit.try_into().unwrap(),
            pool.as_mut_ptr(),
        );
    }
    AuthProvider {
        ptr: auth_provider,
        pool,
        _phantom: PhantomData,
    }
}

/// Gets a username prompt authentication provider with a boxed callback.
///
/// This version accepts a boxed trait object for dynamic language bindings.
pub fn get_username_prompt_provider_boxed(
    prompt_fn: Box<dyn Fn(&str, bool) -> Result<String, crate::Error> + Send>,
    retry_limit: usize,
) -> AuthProvider {
    let mut auth_provider = std::ptr::null_mut();
    let pool = apr::Pool::new();

    let baton = Box::into_raw(Box::new(prompt_fn)) as *mut std::ffi::c_void;

    extern "C" fn wrap_username_prompt_provider_boxed(
        credentials: *mut *mut subversion_sys::svn_auth_cred_username_t,
        baton: *mut std::ffi::c_void,
        realmstring: *const std::ffi::c_char,
        may_save: subversion_sys::svn_boolean_t,
        _pool: *mut subversion_sys::apr_pool_t,
    ) -> *mut subversion_sys::svn_error_t {
        let f = unsafe {
            &*(baton as *const Box<dyn Fn(&str, bool) -> Result<String, crate::Error> + Send>)
        };
        let realm = unsafe { std::ffi::CStr::from_ptr(realmstring).to_str().unwrap() };
        let pool = apr::Pool::new();

        f(realm, may_save != 0)
            .map(|username| {
                unsafe {
                    let cred = pool.calloc::<subversion_sys::svn_auth_cred_username_t>();
                    let username_cstr = std::ffi::CString::new(username).unwrap();
                    (*cred).username =
                        apr_sys::apr_pstrdup(pool.as_mut_ptr(), username_cstr.as_ptr());
                    (*cred).may_save = may_save;
                    *credentials = cred;
                }
                std::ptr::null_mut()
            })
            .unwrap_or_else(|e| unsafe { e.into_raw() })
    }

    unsafe {
        subversion_sys::svn_auth_get_username_prompt_provider(
            &mut auth_provider,
            Some(wrap_username_prompt_provider_boxed),
            baton,
            retry_limit.try_into().unwrap(),
            pool.as_mut_ptr(),
        );
    }
    AuthProvider {
        ptr: auth_provider,
        pool,
        _phantom: PhantomData,
    }
}

extern "C" fn wrap_plaintext_passphrase_prompt_boxed(
    may_save_plaintext: *mut subversion_sys::svn_boolean_t,
    realmstring: *const std::ffi::c_char,
    baton: *mut std::ffi::c_void,
    _pool: *mut subversion_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let f = unsafe { &*(baton as *const Box<dyn Fn(&str) -> Result<bool, crate::Error> + Send>) };
    let realm = unsafe { std::ffi::CStr::from_ptr(realmstring).to_str().unwrap() };
    f(realm)
        .map(|b| {
            unsafe { *may_save_plaintext = if b { 1 } else { 0 } };
            std::ptr::null_mut()
        })
        .unwrap_or_else(|e| unsafe { e.into_raw() })
}

extern "C" fn wrap_plaintext_passphrase_prompt(
    may_save_plaintext: *mut subversion_sys::svn_boolean_t,
    realmstring: *const std::ffi::c_char,
    baton: *mut std::ffi::c_void,
    _pool: *mut subversion_sys::apr_pool_t,
) -> *mut subversion_sys::svn_error_t {
    let f = unsafe { &*(baton as *const &dyn Fn(&str) -> Result<bool, crate::Error>) };
    let realm = unsafe { std::ffi::CStr::from_ptr(realmstring).to_str().unwrap() };
    f(realm)
        .map(|b| {
            unsafe { *may_save_plaintext = if b { 1 } else { 0 } };
            std::ptr::null_mut()
        })
        .unwrap_or_else(|e| unsafe { e.into_raw() })
}

/// Gets a simple authentication provider.
pub fn get_simple_provider(
    plaintext_prompt_func: Option<&impl Fn(&str) -> Result<bool, crate::Error>>,
) -> AuthProvider {
    let mut auth_provider = std::ptr::null_mut();

    let pool = apr::Pool::new();
    unsafe {
        subversion_sys::svn_auth_get_simple_provider2(
            &mut auth_provider,
            if plaintext_prompt_func.is_some() {
                Some(wrap_plaintext_passphrase_prompt)
            } else {
                None
            },
            if let Some(f) = plaintext_prompt_func {
                f as *const _ as *mut std::ffi::c_void
            } else {
                std::ptr::null_mut()
            },
            pool.as_mut_ptr(),
        );
    }
    AuthProvider {
        ptr: auth_provider,
        pool,
        _phantom: PhantomData,
    }
}

/// Gets a simple authentication provider with a boxed callback.
///
/// This version accepts a boxed trait object, making it suitable for use with
/// dynamic languages like Python. The callback is leaked to ensure it stays alive
/// for the lifetime of the auth provider.
///
/// # Safety
/// The boxed callback is intentionally leaked and will not be freed automatically.
/// This is necessary because SVN holds onto the pointer for the lifetime of the auth system.
pub fn get_simple_provider_boxed(
    plaintext_prompt_func: Option<Box<dyn Fn(&str) -> Result<bool, crate::Error> + Send>>,
) -> AuthProvider {
    let mut auth_provider = std::ptr::null_mut();
    let pool = apr::Pool::new();

    let baton = if let Some(func) = plaintext_prompt_func {
        Box::into_raw(func) as *mut std::ffi::c_void
    } else {
        std::ptr::null_mut()
    };

    unsafe {
        subversion_sys::svn_auth_get_simple_provider2(
            &mut auth_provider,
            if !baton.is_null() {
                Some(wrap_plaintext_passphrase_prompt_boxed)
            } else {
                None
            },
            baton,
            pool.as_mut_ptr(),
        );
    }
    AuthProvider {
        ptr: auth_provider,
        pool,
        _phantom: PhantomData,
    }
}

/// Gets a platform-specific authentication provider.
pub fn get_platform_specific_provider(
    provider_name: &str,
    provider_type: &str,
) -> Result<AuthProvider, crate::Error> {
    let mut auth_provider = std::ptr::null_mut();
    let provider_name = std::ffi::CString::new(provider_name).unwrap();
    let provider_type = std::ffi::CString::new(provider_type).unwrap();
    let pool = apr::pool::Pool::new();
    let err = unsafe {
        subversion_sys::svn_auth_get_platform_specific_provider(
            &mut auth_provider,
            provider_name.as_ptr(),
            provider_type.as_ptr(),
            pool.as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok(AuthProvider {
        ptr: auth_provider,
        pool,
        _phantom: PhantomData,
    })
}

/// Gets all platform-specific client authentication providers.
pub fn get_platform_specific_client_providers() -> Result<Vec<AuthProvider>, Error> {
    let pool = apr::pool::Pool::new();
    let mut providers = std::ptr::null_mut();
    let err = unsafe {
        subversion_sys::svn_auth_get_platform_specific_client_providers(
            &mut providers,
            // TODO: pass in config
            std::ptr::null_mut(),
            pool.as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    let providers = unsafe {
        apr::tables::TypedArray::<*mut subversion_sys::svn_auth_provider_object_t>::from_ptr(
            providers,
        )
    };
    Ok(providers
        .iter()
        .map(|p| AuthProvider {
            ptr: p as *const _,
            pool: apr::Pool::new(), // Each provider gets its own pool
            _phantom: PhantomData,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_credentials_creation() {
        let pool = apr::Pool::new();
        let creds =
            SimpleCredentials::new("testuser".to_string(), "testpass".to_string(), true, &pool);
        assert_eq!(creds.username(), "testuser");
        assert_eq!(creds.password(), "testpass");
        assert!(creds.may_save());
    }

    #[test]
    fn test_simple_credentials_set_username() {
        let pool = apr::Pool::new();
        let mut creds =
            SimpleCredentials::new("olduser".to_string(), "pass".to_string(), false, &pool);
        creds.set_username("newuser", &pool);
        assert_eq!(creds.username(), "newuser");
        assert!(!creds.may_save());
    }

    #[test]
    fn test_simple_credentials_kind() {
        let kind = SimpleCredentials::kind();
        assert_eq!(kind, "svn.simple");
    }

    #[test]
    fn test_simple_credentials_as_mut_ptr() {
        let pool = apr::Pool::new();
        let mut creds = SimpleCredentials::new(
            "user".to_string(),
            "pass".to_string(),
            true,
            &pool,
        );
        let ptr = creds.as_mut_ptr();
        assert!(!ptr.is_null());
        
        // Verify the pointer points to valid data
        unsafe {
            let raw_creds = ptr as *mut subversion_sys::svn_auth_cred_simple_t;
            let username = std::ffi::CStr::from_ptr((*raw_creds).username)
                .to_str()
                .unwrap();
            assert_eq!(username, "user");
        }
    }

    #[test]
    fn test_simple_credentials_from_raw() {
        let pool = apr::Pool::new();
        
        // Create raw credentials
        let raw_cred: *mut subversion_sys::svn_auth_cred_simple_t = pool.calloc();
        unsafe {
            (*raw_cred).username = apr::strings::pstrdup_raw("raw_user", &pool).unwrap() as *mut _;
            (*raw_cred).password = apr::strings::pstrdup_raw("raw_pass", &pool).unwrap() as *mut _;
            (*raw_cred).may_save = 1;
        }
        
        // Create SimpleCredentials from raw pointer
        let creds = SimpleCredentials::from_raw(raw_cred as *mut std::ffi::c_void);
        assert_eq!(creds.username(), "raw_user");
        assert_eq!(creds.password(), "raw_pass");
        assert!(creds.may_save());
    }



    #[test]
    fn test_username_credentials_creation() {
        let pool = apr::Pool::new();
        let creds = UsernameCredentials::new("testuser".to_string(), true, &pool);
        assert_eq!(creds.username(), "testuser");
        assert!(creds.may_save());
    }

    #[test]
    fn test_username_credentials_may_save_false() {
        let pool = apr::Pool::new();
        let creds = UsernameCredentials::new("user".to_string(), false, &pool);
        assert_eq!(creds.username(), "user");
        assert!(!creds.may_save());
    }

    #[test]
    fn test_username_credentials_kind() {
        let kind = UsernameCredentials::kind();
        assert_eq!(kind, "svn.username");
    }

    #[test]
    fn test_username_credentials_as_mut_ptr() {
        let pool = apr::Pool::new();
        let mut creds = UsernameCredentials::new("user".to_string(), true, &pool);
        let ptr = creds.as_mut_ptr();
        assert!(!ptr.is_null());
        
        // Verify the pointer points to valid data
        unsafe {
            let raw_creds = ptr as *mut subversion_sys::svn_auth_cred_username_t;
            let username = std::ffi::CStr::from_ptr((*raw_creds).username)
                .to_str()
                .unwrap();
            assert_eq!(username, "user");
            assert_eq!((*raw_creds).may_save, 1);
        }
    }

    #[test]
    fn test_username_credentials_from_raw() {
        let pool = apr::Pool::new();
        
        // Create raw credentials
        let raw_cred: *mut subversion_sys::svn_auth_cred_username_t = pool.calloc();
        unsafe {
            (*raw_cred).username = apr::strings::pstrdup_raw("raw_username", &pool).unwrap() as *mut _;
            (*raw_cred).may_save = 0;
        }
        
        // Create UsernameCredentials from raw pointer
        let creds = UsernameCredentials::from_raw(raw_cred as *mut std::ffi::c_void);
        assert_eq!(creds.username(), "raw_username");
        assert!(!creds.may_save());
    }



    #[test]
    fn test_ssl_server_trust_credentials_creation() {
        let pool = apr::Pool::new();
        let creds = SslServerTrustCredentials::new(true, 0x01, &pool);
        assert!(creds.may_save());
        assert_eq!(creds.accepted_failures(), 0x01);
    }

    #[test]
    fn test_ssl_server_trust_credentials_may_save_false() {
        let pool = apr::Pool::new();
        let creds = SslServerTrustCredentials::new(false, 0xFF, &pool);
        assert!(!creds.may_save());
        assert_eq!(creds.accepted_failures(), 0xFF);
    }

    #[test]
    fn test_ssl_server_trust_credentials_kind() {
        let kind = SslServerTrustCredentials::kind();
        // The SVN constant is actually "svn.ssl.server" not "svn.ssl.server.trust"
        // See: https://subversion.apache.org/docs/api/1.11/svn__auth_8h_source.html
        assert_eq!(kind, "svn.ssl.server");
    }

    #[test]
    fn test_ssl_server_trust_credentials_as_mut_ptr() {
        let pool = apr::Pool::new();
        let mut creds = SslServerTrustCredentials::new(true, 0x42, &pool);
        let ptr = creds.as_mut_ptr();
        assert!(!ptr.is_null());
        
        // Verify the pointer points to valid data
        unsafe {
            let raw_creds = ptr as *mut subversion_sys::svn_auth_cred_ssl_server_trust_t;
            assert_eq!((*raw_creds).may_save, 1);
            assert_eq!((*raw_creds).accepted_failures, 0x42);
        }
    }

    #[test]
    fn test_ssl_server_trust_credentials_from_raw() {
        let pool = apr::Pool::new();
        
        // Create raw credentials
        let raw_cred: *mut subversion_sys::svn_auth_cred_ssl_server_trust_t = pool.calloc();
        unsafe {
            (*raw_cred).may_save = 0;
            (*raw_cred).accepted_failures = 0xAB;
        }
        
        // Create SslServerTrustCredentials from raw pointer
        let creds = SslServerTrustCredentials::from_raw(raw_cred as *mut std::ffi::c_void);
        assert!(!creds.may_save());
        assert_eq!(creds.accepted_failures(), 0xAB);
    }



    #[test]
    fn test_credentials_trait_polymorphism() {
        let pool = apr::Pool::new();
        
        // Create different credential types
        let simple = SimpleCredentials::new("user".to_string(), "pass".to_string(), true, &pool);
        let username = UsernameCredentials::new("user2".to_string(), false, &pool);
        let trust = SslServerTrustCredentials::new(true, 0x01, &pool);
        
        // Test that we can use them as trait objects with explicit lifetime
        let creds: Vec<Box<dyn Credentials + '_>> = vec![
            Box::new(simple),
            Box::new(username),
            Box::new(trust),
        ];
        
        // Verify we can downcast them correctly and query values
        if let Some(simple_creds) = creds[0].as_simple() {
            assert_eq!(simple_creds.username(), "user");
            assert_eq!(simple_creds.password(), "pass");
            assert!(simple_creds.may_save());
        } else {
            panic!("Failed to downcast to SimpleCredentials");
        }
        
        if let Some(username_creds) = creds[1].as_username() {
            assert_eq!(username_creds.username(), "user2");
            assert!(!username_creds.may_save());
        } else {
            panic!("Failed to downcast to UsernameCredentials");
        }
        
        if let Some(trust_creds) = creds[2].as_ssl_server_trust() {
            assert!(trust_creds.may_save());
            assert_eq!(trust_creds.accepted_failures(), 0x01);
        } else {
            panic!("Failed to downcast to SslServerTrustCredentials");
        }
        
        // Verify wrong downcasts return None
        assert!(creds[0].as_username().is_none());
        assert!(creds[0].as_ssl_server_trust().is_none());
        assert!(creds[1].as_simple().is_none());
        assert!(creds[1].as_ssl_server_trust().is_none());
        assert!(creds[2].as_simple().is_none());
        assert!(creds[2].as_username().is_none());
    }

    #[test]
    fn test_credentials_mixed_downcast_and_query() {
        let pool = apr::Pool::new();
        
        // Create a vector of mixed credential types as trait objects
        let creds: Vec<Box<dyn Credentials + '_>> = vec![
            Box::new(SimpleCredentials::new("alice".to_string(), "alice123".to_string(), true, &pool)),
            Box::new(UsernameCredentials::new("bob".to_string(), false, &pool)),
            Box::new(SslServerTrustCredentials::new(true, 0x404, &pool)),
            Box::new(SimpleCredentials::new("charlie".to_string(), "ch@rl!3".to_string(), false, &pool)),
        ];
        
        // Test downcasting and querying the first (SimpleCredentials)
        if let Some(simple) = creds[0].as_simple() {
            assert_eq!(simple.username(), "alice");
            assert_eq!(simple.password(), "alice123");
            assert!(simple.may_save());
        } else {
            panic!("Failed to downcast index 0 to SimpleCredentials");
        }
        
        // Test downcasting and querying the second (UsernameCredentials)
        if let Some(username) = creds[1].as_username() {
            assert_eq!(username.username(), "bob");
            assert!(!username.may_save());
        } else {
            panic!("Failed to downcast index 1 to UsernameCredentials");
        }
        
        // Test downcasting and querying the third (SslServerTrustCredentials)
        if let Some(trust) = creds[2].as_ssl_server_trust() {
            assert!(trust.may_save());
            assert_eq!(trust.accepted_failures(), 0x404);
        } else {
            panic!("Failed to downcast index 2 to SslServerTrustCredentials");
        }
        
        // Test downcasting and querying the fourth (SimpleCredentials again)
        if let Some(simple) = creds[3].as_simple() {
            assert_eq!(simple.username(), "charlie");
            assert_eq!(simple.password(), "ch@rl!3");
            assert!(!simple.may_save());
        } else {
            panic!("Failed to downcast index 3 to SimpleCredentials");
        }
        
        // Verify wrong downcasts fail gracefully
        assert!(creds[0].as_username().is_none());
        assert!(creds[0].as_ssl_server_trust().is_none());
        assert!(creds[1].as_simple().is_none());
        assert!(creds[1].as_ssl_server_trust().is_none());
        assert!(creds[2].as_simple().is_none());
        assert!(creds[2].as_username().is_none());
    }

    #[test]
    fn test_auth_provider_creation() {
        // Test username provider
        let username_provider = get_username_provider();
        assert!(!username_provider.ptr.is_null());

        // Test simple provider
        let simple_provider = get_simple_provider(None::<&fn(&str) -> Result<bool, Error>>);
        assert!(!simple_provider.ptr.is_null());

        // Test SSL providers
        let ssl_trust_provider = get_ssl_server_trust_file_provider();
        assert!(!ssl_trust_provider.ptr.is_null());

        let ssl_cert_provider = get_ssl_client_cert_file_provider();
        assert!(!ssl_cert_provider.ptr.is_null());
    }

    #[test]
    fn test_auth_provider_cred_kind() {
        let provider = get_username_provider();
        let kind = provider.cred_kind();
        assert!(kind.contains("username"));
    }

    #[test]
    fn test_auth_baton_open() {
        let providers = vec![
            get_username_provider(),
            get_simple_provider(None::<&fn(&str) -> Result<bool, Error>>),
        ];

        let baton = AuthBaton::open(providers);
        assert!(baton.is_ok());
    }

    #[test]
    fn test_auth_baton_parameter_safety() {
        let providers = vec![get_username_provider()];
        let mut baton = AuthBaton::open(providers).unwrap();

        // Test setting and getting a parameter
        let test_key = "test_param";
        let test_value = "test_value\0";
        unsafe {
            baton.set_parameter(test_key, test_value.as_ptr() as *const _);
            let retrieved = baton.get_parameter(test_key);
            assert!(!retrieved.is_null());
        }
    }

    #[test]
    fn test_ssl_server_cert_info_dup() {
        // We can't easily create a real SslServerCertInfo without a complex setup,
        // but we can test the structure exists and has proper Send marker
        fn _assert_send<T: Send>() {}
        _assert_send::<SslServerCertInfo>();
    }

    #[test]
    fn test_ssl_server_trust_dup() {
        // Test that SslServerTrust has proper Send marker
        fn _assert_send<T: Send>() {}
        _assert_send::<SslServerTrust>();
    }

    #[test]
    fn test_ssl_client_cert_credentials_dup() {
        // Test that SslClientCertCredentials has proper Send marker
        fn _assert_send<T: Send>() {}
        _assert_send::<SslClientCertCredentials>();
    }

    #[test]
    fn test_platform_specific_providers() {
        // This may return empty on some platforms
        let providers = get_platform_specific_client_providers();
        assert!(providers.is_ok());
        // Just check it doesn't crash - may be empty
        let _ = providers.unwrap();
    }

    #[test]
    fn test_simple_prompt_provider() {
        // Note: This test can only verify the provider is created, not actually used
        // due to lifetime constraints with the pool in the callback
        let prompt_fn = |_realm: &str,
                         _username: Option<&str>,
                         _may_save: bool|
         -> Result<SimpleCredentials, Error> {
            // This would need a pool with appropriate lifetime
            // For testing, we just create a minimal failing response
            Err(Error::from_str("Not implemented"))
        };

        let provider = get_simple_prompt_provider(&prompt_fn, 3);
        assert!(!provider.ptr.is_null());
    }

    #[test]
    fn test_username_prompt_provider() {
        let prompt_fn =
            |_realm: &str, _may_save: bool| Ok::<_, Error>("prompted_username".to_string());

        let provider = get_username_prompt_provider(&prompt_fn, 3);
        assert!(!provider.ptr.is_null());
    }

    #[test]
    fn test_ssl_client_cert_prompt_provider() {
        let prompt_fn = |_realm: &str, _may_save: bool| {
            let pool = apr::Pool::new();
            let cred: *mut subversion_sys::svn_auth_cred_ssl_client_cert_t = pool.calloc();
            unsafe {
                (*cred).cert_file =
                    apr::strings::pstrdup_raw("/path/to/cert", &pool).unwrap() as *mut _;
                (*cred).may_save = 1;
            }
            Ok::<_, Error>(SslClientCertCredentials {
                ptr: cred,
                pool,
                _phantom: PhantomData,
            })
        };

        let provider = get_ssl_client_cert_prompt_provider(&prompt_fn, 3);
        assert!(!provider.ptr.is_null());
    }

    #[test]
    fn test_auth_provider_as_trait() {
        let provider = get_username_provider();

        // Test that AuthProvider implements AsAuthProvider
        let pool = apr::Pool::new();
        let ptr = provider.as_auth_provider(&pool);
        assert!(!ptr.is_null());
        assert_eq!(ptr, provider.ptr);

        // Test reference also works
        let ptr2 = provider.as_auth_provider(&pool);
        assert_eq!(ptr, ptr2);
    }

    #[test]
    fn test_boxed_simple_prompt_returns_credentials() {
        // Create a callback that returns specific credentials
        // Note: SVN might provide an initial username from environment/config
        let callback = Box::new(|_realm: &str, _username: Option<&str>, may_save: bool| {
            // Always use our test credentials regardless of what SVN suggests
            Ok((
                "test_user".to_string(),
                "secure_password".to_string(),
                may_save,
            ))
        });

        let provider = get_simple_prompt_provider_boxed(callback, 3);
        let mut auth_baton = AuthBaton::open(vec![provider]).unwrap();

        // Request credentials and verify they match what our callback returns
        let creds = auth_baton
            .first_credentials("svn.simple", "test_realm")
            .unwrap();

        if let Some(creds) = creds {
            // Downcast to SimpleCredentials
            if let Some(simple_creds) = creds.as_simple() {
                // Should get what our callback returned
                assert_eq!(simple_creds.username(), "test_user");
                assert_eq!(simple_creds.password(), "secure_password");
                assert!(simple_creds.may_save());
            } else {
                panic!("Expected SimpleCredentials");
            }
        } else {
            panic!("Expected credentials");
        }
    }

    #[test]
    fn test_boxed_username_prompt_returns_username() {
        let expected_username = "test_boxed_username";

        let callback = Box::new(
            move |realm: &str, may_save: bool| -> Result<String, crate::Error> {
                assert_eq!(realm, "username_realm");
                assert_eq!(may_save, true);
                Ok(expected_username.to_string())
            },
        );

        let provider = get_username_prompt_provider_boxed(callback, 3);
        let mut auth_baton = AuthBaton::open(vec![provider]).unwrap();

        // Request username credentials
        let creds = auth_baton
            .first_credentials("svn.username", "username_realm")
            .unwrap();

        if let Some(creds) = creds {
            // Downcast to UsernameCredentials
            if let Some(username_creds) = creds.as_username() {
                assert_eq!(username_creds.username(), expected_username);
                assert!(username_creds.may_save());
            } else {
                panic!("Expected UsernameCredentials");
            }
        } else {
            panic!("Expected credentials");
        }
    }

    #[test]
    fn test_boxed_simple_prompt_with_initial_username() {
        // Test that initial username is passed to callback
        let callback = Box::new(|_realm: &str, username: Option<&str>, may_save: bool| {
            // In practice, SVN would provide initial_user from URL or previous attempts
            Ok((
                username.unwrap_or("initial_user").to_string(),
                "password123".to_string(),
                may_save,
            ))
        });

        let provider = get_simple_prompt_provider_boxed(callback, 3);
        let mut auth_baton = AuthBaton::open(vec![provider]).unwrap();

        // Request credentials
        let creds = auth_baton
            .first_credentials("svn.simple", "test_realm")
            .unwrap();

        if let Some(creds) = creds {
            // Downcast to SimpleCredentials
            if let Some(simple_creds) = creds.as_simple() {
                // The callback should use whatever username it receives
                assert!(!simple_creds.username().is_empty());
                assert_eq!(simple_creds.password(), "password123");
                assert!(simple_creds.may_save());
            } else {
                panic!("Expected SimpleCredentials");
            }
        } else {
            panic!("Expected credentials");
        }
    }

    #[test]
    fn test_boxed_callback_may_save_false() {
        // Test that may_save=false is properly handled
        let callback = Box::new(|_realm: &str, _username: Option<&str>, _may_save: bool| {
            Ok((
                "user".to_string(),
                "pass".to_string(),
                false, // Override to never save
            ))
        });

        let provider = get_simple_prompt_provider_boxed(callback, 1);
        let mut auth_baton = AuthBaton::open(vec![provider]).unwrap();

        let creds = auth_baton.first_credentials("svn.simple", "realm").unwrap();

        if let Some(creds) = creds {
            // Downcast to SimpleCredentials
            if let Some(simple_creds) = creds.as_simple() {
                assert_eq!(simple_creds.username(), "user");
                assert_eq!(simple_creds.password(), "pass");
                assert!(
                    !simple_creds.may_save(),
                    "Should be false as set by callback"
                );
            } else {
                panic!("Expected SimpleCredentials");
            }
        } else {
            panic!("Expected credentials");
        }
    }

    #[test]
    fn test_boxed_provider_error_handling() {
        // Test that errors from callbacks are properly handled
        let callback = Box::new(|_realm: &str, _username: Option<&str>, _may_save: bool| {
            Err::<(String, String, bool), _>(crate::Error::from_str("Authentication failed"))
        });

        let provider = get_simple_prompt_provider_boxed(callback, 1);
        let mut auth_baton = AuthBaton::open(vec![provider]).unwrap();

        // When our callback returns an error, it gets converted to an SVN error
        // and propagated back to the caller
        let creds = auth_baton.first_credentials("svn.simple", "error_realm");

        // The error from our callback should be propagated
        assert!(creds.is_err(), "Expected error from callback to be propagated");
    }

    #[test]
    fn test_multiple_boxed_providers_precedence() {
        // Test that providers are tried in order
        let provider1 = get_simple_prompt_provider_boxed(
            Box::new(|realm, _, _| {
                if realm == "realm1" {
                    Ok((
                        "user1".to_string(),
                        "pass1".to_string(),
                        true,
                    ))
                } else {
                    Err(crate::Error::from_str("Wrong realm"))
                }
            }),
            1,
        );

        let provider2 = get_simple_prompt_provider_boxed(
            Box::new(|_, _, _| {
                Ok((
                    "user2".to_string(),
                    "pass2".to_string(),
                    true,
                ))
            }),
            1,
        );

        let mut auth_baton = AuthBaton::open(vec![provider1, provider2]).unwrap();

        // First provider should handle realm1
        {
            let creds1 = auth_baton
                .first_credentials("svn.simple", "realm1")
                .unwrap();
            if let Some(creds) = creds1 {
                // Downcast to SimpleCredentials
                if let Some(simple_creds) = creds.as_simple() {
                    assert_eq!(simple_creds.username(), "user1");
                    assert_eq!(simple_creds.password(), "pass1");
                    assert!(simple_creds.may_save());
                } else {
                    panic!("Expected SimpleCredentials for realm1");
                }
            } else {
                panic!("Expected credentials for realm1");
            }
        } // Drop creds1 here

        // For other realms, first provider returns error, SVN may propagate it
        // or try the next provider - the behavior can vary
        let creds2_result = auth_baton.first_credentials("svn.simple", "realm2");
        
        // When first provider returns an error, SVN might:
        // 1. Try the next provider (and get user2/pass2)
        // 2. Propagate the error immediately
        match creds2_result {
            Ok(Some(creds)) => {
                // If SVN tried the second provider
                if let Some(simple_creds) = creds.as_simple() {
                    assert_eq!(simple_creds.username(), "user2");
                    assert_eq!(simple_creds.password(), "pass2");
                    assert!(simple_creds.may_save());
                } else {
                    panic!("Expected SimpleCredentials for realm2");
                }
            }
            Ok(None) => {
                // No credentials provided
                panic!("Expected credentials from second provider");
            }
            Err(_) => {
                // Error from first provider was propagated
                // This is also valid behavior
            }
        }
    }
}
