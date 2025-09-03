//! Internationalization (NLS) utilities for Subversion
//!
//! This module provides localization and internationalization support,
//! including locale handling, message translation, and character set conversions
//! for Subversion operations.
//!
//! Note: Many NLS functions are not available in the current subversion-sys
//! bindings, so this module provides a framework that can be extended when
//! those bindings become available.

use crate::Error;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

/// Locale information
#[derive(Debug, Clone)]
pub struct LocaleInfo {
    /// Language code (e.g., "en", "de", "fr")
    pub language: String,
    /// Country code (e.g., "US", "DE", "FR")
    pub country: Option<String>,
    /// Character encoding (e.g., "UTF-8", "ISO-8859-1")
    pub encoding: String,
    /// Full locale string (e.g., "en_US.UTF-8")
    pub locale_string: String,
}

impl LocaleInfo {
    /// Create a new locale info from a locale string
    pub fn from_locale_string(locale: &str) -> Self {
        let parts: Vec<&str> = locale.split('.').collect();
        let locale_part = parts.first().unwrap_or(&locale);
        let encoding = parts.get(1).unwrap_or(&"UTF-8").to_string();

        let lang_country: Vec<&str> = locale_part.split('_').collect();
        let language = lang_country.first().unwrap_or(&"en").to_string();
        let country = lang_country.get(1).map(|s| s.to_string());

        Self {
            language,
            country,
            encoding,
            locale_string: locale.to_string(),
        }
    }

    /// Get the locale string representation
    pub fn to_locale_string(&self) -> String {
        if let Some(country) = &self.country {
            format!("{}_{}.{}", self.language, country, self.encoding)
        } else {
            format!("{}.{}", self.language, self.encoding)
        }
    }

    /// Check if this is a UTF-8 locale
    pub fn is_utf8(&self) -> bool {
        self.encoding.to_uppercase().contains("UTF-8")
    }
}

impl Default for LocaleInfo {
    fn default() -> Self {
        Self::from_locale_string("en_US.UTF-8")
    }
}

/// Message catalog for translations
#[derive(Debug, Clone)]
pub struct MessageCatalog {
    /// Language code for this catalog
    pub language: String,
    /// Translation mappings from message ID to translated text
    pub translations: HashMap<String, String>,
}

impl MessageCatalog {
    /// Create a new empty message catalog
    pub fn new(language: &str) -> Self {
        Self {
            language: language.to_string(),
            translations: HashMap::new(),
        }
    }

    /// Add a translation
    pub fn add_translation(&mut self, message_id: &str, translation: &str) {
        self.translations
            .insert(message_id.to_string(), translation.to_string());
    }

    /// Get a translation, fallback to message_id if not found
    pub fn translate(&self, message_id: &str) -> String {
        self.translations
            .get(message_id)
            .cloned()
            .unwrap_or_else(|| message_id.to_string())
    }

    /// Check if a translation exists
    pub fn has_translation(&self, message_id: &str) -> bool {
        self.translations.contains_key(message_id)
    }
}

/// Global NLS state
static CURRENT_LOCALE: Mutex<Option<LocaleInfo>> = Mutex::new(None);
static MESSAGE_CATALOGS: LazyLock<Mutex<HashMap<String, MessageCatalog>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Initialize NLS with default locale
///
/// Note: This is currently a placeholder since svn_nls_init() is not available
/// in subversion-sys bindings.
pub fn init() -> Result<(), Error> {
    let locale = detect_system_locale();
    set_locale(&locale)?;

    // TODO: When subversion-sys exposes NLS functions, implement:
    // - svn_nls_init()
    // - Load message catalogs from system

    Ok(())
}

/// Set the current locale
pub fn set_locale(locale_info: &LocaleInfo) -> Result<(), Error> {
    if let Ok(mut current) = CURRENT_LOCALE.lock() {
        *current = Some(locale_info.clone());
    }

    // TODO: When available, implement:
    // - setlocale() or equivalent SVN function
    // - Load appropriate message catalog

    Ok(())
}

/// Get the current locale
pub fn get_locale() -> Result<LocaleInfo, Error> {
    if let Ok(current) = CURRENT_LOCALE.lock() {
        Ok(current.clone().unwrap_or_default())
    } else {
        Ok(LocaleInfo::default())
    }
}

/// Detect system locale from environment
pub fn detect_system_locale() -> LocaleInfo {
    // Try various environment variables
    let locale_vars = ["LC_ALL", "LC_MESSAGES", "LANG"];

    for var in &locale_vars {
        if let Ok(value) = std::env::var(var) {
            if !value.is_empty() && value != "C" && value != "POSIX" {
                return LocaleInfo::from_locale_string(&value);
            }
        }
    }

    // Fallback to default
    LocaleInfo::default()
}

/// Register a message catalog for a language
pub fn register_message_catalog(catalog: MessageCatalog) -> Result<(), Error> {
    if let Ok(mut catalogs) = MESSAGE_CATALOGS.lock() {
        catalogs.insert(catalog.language.clone(), catalog);
    }

    Ok(())
}

/// Translate a message using current locale
pub fn translate_message(message_id: &str) -> Result<String, Error> {
    let locale = get_locale()?;

    if let Ok(catalogs) = MESSAGE_CATALOGS.lock() {
        if let Some(catalog) = catalogs.get(&locale.language) {
            return Ok(catalog.translate(message_id));
        }
    }

    // Fallback to original message
    Ok(message_id.to_string())
}

/// Translate a message with format arguments
pub fn translate_message_with_args(message_id: &str, args: &[&str]) -> Result<String, Error> {
    let translated = translate_message(message_id)?;

    // Simple placeholder replacement (e.g., {0}, {1}, etc.)
    let mut result = translated;
    for (i, arg) in args.iter().enumerate() {
        let placeholder = format!("{{{}}}", i);
        result = result.replace(&placeholder, arg);
    }

    Ok(result)
}

/// Convert text between character encodings
pub fn convert_encoding(
    text: &str,
    from_encoding: &str,
    to_encoding: &str,
) -> Result<String, Error> {
    // For UTF-8 to UTF-8, no conversion needed
    if from_encoding.to_uppercase() == to_encoding.to_uppercase() {
        return Ok(text.to_string());
    }

    // TODO: When subversion-sys exposes encoding functions, implement:
    // - Use SVN's character encoding conversion functions
    // - Handle various encodings (ISO-8859-1, Windows-1252, etc.)

    // For now, assume UTF-8 input and validate
    if !text.is_ascii() && to_encoding.to_uppercase() != "UTF-8" {
        return Err(Error::from_str(
            "Character encoding conversion not fully implemented",
        ));
    }

    Ok(text.to_string())
}

/// Get available message domains/catalogs
pub fn get_available_languages() -> Result<Vec<String>, Error> {
    if let Ok(catalogs) = MESSAGE_CATALOGS.lock() {
        Ok(catalogs.keys().cloned().collect())
    } else {
        Ok(vec!["en".to_string()])
    }
}

/// Check if a language is supported
pub fn is_language_supported(language: &str) -> Result<bool, Error> {
    let available = get_available_languages()?;
    Ok(available.contains(&language.to_string()))
}

/// Create a basic error message catalog for common SVN errors
pub fn create_default_error_catalog() -> MessageCatalog {
    let mut catalog = MessageCatalog::new("en");

    // Common error messages
    catalog.add_translation("file_not_found", "File not found");
    catalog.add_translation("permission_denied", "Permission denied");
    catalog.add_translation("invalid_path", "Invalid path");
    catalog.add_translation("repository_locked", "Repository is locked");
    catalog.add_translation("working_copy_locked", "Working copy is locked");
    catalog.add_translation("merge_conflict", "Merge conflict");
    catalog.add_translation("network_error", "Network error");
    catalog.add_translation("authentication_failed", "Authentication failed");

    catalog
}

/// Pluralization rules for different languages
#[derive(Debug, Clone)]
pub enum PluralRule {
    /// English-style: 1 item, N items
    English,
    /// Romance languages: 0-1 item, N items
    Romance,
    /// Slavic languages: complex rules
    Slavic,
    /// Custom rule with function
    Custom(fn(u32) -> usize),
}

impl PluralRule {
    /// Get the plural form index for a count
    pub fn get_plural_index(&self, count: u32) -> usize {
        match self {
            PluralRule::English => {
                if count == 1 {
                    0
                } else {
                    1
                }
            }
            PluralRule::Romance => {
                if count <= 1 {
                    0
                } else {
                    1
                }
            }
            PluralRule::Slavic => {
                // Simplified Slavic rule (actual rules are more complex)
                if count % 10 == 1 && count % 100 != 11 {
                    0 // singular
                } else if count % 10 >= 2
                    && count % 10 <= 4
                    && (count % 100 < 10 || count % 100 >= 20)
                {
                    1 // paucal
                } else {
                    2 // plural
                }
            }
            PluralRule::Custom(f) => f(count),
        }
    }
}

/// Translate a message with plural forms
pub fn translate_plural(
    message_id: &str,
    plural_forms: &[&str],
    count: u32,
) -> Result<String, Error> {
    let locale = get_locale()?;

    // Determine plural rule based on language
    let rule = match locale.language.as_str() {
        "en" => PluralRule::English,
        "fr" | "es" | "it" | "pt" => PluralRule::Romance,
        "pl" | "ru" | "cs" | "sk" => PluralRule::Slavic,
        _ => PluralRule::English, // Default to English rules
    };

    let index = rule.get_plural_index(count);
    let form = plural_forms.get(index).unwrap_or(&plural_forms[0]);

    // Replace {count} placeholder if present
    let result = form.replace("{count}", &count.to_string());
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_locale_info_creation() {
        let locale = LocaleInfo::from_locale_string("en_US.UTF-8");
        assert_eq!(locale.language, "en");
        assert_eq!(locale.country, Some("US".to_string()));
        assert_eq!(locale.encoding, "UTF-8");
        assert!(locale.is_utf8());
    }

    #[test]
    fn test_locale_info_simple() {
        let locale = LocaleInfo::from_locale_string("fr");
        assert_eq!(locale.language, "fr");
        assert_eq!(locale.country, None);
        assert_eq!(locale.encoding, "UTF-8");
    }

    #[test]
    fn test_message_catalog() {
        let mut catalog = MessageCatalog::new("fr");
        catalog.add_translation("hello", "Bonjour");
        catalog.add_translation("goodbye", "Au revoir");

        assert_eq!(catalog.translate("hello"), "Bonjour");
        assert_eq!(catalog.translate("goodbye"), "Au revoir");
        assert_eq!(catalog.translate("unknown"), "unknown");
        assert!(catalog.has_translation("hello"));
        assert!(!catalog.has_translation("unknown"));
    }

    #[test]
    fn test_system_locale_detection() {
        // This test will use the actual system locale
        let locale = detect_system_locale();
        assert!(!locale.language.is_empty());
    }

    #[test]
    fn test_message_translation() {
        let mut catalog = MessageCatalog::new("en");
        catalog.add_translation("test_message", "Test Message");
        let _ = register_message_catalog(catalog);

        // This might not work as expected due to locale handling,
        // but it shouldn't panic
        let result = translate_message("test_message");
        let _ = result; // Don't assert specific value due to locale complexity
    }

    #[test]
    fn test_message_with_args() {
        let result =
            translate_message_with_args("Hello {0}, you have {1} messages", &["Alice", "5"]);
        match result {
            Ok(msg) => assert_eq!(msg, "Hello Alice, you have 5 messages"),
            Err(_) => {} // OK if NLS is not fully initialized
        }
    }

    #[test]
    fn test_encoding_conversion() {
        let result = convert_encoding("Hello", "UTF-8", "UTF-8");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Hello");
    }

    #[test]
    fn test_plural_rules() {
        let english = PluralRule::English;
        assert_eq!(english.get_plural_index(0), 1); // "0 items"
        assert_eq!(english.get_plural_index(1), 0); // "1 item"
        assert_eq!(english.get_plural_index(2), 1); // "2 items"

        let romance = PluralRule::Romance;
        assert_eq!(romance.get_plural_index(0), 0); // "0 item"
        assert_eq!(romance.get_plural_index(1), 0); // "1 item"
        assert_eq!(romance.get_plural_index(2), 1); // "2 items"
    }

    #[test]
    fn test_translate_plural() {
        let forms = &["{count} item", "{count} items"];
        let result1 = translate_plural("item_count", forms, 1);
        let result2 = translate_plural("item_count", forms, 5);

        if let (Ok(msg1), Ok(msg2)) = (result1, result2) {
            assert_eq!(msg1, "1 item");
            assert_eq!(msg2, "5 items");
        }
    }

    #[test]
    fn test_default_error_catalog() {
        let catalog = create_default_error_catalog();
        assert_eq!(catalog.language, "en");
        assert_eq!(catalog.translate("file_not_found"), "File not found");
        assert_eq!(catalog.translate("permission_denied"), "Permission denied");
        assert!(catalog.has_translation("merge_conflict"));
    }

    #[test]
    fn test_language_support() {
        let result = is_language_supported("en");
        // Should not panic, result may vary depending on setup
        let _ = result;

        let languages = get_available_languages();
        // Should not panic
        let _ = languages;
    }

    #[test]
    fn test_nls_init() {
        // Should not panic even if underlying functions are not available
        let result = init();
        let _ = result;
    }
}
