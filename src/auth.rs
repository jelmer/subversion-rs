use crate::Error;
use std::marker::PhantomData;

pub struct AuthBaton<'pool> {
    ptr: *mut subversion_sys::svn_auth_baton_t,
    _pool: std::marker::PhantomData<&'pool apr::Pool>,
}
unsafe impl Send for AuthBaton<'_> {}

pub trait Credentials {
    fn kind() -> &'static str;

    fn as_mut_ptr(&mut self) -> *mut std::ffi::c_void;

    fn from_raw(cred: *mut std::ffi::c_void) -> Self
    where
        Self: Sized;
}

pub struct SimpleCredentials<'pool> {
    ptr: *mut subversion_sys::svn_auth_cred_simple_t,
    _pool: std::marker::PhantomData<&'pool apr::Pool>,
}

impl<'pool> SimpleCredentials<'pool> {
    pub fn username(&self) -> &str {
        unsafe {
            std::ffi::CStr::from_ptr((*self.ptr).username)
                .to_str()
                .unwrap()
        }
    }

    pub fn password(&self) -> &str {
        unsafe {
            std::ffi::CStr::from_ptr((*self.ptr).password)
                .to_str()
                .unwrap()
        }
    }

    pub fn set_username(&mut self, username: &str, pool: &apr::Pool) {
        unsafe {
            (*self.ptr).username = apr::strings::pstrdup_raw(username, pool).unwrap() as *mut _;
        }
    }

    pub fn may_save(&self) -> bool {
        unsafe { (*self.ptr).may_save != 0 }
    }

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
    fn kind() -> &'static str {
        std::str::from_utf8(subversion_sys::SVN_AUTH_CRED_SIMPLE).unwrap()
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
}

impl<'pool> AuthBaton<'pool> {
    pub fn as_mut_ptr(&mut self) -> *mut subversion_sys::svn_auth_baton_t {
        self.ptr
    }

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
        let name = std::ffi::CString::new(name).unwrap();
        subversion_sys::svn_auth_get_parameter(self.ptr, name.as_ptr())
    }

    /// Set a parameter on the auth baton.
    ///
    /// # Safety
    /// The caller must ensure that the value is valid for the lifetime of the auth baton.
    pub unsafe fn set_parameter(&mut self, name: &str, value: *const std::ffi::c_void) {
        let name = std::ffi::CString::new(name).unwrap();
        subversion_sys::svn_auth_set_parameter(self.ptr, name.as_ptr(), value);
    }

    pub fn open(providers: &[impl AsAuthProvider]) -> Result<Self, Error> {
        let pool = apr::pool::Pool::new();
        let mut baton = std::ptr::null_mut();
        unsafe {
            let mut provider_array = apr::tables::ArrayHeader::<
                *const subversion_sys::svn_auth_provider_object_t,
            >::new(&pool);
            for provider in providers {
                provider_array.push(provider.as_auth_provider(&pool));
            }
            subversion_sys::svn_auth_open(&mut baton, provider_array.as_ptr(), pool.as_mut_ptr());
            Ok(Self {
                ptr: baton,
                _pool: std::marker::PhantomData,
            })
        }
    }
}

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

pub trait AsAuthProvider {
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
pub struct AuthProvider {
    ptr: *const subversion_sys::svn_auth_provider_object_t,
    pool: apr::Pool,
    _phantom: PhantomData<*mut ()>,
}
unsafe impl Send for AuthProvider {}

impl AuthProvider {
    pub fn cred_kind(&self) -> &str {
        let cred_kind = unsafe { (*(*self.ptr).vtable).cred_kind };
        unsafe { std::ffi::CStr::from_ptr(cred_kind).to_str().unwrap() }
    }

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
pub struct SslServerCertInfo {
    ptr: *const subversion_sys::svn_auth_ssl_server_cert_info_t,
    pool: apr::Pool,
    _phantom: PhantomData<*mut ()>,
}
unsafe impl Send for SslServerCertInfo {}

impl SslServerCertInfo {
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

pub fn get_ssl_server_trust_prompt_provider(
    prompt_func: &impl Fn(&'_ str, usize, &'_ SslServerCertInfo, bool) -> Result<SslServerTrust, Error>,
) -> AuthProvider {
    let mut auth_provider = std::ptr::null_mut();

    extern "C" fn wrap_ssl_server_trust_prompt_fn(
        cred: *mut *mut subversion_sys::svn_auth_cred_ssl_server_trust_t,
        baton: *mut std::ffi::c_void,
        realmstring: *const std::ffi::c_char,
        failures: apr_sys::apr_uint32_t,
        cert_info: *const subversion_sys::svn_auth_ssl_server_cert_info_t,
        may_save: subversion_sys::svn_boolean_t,
        pool: *mut apr_sys::apr_pool_t,
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
    _pool: *mut apr_sys::apr_pool_t,
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
        _pool: *mut apr_sys::apr_pool_t,
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
        _pool: *mut apr_sys::apr_pool_t,
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

extern "C" fn wrap_plaintext_passphrase_prompt(
    may_save_plaintext: *mut subversion_sys::svn_boolean_t,
    realmstring: *const std::ffi::c_char,
    baton: *mut std::ffi::c_void,
    _pool: *mut apr_sys::apr_pool_t,
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
    let providers =
        apr::tables::ArrayHeader::<*mut subversion_sys::svn_auth_provider_object_t>::from_ptr(
            providers,
        );
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
        assert!(kind.contains("simple"));
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

        let baton = AuthBaton::open(&providers);
        assert!(baton.is_ok());
    }

    #[test]
    fn test_auth_baton_parameter_safety() {
        let providers = vec![get_username_provider()];
        let mut baton = AuthBaton::open(&providers).unwrap();

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
}
