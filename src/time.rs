use crate::Error;
use apr::time::Time;
use subversion_sys::{svn_time_from_cstring, svn_time_to_cstring, svn_time_to_human_cstring};

/// Converts a time to its canonical string representation.
pub fn to_cstring(time: Time) -> String {
    let pool = apr::pool::Pool::new();
    let x = unsafe { svn_time_to_cstring(time.into(), pool.as_mut_ptr()) };
    let s = unsafe { std::ffi::CStr::from_ptr(x) };
    s.to_string_lossy().into_owned()
}

/// Parses a time from its canonical string representation.
pub fn from_cstring(s: &str) -> Result<Time, crate::Error> {
    let mut t = apr::apr_time_t::default();
    let s = std::ffi::CString::new(s).unwrap();
    let pool = apr::pool::Pool::new();
    let err = unsafe { svn_time_from_cstring(&mut t, s.as_ptr(), pool.as_mut_ptr()) };
    Error::from_raw(err)?;
    Ok(Time::from(t))
}

/// Converts a time to a human-readable string representation.
pub fn to_human_cstring(time: Time) -> String {
    let pool = apr::pool::Pool::new();
    let x = unsafe { svn_time_to_human_cstring(time.into(), pool.as_mut_ptr()) };
    let s = unsafe { std::ffi::CStr::from_ptr(x) };
    s.to_string_lossy().into_owned()
}

/// Parses a date string into a time value.
pub fn parse_date(now: Time, date: &str) -> Result<(bool, Time), crate::Error> {
    let mut t = apr::apr_time_t::default();
    let mut matched: i32 = 0;
    let date = std::ffi::CString::new(date).unwrap();
    let pool = apr::pool::Pool::new();
    let err = unsafe {
        subversion_sys::svn_parse_date(
            &mut matched,
            &mut t,
            date.as_ptr(),
            now.into(),
            pool.as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;
    Ok((matched != 0, Time::from(t)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let time = Time::from(1234567890000000_i64); // microseconds
        let s = to_cstring(time);
        let parsed = from_cstring(&s).unwrap();
        let parsed_raw: apr::apr_time_t = parsed.into();
        let time_raw: apr::apr_time_t = time.into();
        assert_eq!(parsed_raw, time_raw);
    }

    #[test]
    fn test_from_cstring_valid() {
        let time_str = "2009-02-13T23:31:30.000000Z";
        let result = from_cstring(time_str);
        assert!(result.is_ok());
        let time = result.unwrap();
        let time_raw: apr::apr_time_t = time.into();
        assert_eq!(time_raw, 1234567890000000);
    }

    #[test]
    fn test_from_cstring_invalid() {
        let time_str = "not a valid time";
        let result = from_cstring(time_str);
        assert!(result.is_err());
    }

    #[test]
    fn test_to_cstring_format() {
        let time = Time::from(1234567890000000_i64);
        let s = to_cstring(time);
        assert_eq!(s, "2009-02-13T23:31:30.000000Z");
    }

    #[test]
    fn test_to_human_cstring_format() {
        let time = Time::from(1234567890000000_i64);
        let s = to_human_cstring(time);
        // Human format is like "2009-02-13 23:31:30 +0000 (Fri, 13 Feb 2009)"
        assert_eq!(s, "2009-02-13 23:31:30 +0000 (Fri, 13 Feb 2009)");
    }

    #[test]
    fn test_parse_date_iso8601() {
        let now = Time::now();
        let result = parse_date(now, "2009-02-13T23:31:30.000000Z");
        assert!(result.is_ok());
        let (matched, parsed_time) = result.unwrap();
        assert_eq!(matched, true);
        let parsed_raw: apr::apr_time_t = parsed_time.into();
        assert_eq!(parsed_raw, 1234567890000000);
    }

    #[test]
    fn test_parse_date_invalid() {
        let now = Time::now();
        let result = parse_date(now, "not a valid date");
        assert!(result.is_ok());
        let (matched, _) = result.unwrap();
        assert_eq!(matched, false);
    }
}
