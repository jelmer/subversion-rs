use crate::Error;
use apr::pool::PooledPtr;

pub struct AuthBaton(PooledPtr<crate::generated::svn_auth_baton_t>);

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
        let mut pool = apr::pool::Pool::new();
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
        let mut pool = apr::pool::Pool::new();
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
        let mut pool = apr::pool::Pool::new();
        let err = unsafe {
            crate::generated::svn_auth_save_credentials(self.0.as_mut_ptr(), pool.as_mut_ptr())
        };
        Error::from_raw(err)?;
        Ok(())
    }

    fn next_credentials(&mut self) -> Result<Option<C>, Error> {
        let mut cred = std::ptr::null_mut();
        let mut pool = apr::pool::Pool::new();
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
