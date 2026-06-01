//! X.509 certificate handling for Subversion
//!
//! This module provides utilities for X.509 certificate parsing and
//! information extraction, wrapping the SVN x509 C API.

use crate::Error;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// X.509 certificate information
#[derive(Debug, Clone)]
pub struct CertificateInfo {
    /// Certificate subject (DN)
    pub subject: String,
    /// Certificate issuer (DN)
    pub issuer: String,
    /// Certificate validity start time
    pub valid_from: SystemTime,
    /// Certificate validity end time
    pub valid_until: SystemTime,
    /// Certificate fingerprint (SHA-1)
    pub fingerprint: String,
    /// Subject Alternative Names
    pub subject_alt_names: Vec<String>,
}

/// Parse a certificate from PEM format
///
/// Decodes the base64 PEM data to DER and then parses using `svn_x509_parse_cert`.
pub fn parse_certificate_pem(pem_data: &str) -> Result<CertificateInfo, Error<'static>> {
    if pem_data.is_empty() {
        return Err(Error::from_message("Empty PEM data"));
    }

    if !pem_data.contains("-----BEGIN CERTIFICATE-----") {
        return Err(Error::from_message(
            "Invalid PEM format: missing certificate header",
        ));
    }

    // Extract base64 content between PEM markers
    let base64_content: String = pem_data
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect();

    // Decode base64 to DER
    let der_data = crate::base64::decode(&base64_content)?;

    parse_certificate_der(&der_data)
}

/// Parse a certificate from DER format
pub fn parse_certificate_der(der_data: &[u8]) -> Result<CertificateInfo, Error<'static>> {
    if der_data.is_empty() {
        return Err(Error::from_message("Empty DER data"));
    }

    let pool = apr::Pool::new();
    let mut certinfo: *mut subversion_sys::svn_x509_certinfo_t = std::ptr::null_mut();

    let err = unsafe {
        subversion_sys::svn_x509_parse_cert(
            &mut certinfo,
            der_data.as_ptr() as *const i8,
            der_data.len() as apr_sys::apr_size_t,
            pool.as_mut_ptr(),
            pool.as_mut_ptr(),
        )
    };
    Error::from_raw(err)?;

    if certinfo.is_null() {
        return Err(Error::from_message("Failed to parse certificate"));
    }

    // Extract certificate information
    let subject = unsafe {
        let subject_ptr =
            subversion_sys::svn_x509_certinfo_get_subject(certinfo, pool.as_mut_ptr());
        if subject_ptr.is_null() {
            "".to_string()
        } else {
            std::ffi::CStr::from_ptr(subject_ptr)
                .to_str()
                .unwrap_or("")
                .to_string()
        }
    };

    let issuer = unsafe {
        let issuer_ptr = subversion_sys::svn_x509_certinfo_get_issuer(certinfo, pool.as_mut_ptr());
        if issuer_ptr.is_null() {
            "".to_string()
        } else {
            std::ffi::CStr::from_ptr(issuer_ptr)
                .to_str()
                .unwrap_or("")
                .to_string()
        }
    };

    let valid_from = unsafe {
        let time = subversion_sys::svn_x509_certinfo_get_valid_from(certinfo);
        // APR time is in microseconds since epoch
        UNIX_EPOCH + Duration::from_micros(time as u64)
    };

    let valid_until = unsafe {
        let time = subversion_sys::svn_x509_certinfo_get_valid_to(certinfo);
        UNIX_EPOCH + Duration::from_micros(time as u64)
    };

    let fingerprint = unsafe {
        let digest = subversion_sys::svn_x509_certinfo_get_digest(certinfo);
        if digest.is_null() || (*digest).digest.is_null() {
            "".to_string()
        } else {
            let digest_size = subversion_sys::svn_checksum_size(digest) as usize;
            let slice = std::slice::from_raw_parts((*digest).digest, digest_size);
            slice
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<_>>()
                .join(":")
        }
    };

    let subject_alt_names = unsafe {
        let hostnames = subversion_sys::svn_x509_certinfo_get_hostnames(certinfo);
        if hostnames.is_null() {
            Vec::new()
        } else {
            let arr = &*(hostnames as *const apr_sys::apr_array_header_t);
            let mut names = Vec::new();
            for i in 0..arr.nelts {
                let ptr = *(arr.elts as *const *const i8).offset(i as isize);
                if !ptr.is_null() {
                    if let Ok(s) = std::ffi::CStr::from_ptr(ptr).to_str() {
                        names.push(s.to_string());
                    }
                }
            }
            names
        }
    };

    Ok(CertificateInfo {
        subject,
        issuer,
        valid_from,
        valid_until,
        fingerprint,
        subject_alt_names,
    })
}

/// Calculate SHA-1 fingerprint of certificate DER data.
///
/// Returns the fingerprint as colon-separated hex bytes.
pub fn calculate_fingerprint(cert_data: &[u8]) -> Result<String, Error<'static>> {
    if cert_data.is_empty() {
        return Err(Error::from_message("Empty certificate data"));
    }

    let pool = apr::Pool::new();
    let checksum = crate::checksum(crate::ChecksumKind::SHA1, cert_data, &pool)?;
    let hex = checksum.to_hex(&pool);
    // Convert hex string "aabbccdd..." to "aa:bb:cc:dd:..."
    let bytes: Vec<&str> = (0..hex.len()).step_by(2).map(|i| &hex[i..i + 2]).collect();
    Ok(bytes.join(":"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_certificate_pem_missing_header() {
        let invalid_pem = "not a certificate";
        let result = parse_certificate_pem(invalid_pem);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_certificate_pem_empty() {
        let result = parse_certificate_pem("");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_certificate_pem_invalid_der() {
        // Valid PEM structure but invalid certificate content
        let pem_data = "-----BEGIN CERTIFICATE-----\nMIIC\n-----END CERTIFICATE-----";
        let result = parse_certificate_pem(pem_data);
        // Should fail during DER parsing
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_certificate_der_empty() {
        let empty_der: Vec<u8> = vec![];
        let result = parse_certificate_der(&empty_der);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_certificate_der_invalid() {
        // Invalid DER data should return an error
        let invalid_der = vec![0x30, 0x82, 0x02, 0x5c];
        let result = parse_certificate_der(&invalid_der);
        assert!(result.is_err());
    }

    // A self-signed test certificate (CN=test.example.com, O=Test Org) with
    // subjectAltNames test.example.com and alt.example.com. Generated with
    // `openssl req -x509`. Used to exercise the success path of cert parsing.
    const TEST_CERT_PEM: &str = "-----BEGIN CERTIFICATE-----
MIIDbDCCAlSgAwIBAgIUCAeaZuicutxOVaVBKFdCi/tjL7IwDQYJKoZIhvcNAQEL
BQAwLjEZMBcGA1UEAwwQdGVzdC5leGFtcGxlLmNvbTERMA8GA1UECgwIVGVzdCBP
cmcwHhcNMjYwNjAxMTAwNTAwWhcNMzYwNTI5MTAwNTAwWjAuMRkwFwYDVQQDDBB0
ZXN0LmV4YW1wbGUuY29tMREwDwYDVQQKDAhUZXN0IE9yZzCCASIwDQYJKoZIhvcN
AQEBBQADggEPADCCAQoCggEBAMDTQdPvvJqqa6wT16jiDZJUmwgs5XQ+Dxc5Xvvd
QzWoi5uouPDbxwdbu5bBaLwabaW0IvVwec4ZmVT012EWy6q5KBbvEldlUjrH1utR
dNi2isE1Jt5mZKeMF8ANjjpBGppt08hB18iIvgHWyE5lCd3SvvGfo1bYfmR7QMcm
c+X6f82UJgaNwp6R6X4yjxOyrvO9PjTGOrw/CYdVuPOBUAuOa0bOOl7k7+qMTjnl
WQh7jpj7EImRFheNvsv291zVh1U1imo17o0Q3XwsEEd8UP2zGkd9MlpuF3Dw4pJX
MjirODGKxyw2jVqOk4PbP1Sk1EXeQvc1DYzinGem+l+oNgMCAwEAAaOBgTB/MB0G
A1UdDgQWBBTcGCNnGHnxrsv7D3woCSzIY2OuTzAfBgNVHSMEGDAWgBTcGCNnGHnx
rsv7D3woCSzIY2OuTzAPBgNVHRMBAf8EBTADAQH/MCwGA1UdEQQlMCOCEHRlc3Qu
ZXhhbXBsZS5jb22CD2FsdC5leGFtcGxlLmNvbTANBgkqhkiG9w0BAQsFAAOCAQEA
N3bhFE+mJUqwc5AU2kS0oeQ/JwupKZRFWNnb1rlIx73YD+4pKol3xLJ1H0wcLnA3
+ZeGX288ASCwZ2+EfyY56GyOujLz0j0shkV1zuyWAENHCkhRK5Hl9IQFBiAb+wNo
3XnjQFtF+oZGs7rjvgb/ozay2YzTM2fiUqANBjARSrPNbtgDfa5U2Php1jCNxIDH
J3HKhSFpt9jlb+3dvhq9ShlcmV2F5y/glfGVRsFZvJBW4sYO/lcT58zIylnwN2oV
ebn+krKm/zqHOQhbSju7gb0iuY9ZFIN/+QfSJNd7n/hAfWW5IslhexkctzcGjWiW
kgT02u0H17qYrydWSGau0A==
-----END CERTIFICATE-----";

    #[test]
    fn test_parse_certificate_pem_success() {
        let info = parse_certificate_pem(TEST_CERT_PEM).unwrap();

        assert_eq!(info.subject, "CN=test.example.com, O=Test Org");
        assert_eq!(info.issuer, "CN=test.example.com, O=Test Org");
        assert_eq!(
            info.subject_alt_names,
            vec![
                "test.example.com".to_string(),
                "alt.example.com".to_string()
            ]
        );

        // Fingerprint is 20 SHA-1 bytes rendered as colon-separated hex.
        assert_eq!(info.fingerprint.matches(':').count(), 19);
        assert!(!info.fingerprint.is_empty());

        // Validity window: both times are after the Unix epoch and ordered.
        assert!(info.valid_from > UNIX_EPOCH);
        assert!(info.valid_until > UNIX_EPOCH);
        assert!(info.valid_from < info.valid_until);
    }

    #[test]
    fn test_parse_certificate_der_success() {
        // Decode the PEM body ourselves and feed DER directly.
        let base64_body: String = TEST_CERT_PEM
            .lines()
            .filter(|l| !l.starts_with("-----"))
            .collect();
        let der = crate::base64::decode(&base64_body).unwrap();

        let info = parse_certificate_der(&der).unwrap();
        assert_eq!(info.subject, "CN=test.example.com, O=Test Org");
        assert_eq!(info.fingerprint.matches(':').count(), 19);
    }

    #[test]
    fn test_calculate_fingerprint() {
        let cert_data = vec![0x30, 0x82, 0x02, 0x5c]; // Fake certificate data
        let fp = calculate_fingerprint(&cert_data).unwrap();
        // SHA-1 produces 20 bytes = 40 hex chars = 20 colon-separated pairs
        assert_eq!(fp.matches(':').count(), 19);

        let empty_data: Vec<u8> = vec![];
        let result = calculate_fingerprint(&empty_data);
        assert!(result.is_err());
    }
}
