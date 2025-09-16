use crate::Error;
use apr::time::Time;
use subversion_sys::{svn_time_from_cstring, svn_time_to_cstring, svn_time_to_human_cstring};

/// Converts a time to its canonical string representation.
pub fn to_cstring(time: Time) -> String {
    let x = unsafe { svn_time_to_cstring(time.into(), apr::pool::Pool::new().as_mut_ptr()) };
    let s = unsafe { std::ffi::CStr::from_ptr(x) };
    s.to_string_lossy().into_owned()
}

/// Parses a time from its canonical string representation.
pub fn from_cstring(s: &str) -> Result<Time, crate::Error> {
    let mut t = apr::apr_time_t::default();
    let s = std::ffi::CString::new(s).unwrap();
    let err =
        unsafe { svn_time_from_cstring(&mut t, s.as_ptr(), apr::pool::Pool::new().as_mut_ptr()) };
    Error::from_raw(err)?;
    Ok(Time::from(t))
}

/// Converts a time to a human-readable string representation.
pub fn to_human_cstring(time: Time) -> String {
    let x = unsafe { svn_time_to_human_cstring(time.into(), apr::pool::Pool::new().as_mut_ptr()) };
    let s = unsafe { std::ffi::CStr::from_ptr(x) };
    s.to_string_lossy().into_owned()
}

/// Parses a date string into a time value.
pub fn parse_date(now: Time, date: &str) -> Result<(bool, Time), crate::Error> {
    let mut t = apr::apr_time_t::default();
    let mut matched: i32 = 0;
    let date = std::ffi::CString::new(date).unwrap();
    let err = unsafe {
        subversion_sys::svn_parse_date(
            &mut matched,
            &mut t,
            date.as_ptr(),
            now.into(),
            apr::pool::Pool::new().as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok((matched != 0, Time::from(t)))
}
