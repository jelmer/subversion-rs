//! Base64 encoding and decoding using Subversion's base64 functions.

use crate::Error;

/// Decode a base64-encoded string into bytes.
pub fn decode(input: &str) -> Result<Vec<u8>, Error<'static>> {
    let pool = apr::Pool::new();
    let svn_str = subversion_sys::svn_string_t {
        data: input.as_ptr() as *const std::ffi::c_char,
        len: input.len() as apr_sys::apr_size_t,
    };
    let result = unsafe { subversion_sys::svn_base64_decode_string(&svn_str, pool.as_mut_ptr()) };
    if result.is_null() {
        return Err(Error::from_message("Base64 decode returned null"));
    }
    let result_ref = unsafe { &*result };
    let bytes = unsafe { std::slice::from_raw_parts(result_ref.data as *const u8, result_ref.len) };
    Ok(bytes.to_vec())
}

/// Encode bytes into a base64 string.
pub fn encode(input: &[u8]) -> Result<String, Error<'static>> {
    let pool = apr::Pool::new();
    let svn_str = subversion_sys::svn_string_t {
        data: input.as_ptr() as *const std::ffi::c_char,
        len: input.len() as apr_sys::apr_size_t,
    };
    let result =
        unsafe { subversion_sys::svn_base64_encode_string2(&svn_str, 0, pool.as_mut_ptr()) };
    if result.is_null() {
        return Err(Error::from_message("Base64 encode returned null"));
    }
    let result_ref = unsafe { &*result };
    let bytes = unsafe { std::slice::from_raw_parts(result_ref.data as *const u8, result_ref.len) };
    String::from_utf8(bytes.to_vec())
        .map_err(|_| Error::from_message("Base64 encode produced invalid UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_roundtrip() {
        let original = b"Hello, World!";
        let encoded = encode(original).unwrap();
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_decode_known_value() {
        let decoded = decode("SGVsbG8=").unwrap();
        assert_eq!(decoded, b"Hello");
    }

    #[test]
    fn test_encode_known_value() {
        let encoded = encode(b"Hello").unwrap();
        assert_eq!(encoded.trim(), "SGVsbG8=");
    }

    #[test]
    fn test_empty() {
        let encoded = encode(b"").unwrap();
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded, b"");
    }
}
