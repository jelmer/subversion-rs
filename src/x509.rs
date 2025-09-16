//! X.509 certificate handling for Subversion
//!
//! This module provides utilities for X.509 certificate parsing, validation,
//! and information extraction, commonly used for HTTPS connections in Subversion.
//!
//! Note: Many X.509 certificate functions are not available in the current
//! subversion-sys bindings, so this module provides a framework that can be
//! extended when those bindings become available.

use crate::Error;
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// X.509 certificate information
#[derive(Debug, Clone)]
pub struct CertificateInfo {
    /// Certificate subject (DN)
    pub subject: String,
    /// Certificate issuer (DN)
    pub issuer: String,
    /// Certificate serial number
    pub serial_number: String,
    /// Certificate validity start time
    pub valid_from: SystemTime,
    /// Certificate validity end time
    pub valid_until: SystemTime,
    /// Certificate fingerprint (SHA-1)
    pub fingerprint: String,
    /// Certificate algorithm
    pub signature_algorithm: String,
    /// Certificate version
    pub version: u32,
    /// Subject Alternative Names
    pub subject_alt_names: Vec<String>,
    /// Key usage
    pub key_usage: Vec<KeyUsage>,
    /// Extended key usage
    pub extended_key_usage: Vec<ExtendedKeyUsage>,
}

/// Key usage enumeration
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyUsage {
    /// Digital signature capability.
    DigitalSignature,
    /// Non-repudiation capability.
    NonRepudiation,
    /// Key encipherment capability.
    KeyEncipherment,
    /// Data encipherment capability.
    DataEncipherment,
    /// Key agreement capability.
    KeyAgreement,
    /// Certificate signing capability.
    KeyCertSign,
    /// CRL signing capability.
    CrlSign,
    /// Encipher only capability.
    EncipherOnly,
    /// Decipher only capability.
    DecipherOnly,
}

/// Extended key usage enumeration
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtendedKeyUsage {
    /// TLS server authentication.
    ServerAuth,
    /// TLS client authentication.
    ClientAuth,
    /// Code signing.
    CodeSigning,
    /// Email protection.
    EmailProtection,
    /// Time stamping.
    TimeStamping,
    /// OCSP signing.
    OcspSigning,
    /// Other extended key usage.
    Other(String),
}

impl CertificateInfo {
    /// Check if the certificate is currently valid
    pub fn is_valid_now(&self) -> bool {
        let now = SystemTime::now();
        now >= self.valid_from && now <= self.valid_until
    }

    /// Check if the certificate is expired
    pub fn is_expired(&self) -> bool {
        SystemTime::now() > self.valid_until
    }

    /// Check if the certificate is not yet valid
    pub fn is_not_yet_valid(&self) -> bool {
        SystemTime::now() < self.valid_from
    }

    /// Get the time remaining until expiration
    pub fn time_until_expiry(&self) -> Result<Duration, Error> {
        self.valid_until
            .duration_since(SystemTime::now())
            .map_err(|_| Error::from_str("Certificate is already expired"))
    }

    /// Get the age of the certificate
    pub fn age(&self) -> Result<Duration, Error> {
        SystemTime::now()
            .duration_since(self.valid_from)
            .map_err(|_| Error::from_str("Certificate is not yet valid"))
    }

    /// Check if certificate is valid for a specific hostname
    pub fn is_valid_for_hostname(&self, hostname: &str) -> bool {
        // Check subject CN
        if self.subject.contains(&format!("CN={}", hostname)) {
            return true;
        }

        // Check subject alternative names
        for san in &self.subject_alt_names {
            if san == hostname {
                return true;
            }
            // Support wildcard matching
            if san.starts_with("*.") {
                let wildcard_domain = &san[2..];
                if hostname.ends_with(wildcard_domain) && hostname.len() > wildcard_domain.len() {
                    // Check if there's exactly one dot separating the prefix from the wildcard domain
                    let prefix_with_dot = &hostname[..hostname.len() - wildcard_domain.len()];
                    if prefix_with_dot.ends_with('.') {
                        let prefix = &prefix_with_dot[..prefix_with_dot.len() - 1];
                        // Wildcard should only match one level (no dots in prefix)
                        if !prefix.contains('.') && !prefix.is_empty() {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }

    /// Check if certificate has a specific key usage
    pub fn has_key_usage(&self, usage: &KeyUsage) -> bool {
        self.key_usage.contains(usage)
    }

    /// Check if certificate has a specific extended key usage
    pub fn has_extended_key_usage(&self, usage: &ExtendedKeyUsage) -> bool {
        self.extended_key_usage.contains(usage)
    }
}

/// Certificate validation result
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationResult {
    /// Certificate is valid.
    Valid,
    /// Certificate has expired.
    Expired,
    /// Certificate is not yet valid.
    NotYetValid,
    /// Certificate has an invalid signature.
    InvalidSignature,
    /// Certificate issuer is not trusted.
    UntrustedIssuer,
    /// Certificate has been revoked.
    Revoked,
    /// Certificate is not valid for the hostname.
    InvalidHostname,
    /// Other validation error.
    Other(String),
}

/// Certificate validation context
#[derive(Debug, Clone)]
pub struct ValidationContext {
    /// Trusted certificate authorities
    pub trusted_cas: Vec<CertificateInfo>,
    /// Certificate revocation lists
    pub crls: Vec<String>,
    /// Whether to check certificate revocation
    pub check_revocation: bool,
    /// Whether to validate hostname
    pub validate_hostname: bool,
    /// Target hostname for validation
    pub hostname: Option<String>,
    /// Maximum certificate chain depth
    pub max_chain_depth: u32,
    /// Allow self-signed certificates
    pub allow_self_signed: bool,
}

impl ValidationContext {
    /// Create a new validation context
    pub fn new() -> Self {
        Self {
            trusted_cas: Vec::new(),
            crls: Vec::new(),
            check_revocation: false,
            validate_hostname: true,
            hostname: None,
            max_chain_depth: 10,
            allow_self_signed: false,
        }
    }

    /// Add a trusted CA certificate
    pub fn add_trusted_ca(&mut self, ca_cert: CertificateInfo) {
        self.trusted_cas.push(ca_cert);
    }

    /// Set hostname for validation
    pub fn set_hostname(&mut self, hostname: String) {
        self.hostname = Some(hostname);
    }

    /// Validate a certificate
    pub fn validate(&self, cert: &CertificateInfo) -> ValidationResult {
        // Check validity period
        if cert.is_expired() {
            return ValidationResult::Expired;
        }
        if cert.is_not_yet_valid() {
            return ValidationResult::NotYetValid;
        }

        // Check hostname if specified
        if self.validate_hostname {
            if let Some(ref hostname) = self.hostname {
                if !cert.is_valid_for_hostname(hostname) {
                    return ValidationResult::InvalidHostname;
                }
            }
        }

        // TODO: When subversion-sys exposes certificate validation functions:
        // - Check certificate signature against issuer
        // - Verify certificate chain
        // - Check certificate revocation status
        // - Validate certificate against trusted CAs

        ValidationResult::Valid
    }
}

impl Default for ValidationContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a certificate from PEM format
///
/// Note: This is currently a placeholder since certificate parsing functions
/// are not available in subversion-sys bindings.
pub fn parse_certificate_pem(pem_data: &str) -> Result<CertificateInfo, Error> {
    if pem_data.is_empty() {
        return Err(Error::from_str("Empty PEM data"));
    }

    if !pem_data.contains("-----BEGIN CERTIFICATE-----") {
        return Err(Error::from_str(
            "Invalid PEM format: missing certificate header",
        ));
    }

    // TODO: When subversion-sys exposes certificate parsing functions, implement:
    // - Parse PEM to DER format
    // - Extract certificate fields
    // - Parse subject and issuer DNs
    // - Extract validity dates
    // - Calculate fingerprint

    // For now, return a placeholder certificate
    Ok(CertificateInfo {
        subject: "CN=placeholder".to_string(),
        issuer: "CN=placeholder-ca".to_string(),
        serial_number: "00".to_string(),
        valid_from: UNIX_EPOCH,
        valid_until: UNIX_EPOCH + Duration::from_secs(365 * 24 * 3600), // 1 year
        fingerprint: "00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00:00".to_string(),
        signature_algorithm: "sha256WithRSAEncryption".to_string(),
        version: 3,
        subject_alt_names: Vec::new(),
        key_usage: Vec::new(),
        extended_key_usage: Vec::new(),
    })
}

/// Parse a certificate from DER format
pub fn parse_certificate_der(der_data: &[u8]) -> Result<CertificateInfo, Error> {
    if der_data.is_empty() {
        return Err(Error::from_str("Empty DER data"));
    }

    // TODO: When available, implement DER parsing
    // This would involve parsing ASN.1 structure

    // For now, return a placeholder certificate
    Ok(CertificateInfo {
        subject: "CN=placeholder-der".to_string(),
        issuer: "CN=placeholder-der-ca".to_string(),
        serial_number: "01".to_string(),
        valid_from: UNIX_EPOCH,
        valid_until: UNIX_EPOCH + Duration::from_secs(365 * 24 * 3600), // 1 year
        fingerprint: "01:01:01:01:01:01:01:01:01:01:01:01:01:01:01:01:01:01:01:01".to_string(),
        signature_algorithm: "sha256WithRSAEncryption".to_string(),
        version: 3,
        subject_alt_names: Vec::new(),
        key_usage: Vec::new(),
        extended_key_usage: Vec::new(),
    })
}

/// Calculate certificate fingerprint
pub fn calculate_fingerprint(cert_data: &[u8]) -> Result<String, Error> {
    if cert_data.is_empty() {
        return Err(Error::from_str("Empty certificate data"));
    }

    // TODO: When available, implement SHA-1 or SHA-256 fingerprint calculation
    // This would involve hashing the certificate DER data

    // For now, return a placeholder fingerprint
    Ok(
        "placeholder:fingerprint:00:11:22:33:44:55:66:77:88:99:aa:bb:cc:dd:ee:ff:00:11:22:33"
            .to_string(),
    )
}

/// Certificate store for managing multiple certificates
#[derive(Debug, Clone)]
pub struct CertificateStore {
    /// Stored certificates indexed by fingerprint
    certificates: HashMap<String, CertificateInfo>,
}

impl CertificateStore {
    /// Create a new certificate store
    pub fn new() -> Self {
        Self {
            certificates: HashMap::new(),
        }
    }

    /// Add a certificate to the store
    pub fn add_certificate(&mut self, cert: CertificateInfo) {
        self.certificates.insert(cert.fingerprint.clone(), cert);
    }

    /// Get a certificate by fingerprint
    pub fn get_certificate(&self, fingerprint: &str) -> Option<&CertificateInfo> {
        self.certificates.get(fingerprint)
    }

    /// Remove a certificate by fingerprint
    pub fn remove_certificate(&mut self, fingerprint: &str) -> Option<CertificateInfo> {
        self.certificates.remove(fingerprint)
    }

    /// Get all certificates
    pub fn get_all_certificates(&self) -> Vec<&CertificateInfo> {
        self.certificates.values().collect()
    }

    /// Find certificates by subject
    pub fn find_by_subject(&self, subject: &str) -> Vec<&CertificateInfo> {
        self.certificates
            .values()
            .filter(|cert| cert.subject.contains(subject))
            .collect()
    }

    /// Find certificates by issuer
    pub fn find_by_issuer(&self, issuer: &str) -> Vec<&CertificateInfo> {
        self.certificates
            .values()
            .filter(|cert| cert.issuer.contains(issuer))
            .collect()
    }

    /// Find expired certificates
    pub fn find_expired(&self) -> Vec<&CertificateInfo> {
        self.certificates
            .values()
            .filter(|cert| cert.is_expired())
            .collect()
    }

    /// Find certificates expiring within a duration
    pub fn find_expiring_within(&self, duration: Duration) -> Vec<&CertificateInfo> {
        let threshold = SystemTime::now() + duration;
        self.certificates
            .values()
            .filter(|cert| cert.valid_until <= threshold && !cert.is_expired())
            .collect()
    }

    /// Get certificate count
    pub fn count(&self) -> usize {
        self.certificates.len()
    }

    /// Clear all certificates
    pub fn clear(&mut self) {
        self.certificates.clear();
    }
}

impl Default for CertificateStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract subject field from certificate subject DN
pub fn extract_subject_field(subject_dn: &str, field: &str) -> Option<String> {
    for part in subject_dn.split(',') {
        let part = part.trim();
        if let Some(equals_pos) = part.find('=') {
            let key = part[..equals_pos].trim();
            let value = part[equals_pos + 1..].trim();
            if key.eq_ignore_ascii_case(field) {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Get common name from certificate subject
pub fn get_common_name(cert: &CertificateInfo) -> Option<String> {
    extract_subject_field(&cert.subject, "CN")
}

/// Get organization from certificate subject
pub fn get_organization(cert: &CertificateInfo) -> Option<String> {
    extract_subject_field(&cert.subject, "O")
}

/// Get country from certificate subject
pub fn get_country(cert: &CertificateInfo) -> Option<String> {
    extract_subject_field(&cert.subject, "C")
}

/// Format certificate information for display
pub fn format_certificate_info(cert: &CertificateInfo) -> String {
    let mut info = String::new();
    info.push_str(&format!("Subject: {}\n", cert.subject));
    info.push_str(&format!("Issuer: {}\n", cert.issuer));
    info.push_str(&format!("Serial Number: {}\n", cert.serial_number));
    info.push_str(&format!("Valid From: {:?}\n", cert.valid_from));
    info.push_str(&format!("Valid Until: {:?}\n", cert.valid_until));
    info.push_str(&format!("Fingerprint: {}\n", cert.fingerprint));
    info.push_str(&format!(
        "Signature Algorithm: {}\n",
        cert.signature_algorithm
    ));
    info.push_str(&format!("Version: {}\n", cert.version));

    if !cert.subject_alt_names.is_empty() {
        info.push_str(&format!(
            "Subject Alt Names: {}\n",
            cert.subject_alt_names.join(", ")
        ));
    }

    info
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_certificate() -> CertificateInfo {
        let now = SystemTime::now();
        let one_hour_ago = now - Duration::from_secs(3600);
        let one_year_from_now = now + Duration::from_secs(365 * 24 * 3600);

        CertificateInfo {
            subject: "CN=example.com,O=Example Corp,C=US".to_string(),
            issuer: "CN=Example CA,O=Example Corp,C=US".to_string(),
            serial_number: "123456789".to_string(),
            valid_from: one_hour_ago,
            valid_until: one_year_from_now,
            fingerprint: "aa:bb:cc:dd:ee:ff:00:11:22:33:44:55:66:77:88:99:aa:bb:cc:dd".to_string(),
            signature_algorithm: "sha256WithRSAEncryption".to_string(),
            version: 3,
            subject_alt_names: vec!["example.com".to_string(), "*.example.com".to_string()],
            key_usage: vec![KeyUsage::DigitalSignature, KeyUsage::KeyEncipherment],
            extended_key_usage: vec![ExtendedKeyUsage::ServerAuth],
        }
    }

    #[test]
    fn test_certificate_info_creation() {
        let cert = create_test_certificate();
        assert_eq!(cert.subject, "CN=example.com,O=Example Corp,C=US");
        assert_eq!(cert.issuer, "CN=Example CA,O=Example Corp,C=US");
        assert_eq!(cert.version, 3);
    }

    #[test]
    fn test_certificate_validity() {
        let cert = create_test_certificate();
        // Our test cert is valid from one hour ago to one year from now
        assert!(cert.is_valid_now());
        assert!(!cert.is_expired());
        assert!(!cert.is_not_yet_valid());
    }

    #[test]
    fn test_hostname_validation() {
        let cert = create_test_certificate();
        assert!(cert.is_valid_for_hostname("example.com"));
        assert!(cert.is_valid_for_hostname("sub.example.com"));
        assert!(!cert.is_valid_for_hostname("other.com"));
        assert!(!cert.is_valid_for_hostname("sub.sub.example.com"));
    }

    #[test]
    fn test_key_usage() {
        let cert = create_test_certificate();
        assert!(cert.has_key_usage(&KeyUsage::DigitalSignature));
        assert!(cert.has_key_usage(&KeyUsage::KeyEncipherment));
        assert!(!cert.has_key_usage(&KeyUsage::KeyCertSign));
    }

    #[test]
    fn test_extended_key_usage() {
        let cert = create_test_certificate();
        assert!(cert.has_extended_key_usage(&ExtendedKeyUsage::ServerAuth));
        assert!(!cert.has_extended_key_usage(&ExtendedKeyUsage::ClientAuth));
    }

    #[test]
    fn test_validation_context() {
        let mut context = ValidationContext::new();
        context.set_hostname("example.com".to_string());
        context.validate_hostname = true;

        let cert = create_test_certificate();
        let result = context.validate(&cert);

        // Should be valid since our test cert is valid for example.com
        assert_eq!(result, ValidationResult::Valid);
    }

    #[test]
    fn test_certificate_store() {
        let mut store = CertificateStore::new();
        let cert = create_test_certificate();
        let fingerprint = cert.fingerprint.clone();

        assert_eq!(store.count(), 0);

        store.add_certificate(cert);
        assert_eq!(store.count(), 1);

        let retrieved = store.get_certificate(&fingerprint);
        assert!(retrieved.is_some());
        assert_eq!(
            retrieved.unwrap().subject,
            "CN=example.com,O=Example Corp,C=US"
        );

        let removed = store.remove_certificate(&fingerprint);
        assert!(removed.is_some());
        assert_eq!(store.count(), 0);
    }

    #[test]
    fn test_certificate_store_search() {
        let mut store = CertificateStore::new();
        let cert = create_test_certificate();
        store.add_certificate(cert);

        let by_subject = store.find_by_subject("example.com");
        assert_eq!(by_subject.len(), 1);

        let by_issuer = store.find_by_issuer("Example CA");
        assert_eq!(by_issuer.len(), 1);

        let expired = store.find_expired();
        // Our test certificate is valid from one hour ago to one year from now, so should not be expired
        assert_eq!(expired.len(), 0);
    }

    #[test]
    fn test_parse_certificate_pem() {
        let pem_data = "-----BEGIN CERTIFICATE-----\nMIICdummy...\n-----END CERTIFICATE-----";
        let result = parse_certificate_pem(pem_data);
        assert!(result.is_ok());

        let invalid_pem = "not a certificate";
        let result = parse_certificate_pem(invalid_pem);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_certificate_der() {
        let der_data = vec![0x30, 0x82, 0x02, 0x5c]; // Fake DER data
        let result = parse_certificate_der(&der_data);
        assert!(result.is_ok());

        let empty_der: Vec<u8> = vec![];
        let result = parse_certificate_der(&empty_der);
        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_fingerprint() {
        let cert_data = vec![0x30, 0x82, 0x02, 0x5c]; // Fake certificate data
        let result = calculate_fingerprint(&cert_data);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("placeholder"));

        let empty_data: Vec<u8> = vec![];
        let result = calculate_fingerprint(&empty_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_subject_field() {
        let subject_dn = "CN=example.com,O=Example Corp,C=US";

        assert_eq!(
            extract_subject_field(subject_dn, "CN"),
            Some("example.com".to_string())
        );
        assert_eq!(
            extract_subject_field(subject_dn, "O"),
            Some("Example Corp".to_string())
        );
        assert_eq!(
            extract_subject_field(subject_dn, "C"),
            Some("US".to_string())
        );
        assert_eq!(extract_subject_field(subject_dn, "L"), None);
    }

    #[test]
    fn test_certificate_field_extraction() {
        let cert = create_test_certificate();

        assert_eq!(get_common_name(&cert), Some("example.com".to_string()));
        assert_eq!(get_organization(&cert), Some("Example Corp".to_string()));
        assert_eq!(get_country(&cert), Some("US".to_string()));
    }

    #[test]
    fn test_format_certificate_info() {
        let cert = create_test_certificate();
        let formatted = format_certificate_info(&cert);

        assert!(formatted.contains("Subject: CN=example.com,O=Example Corp,C=US"));
        assert!(formatted.contains("Issuer: CN=Example CA,O=Example Corp,C=US"));
        assert!(formatted.contains("Version: 3"));
        assert!(formatted.contains("example.com, *.example.com"));
    }
}
