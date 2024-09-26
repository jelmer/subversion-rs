use crate::Error;
use apr::pool::PooledPtr;

pub struct AuthBaton(PooledPtr<crate::generated::svn_auth_baton_t>);
unsafe impl Send for AuthBaton {}

pub trait Credentials {
    fn kind() -> &'static str;

    fn from_raw(cred: *mut std::ffi::c_void, pool: std::rc::Rc<apr::pool::Pool>) -> Self
    where
        Self: Sized;
}

pub struct SimpleCredentials(PooledPtr<crate::generated::svn_auth_cred_simple_t>);

impl SimpleCredentials {
    pub fn username(&self) -> &str {
        unsafe { std::ffi::CStr::from_ptr(self.0.username).to_str().unwrap() }
    }

    pub fn password(&self) -> &str {
        unsafe { std::ffi::CStr::from_ptr(self.0.password).to_str().unwrap() }
    }

    pub fn may_save(&self) -> bool {
        self.0.may_save != 0
    }
}

impl Credentials for SimpleCredentials {
    fn kind() -> &'static str {
        std::str::from_utf8(crate::generated::SVN_AUTH_CRED_SIMPLE).unwrap()
    }

    fn from_raw(cred: *mut std::ffi::c_void, pool: std::rc::Rc<apr::pool::Pool>) -> Self {
        unsafe {
            Self(PooledPtr::in_pool(
                pool,
                cred as *mut crate::generated::svn_auth_cred_simple_t,
            ))
        }
    }
}

impl AuthBaton {
    pub fn as_mut_ptr(&mut self) -> *mut crate::generated::svn_auth_baton_t {
        self.0.as_mut_ptr()
    }

    pub fn credentials<C: Credentials>(&mut self, realm: &str) -> Result<IterState<C>, Error> {
        let cred_kind = std::ffi::CString::new(C::kind()).unwrap();
        let realm = std::ffi::CString::new(realm).unwrap();
        let mut cred = std::ptr::null_mut();
        let mut state = std::ptr::null_mut();
        let pool = apr::pool::Pool::new();
        unsafe {
            let err = crate::generated::svn_auth_first_credentials(
                &mut cred,
                &mut state,
                cred_kind.as_ptr(),
                realm.as_ptr(),
                self.0.as_mut_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            let pool = std::rc::Rc::new(pool);
            let first_creds = C::from_raw(cred, pool.clone());
            Ok(IterState(
                PooledPtr::in_pool(pool, state),
                Some(first_creds),
            ))
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
            crate::generated::svn_auth_forget_credentials(
                self.0.as_mut_ptr(),
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
        crate::generated::svn_auth_get_parameter(self.0.as_mut_ptr(), name.as_ptr())
    }

    /// Set a parameter on the auth baton.
    ///
    /// # Safety
    /// The caller must ensure that the value is valid for the lifetime of the auth baton.
    pub unsafe fn set_parameter(&mut self, name: &str, value: *const std::ffi::c_void) {
        let name = std::ffi::CString::new(name).unwrap();
        crate::generated::svn_auth_set_parameter(self.0.as_mut_ptr(), name.as_ptr(), value);
    }

    pub fn open(providers: &[&impl AsAuthProvider]) -> Result<Self, Error> {
        let mut baton = std::ptr::null_mut();
        Ok(Self(PooledPtr::initialize(|pool| unsafe {
            let mut provider_array = apr::tables::ArrayHeader::<
                *const crate::generated::svn_auth_provider_object_t,
            >::new();
            for provider in providers {
                provider_array.push(provider.as_auth_provider(pool));
            }
            crate::generated::svn_auth_open(&mut baton, provider_array.as_ptr(), pool.as_mut_ptr());
            Ok::<_, Error>(baton)
        })?))
    }
}

pub struct IterState<C: Credentials>(PooledPtr<crate::generated::svn_auth_iterstate_t>, Option<C>);

impl<C: Credentials> IterState<C> {
    pub fn save_credentials(&mut self) -> Result<(), Error> {
        let pool = apr::pool::Pool::new();
        let err = unsafe {
            crate::generated::svn_auth_save_credentials(self.0.as_mut_ptr(), pool.as_mut_ptr())
        };
        Error::from_raw(err)?;
        Ok(())
    }

    fn next_credentials(&mut self) -> Result<Option<C>, Error> {
        let mut cred = std::ptr::null_mut();
        let pool = apr::pool::Pool::new();
        unsafe {
            let err = crate::generated::svn_auth_next_credentials(
                &mut cred,
                self.0.as_mut_ptr(),
                pool.as_mut_ptr(),
            );
            Error::from_raw(err)?;
            if cred.is_null() {
                return Ok(None);
            }
            Ok(Some(C::from_raw(cred, std::rc::Rc::new(pool))))
        }
    }
}

impl<C: Credentials> Iterator for IterState<C> {
    type Item = C;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(creds) = self.1.take() {
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
        pool: &mut apr::pool::Pool,
    ) -> *mut crate::generated::svn_auth_provider_object_t;
}

#[allow(dead_code)]
pub struct AuthProviderObject(PooledPtr<crate::generated::svn_auth_provider_object_t>);
unsafe impl Send for AuthProviderObject {}

pub fn get_username_provider() -> AuthProviderObject {
    let mut auth_provider = std::ptr::null_mut();

    AuthProviderObject(
        PooledPtr::initialize(|pool| unsafe {
            crate::generated::svn_auth_get_username_provider(&mut auth_provider, pool.as_mut_ptr());
            Ok::<_, Error>(auth_provider)
        })
        .unwrap(),
    )
}

pub fn get_ssl_server_trust_file_provider() -> AuthProviderObject {
    let mut auth_provider = std::ptr::null_mut();

    AuthProviderObject(
        PooledPtr::initialize(|pool| unsafe {
            crate::generated::svn_auth_get_ssl_server_trust_file_provider(
                &mut auth_provider,
                pool.as_mut_ptr(),
            );
            Ok::<_, Error>(auth_provider)
        })
        .unwrap(),
    )
}

pub fn get_ssl_client_cert_file_provider() -> AuthProviderObject {
    let mut auth_provider = std::ptr::null_mut();

    AuthProviderObject(
        PooledPtr::initialize(|pool| unsafe {
            crate::generated::svn_auth_get_ssl_client_cert_file_provider(
                &mut auth_provider,
                pool.as_mut_ptr(),
            );
            Ok::<_, Error>(auth_provider)
        })
        .unwrap(),
    )
}

pub struct SslClientCertCredentials(crate::generated::svn_auth_cred_ssl_client_cert_t);
unsafe impl Send for SslClientCertCredentials {}

extern "C" fn wrap_client_cert_prompt_fn(cred: *mut *mut crate::generated::svn_auth_cred_ssl_client_cert_t, baton: *mut std::ffi::c_void, realmstring: *const std::ffi::c_char, may_save: crate::generated::svn_boolean_t, _pool: *mut apr::apr_pool_t) -> *mut crate::generated::svn_error_t {
    let f = unsafe { &*(baton as *const &dyn Fn(&str, bool) -> Result<SslClientCertCredentials, crate::Error>) };
    let realm = unsafe { std::ffi::CStr::from_ptr(realmstring).to_str().unwrap() };
    f(realm, may_save != 0)
        .map(|creds| {
            unsafe { *cred = Box::into_raw(Box::new(creds.0)) };
            std::ptr::null_mut()
        })
        .unwrap_or_else(|e| unsafe { e.into_raw() })
}

pub fn get_ssl_client_cert_prompt_provider(
    prompt_fn: &impl Fn(&str, bool) -> Result<SslClientCertCredentials, crate::Error>,
    retry_limit: usize,
) -> AuthProviderObject {
    let mut auth_provider = std::ptr::null_mut();

    AuthProviderObject(
        PooledPtr::initialize(|pool| unsafe {
            crate::generated::svn_auth_get_ssl_client_cert_prompt_provider(
                &mut auth_provider,
                Some(wrap_client_cert_prompt_fn),
                prompt_fn as *const _ as *mut std::ffi::c_void,
                retry_limit.try_into().unwrap(),
                pool.as_mut_ptr(),
            );
            Ok::<_, Error>(auth_provider)
        })
        .unwrap(),
    )
}

pub fn get_ssl_client_cert_pw_file_provider(
    prompt_fn: &impl Fn(&str) -> Result<bool, crate::Error>,
) -> AuthProviderObject {
    let mut auth_provider = std::ptr::null_mut();

    AuthProviderObject(
        PooledPtr::initialize(|pool| unsafe {
            crate::generated::svn_auth_get_ssl_client_cert_pw_file_provider2(
                &mut auth_provider,
                Some(wrap_plaintext_passphrase_prompt),
                prompt_fn as *const _ as *mut std::ffi::c_void,
                pool.as_mut_ptr(),
            );
            Ok::<_, Error>(auth_provider)
        })
        .unwrap(),
    )
}



pub fn get_simple_prompt_provider(
    prompt_fn: &impl Fn(&str, &str, bool) -> Result<SimpleCredentials, crate::Error>,
    retry_limit: usize,
) -> AuthProviderObject {
    let mut auth_provider = std::ptr::null_mut();

    extern "C" fn wrap_simple_prompt_provider(
        credentials: *mut *mut crate::generated::svn_auth_cred_simple_t,
        baton: *mut std::ffi::c_void,
        realmstring: *const std::ffi::c_char,
        username: *const std::ffi::c_char,
        may_save: crate::generated::svn_boolean_t,
        _pool: *mut apr::apr_pool_t,
    ) -> *mut crate::generated::svn_error_t {
        let f = unsafe {
            &*(baton as *const &dyn Fn(&str, &str, bool) -> Result<SimpleCredentials, crate::Error>)
        };
        let realm = unsafe { std::ffi::CStr::from_ptr(realmstring).to_str().unwrap() };
        let username = unsafe { std::ffi::CStr::from_ptr(username).to_str().unwrap() };
        f(realm, username, may_save != 0)
            .map(|mut creds| {
                unsafe { *credentials = creds.0.as_mut_ptr() };
                std::ptr::null_mut()
            })
            .unwrap_or_else(|e| unsafe { e.into_raw() })
    }

    AuthProviderObject(
        PooledPtr::initialize(|pool| unsafe {
            crate::generated::svn_auth_get_simple_prompt_provider(
                &mut auth_provider,
                Some(wrap_simple_prompt_provider),
                prompt_fn as *const _ as *mut std::ffi::c_void,
                retry_limit.try_into().unwrap(),
                pool.as_mut_ptr(),
            );
            Ok::<_, Error>(auth_provider)
        })
        .unwrap(),
    )
}

pub fn get_username_prompt_provider(
    prompt_fn: &impl Fn(&str, bool) -> Result<String, crate::Error>,
    retry_limit: usize,
) -> AuthProviderObject {
    let mut auth_provider = std::ptr::null_mut();

    extern "C" fn wrap_username_prompt_provider(
        credentials: *mut *mut crate::generated::svn_auth_cred_username_t,
        baton: *mut std::ffi::c_void,
        realmstring: *const std::ffi::c_char,
        may_save: crate::generated::svn_boolean_t,
        _pool: *mut apr::apr_pool_t,
    ) -> *mut crate::generated::svn_error_t {
        let f = unsafe { &*(baton as *const &dyn Fn(&str, bool) -> Result<String, crate::Error>) };
        let realm = unsafe { std::ffi::CStr::from_ptr(realmstring).to_str().unwrap() };
        f(realm, may_save != 0)
            .map(|username| {
                let username = std::ffi::CString::new(username).unwrap();
                let creds = crate::generated::svn_auth_cred_username_t {
                    username: username.as_ptr(),
                    may_save,
                };
                unsafe { *credentials = Box::into_raw(Box::new(creds)) };
                std::ptr::null_mut()
            })
            .unwrap_or_else(|e| unsafe { e.into_raw() })
    }

    AuthProviderObject(
        PooledPtr::initialize(|pool| unsafe {
            crate::generated::svn_auth_get_username_prompt_provider(
                &mut auth_provider,
                Some(wrap_username_prompt_provider),
                prompt_fn as *const _ as *mut std::ffi::c_void,
                retry_limit.try_into().unwrap(),
                pool.as_mut_ptr(),
            );
            Ok::<_, Error>(auth_provider)
        })
        .unwrap(),
    )
}

    extern "C" fn wrap_plaintext_passphrase_prompt(
        may_save_plaintext: *mut crate::generated::svn_boolean_t,
        realmstring: *const std::ffi::c_char,
        baton: *mut std::ffi::c_void,
        _pool: *mut apr::apr_pool_t,
    ) -> *mut crate::generated::svn_error_t {
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
    plaintext_prompt_func: &impl Fn(&str) -> Result<bool, crate::Error>,
) -> AuthProviderObject {
    let mut auth_provider = std::ptr::null_mut();

    AuthProviderObject(
        PooledPtr::initialize(|pool| unsafe {
            crate::generated::svn_auth_get_simple_provider2(
                &mut auth_provider,
                Some(wrap_plaintext_passphrase_prompt),
                plaintext_prompt_func as *const _ as *mut std::ffi::c_void,
                pool.as_mut_ptr(),
            );
            Ok::<_, Error>(auth_provider)
        })
        .unwrap(),
    )
}

pub fn get_platform_specific_provider(
    provider_name: &str,
    provider_type: &str,
) -> Result<AuthProviderObject, crate::Error> {
    let mut auth_provider = std::ptr::null_mut();
    let provider_name = std::ffi::CString::new(provider_name).unwrap();
    let provider_type = std::ffi::CString::new(provider_type).unwrap();
    let pool = apr::pool::Pool::new();
    let err = unsafe {
        crate::generated::svn_auth_get_platform_specific_provider(
            &mut auth_provider,
            provider_name.as_ptr(),
            provider_type.as_ptr(),
            pool.as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    let pool = std::rc::Rc::new(pool);
    Ok(AuthProviderObject(unsafe {
        PooledPtr::in_pool(pool, auth_provider)
    }))
}

pub fn get_platform_specific_client_providers() -> Result<Vec<AuthProviderObject>, Error> {
    let pool = std::rc::Rc::new(apr::pool::Pool::new());
    let mut providers = std::ptr::null_mut();
    let err = unsafe {
        crate::generated::svn_auth_get_platform_specific_client_providers(
            &mut providers,
            // TODO: pass in config
            std::ptr::null_mut(),
            pool.as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    let providers = unsafe { apr::tables::ArrayHeader::<*mut crate::generated::svn_auth_provider_object_t>::from_raw_parts(&pool, providers) };
    Ok(providers
        .iter()
        .map(|p| AuthProviderObject(unsafe { PooledPtr::in_pool(pool.clone(), p) }))
        .collect())
}
