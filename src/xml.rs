//! XML handling utilities for Subversion operations
//!
//! This module provides utilities for XML operations commonly used in
//! Subversion, such as creating XML headers and escaping XML content.

use crate::Error;
use std::collections::HashMap;

/// XML declaration and header information
#[derive(Debug, Clone)]
pub struct XmlHeader {
    /// XML version (default: "1.0")
    pub version: String,
    /// Character encoding (default: "UTF-8")  
    pub encoding: String,
    /// Standalone declaration
    pub standalone: Option<bool>,
}

impl Default for XmlHeader {
    fn default() -> Self {
        Self {
            version: "1.0".to_string(),
            encoding: "UTF-8".to_string(),
            standalone: None,
        }
    }
}

impl XmlHeader {
    /// Create XML header string
    pub fn to_string(&self) -> String {
        let mut header = format!(
            "<?xml version=\"{}\" encoding=\"{}\"",
            self.version, self.encoding
        );

        if let Some(standalone) = self.standalone {
            let standalone_str = if standalone { "yes" } else { "no" };
            header.push_str(&format!(" standalone=\"{}\"", standalone_str));
        }

        header.push_str("?>");
        header
    }
}

/// Create a standard XML header
pub fn make_header() -> String {
    XmlHeader::default().to_string()
}

/// Create a custom XML header
pub fn make_custom_header(version: &str, encoding: &str, standalone: Option<bool>) -> String {
    XmlHeader {
        version: version.to_string(),
        encoding: encoding.to_string(),
        standalone,
    }
    .to_string()
}

/// Escape special XML characters in text content
pub fn escape_text(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '&' => "&amp;".to_string(),
            '"' => "&quot;".to_string(),
            '\'' => "&apos;".to_string(),
            c => c.to_string(),
        })
        .collect()
}

/// Escape text for use in XML attributes
pub fn escape_attribute(text: &str) -> String {
    // For attributes, we need to escape quotes and other special chars
    text.chars()
        .map(|c| match c {
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '&' => "&amp;".to_string(),
            '"' => "&quot;".to_string(),
            '\'' => "&apos;".to_string(),
            '\n' => "&#10;".to_string(),
            '\r' => "&#13;".to_string(),
            '\t' => "&#9;".to_string(),
            c => c.to_string(),
        })
        .collect()
}

/// Simple XML element builder
#[derive(Debug, Clone)]
pub struct XmlElement {
    /// Element name/tag
    pub name: String,
    /// Element attributes
    pub attributes: HashMap<String, String>,
    /// Element text content
    pub content: Option<String>,
    /// Child elements
    pub children: Vec<XmlElement>,
    /// Whether this is a self-closing element
    pub self_closing: bool,
}

impl XmlElement {
    /// Create a new XML element
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            attributes: HashMap::new(),
            content: None,
            children: Vec::new(),
            self_closing: false,
        }
    }

    /// Add an attribute
    pub fn with_attribute(mut self, name: &str, value: &str) -> Self {
        self.attributes.insert(name.to_string(), value.to_string());
        self
    }

    /// Set text content
    pub fn with_content(mut self, content: &str) -> Self {
        self.content = Some(content.to_string());
        self
    }

    /// Add a child element
    pub fn with_child(mut self, child: XmlElement) -> Self {
        self.children.push(child);
        self
    }

    /// Mark as self-closing
    pub fn self_closing(mut self) -> Self {
        self.self_closing = true;
        self
    }

    /// Convert to XML string
    pub fn to_string(&self) -> String {
        let mut result = String::new();

        // Opening tag
        result.push('<');
        result.push_str(&self.name);

        // Attributes
        for (key, value) in &self.attributes {
            result.push_str(&format!(" {}=\"{}\"", key, escape_attribute(value)));
        }

        if self.self_closing {
            result.push_str("/>");
            return result;
        }

        result.push('>');

        // Content
        if let Some(content) = &self.content {
            result.push_str(&escape_text(content));
        }

        // Children
        for child in &self.children {
            result.push_str(&child.to_string());
        }

        // Closing tag
        result.push_str(&format!("</{}>", self.name));

        result
    }

    /// Convert to pretty-printed XML with indentation
    pub fn to_pretty_string(&self, indent_level: usize) -> String {
        let indent = "  ".repeat(indent_level);
        let mut result = String::new();

        // Opening tag
        result.push_str(&indent);
        result.push('<');
        result.push_str(&self.name);

        // Attributes
        for (key, value) in &self.attributes {
            result.push_str(&format!(" {}=\"{}\"", key, escape_attribute(value)));
        }

        if self.self_closing {
            result.push_str("/>\n");
            return result;
        }

        result.push('>');

        let has_children = !self.children.is_empty();
        let has_content = self.content.is_some();

        if has_children || (has_content && self.content.as_ref().unwrap().contains('\n')) {
            result.push('\n');
        }

        // Content
        if let Some(content) = &self.content {
            if has_children {
                result.push_str(&format!("{}  {}\n", indent, escape_text(content)));
            } else {
                result.push_str(&escape_text(content));
            }
        }

        // Children
        for child in &self.children {
            result.push_str(&child.to_pretty_string(indent_level + 1));
        }

        // Closing tag
        if has_children {
            result.push_str(&indent);
        }
        result.push_str(&format!("</{}>", self.name));

        if indent_level > 0 {
            result.push('\n');
        }

        result
    }
}

/// Parse basic XML elements (simplified parser for common cases)
///
/// Note: This is a very basic XML parser, not a full-featured one.
/// For complex XML parsing, consider using a dedicated XML library.
pub fn parse_simple_element(xml: &str) -> Result<XmlElement, Error> {
    let trimmed = xml.trim();

    // Check for self-closing tag
    if trimmed.ends_with("/>") {
        return parse_self_closing_tag(trimmed);
    }

    // Find the element name
    let start_tag_end = trimmed
        .find('>')
        .ok_or_else(|| Error::from_str("Invalid XML: no closing >"))?;
    let opening_tag = &trimmed[1..start_tag_end]; // Skip '<'

    // Parse element name and attributes
    let parts: Vec<&str> = opening_tag.split_whitespace().collect();
    let element_name = parts
        .first()
        .ok_or_else(|| Error::from_str("Invalid XML: no element name"))?;

    let mut element = XmlElement::new(element_name);

    // Simple attribute parsing (name="value")
    for part in parts.iter().skip(1) {
        if let Some(eq_pos) = part.find('=') {
            let attr_name = &part[..eq_pos];
            let attr_value = &part[eq_pos + 1..];
            // Remove quotes
            let cleaned_value = attr_value.trim_matches('"').trim_matches('\'');
            element = element.with_attribute(attr_name, cleaned_value);
        }
    }

    // Find closing tag
    let closing_tag = format!("</{}>", element_name);
    if let Some(closing_pos) = trimmed.rfind(&closing_tag) {
        // Extract content between tags
        let content_start = start_tag_end + 1;
        let content = &trimmed[content_start..closing_pos];

        if !content.trim().is_empty() {
            element = element.with_content(content.trim());
        }
    }

    Ok(element)
}

fn parse_self_closing_tag(tag: &str) -> Result<XmlElement, Error> {
    let inner = &tag[1..tag.len() - 2]; // Remove '< and '/>'
    let parts: Vec<&str> = inner.split_whitespace().collect();
    let element_name = parts
        .first()
        .ok_or_else(|| Error::from_str("Invalid XML: no element name"))?;

    let mut element = XmlElement::new(element_name).self_closing();

    // Parse attributes
    for part in parts.iter().skip(1) {
        if let Some(eq_pos) = part.find('=') {
            let attr_name = &part[..eq_pos];
            let attr_value = &part[eq_pos + 1..];
            let cleaned_value = attr_value.trim_matches('"').trim_matches('\'');
            element = element.with_attribute(attr_name, cleaned_value);
        }
    }

    Ok(element)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xml_header_default() {
        let header = make_header();
        assert_eq!(header, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
    }

    #[test]
    fn test_xml_header_custom() {
        let header = make_custom_header("1.1", "ISO-8859-1", Some(true));
        assert_eq!(
            header,
            "<?xml version=\"1.1\" encoding=\"ISO-8859-1\" standalone=\"yes\"?>"
        );
    }

    #[test]
    fn test_escape_text() {
        let text = "Hello <world> & \"friends\"!";
        let escaped = escape_text(text);
        assert_eq!(escaped, "Hello &lt;world&gt; &amp; &quot;friends&quot;!");
    }

    #[test]
    fn test_escape_attribute() {
        let attr = "value with \"quotes\" & <brackets>";
        let escaped = escape_attribute(attr);
        assert_eq!(
            escaped,
            "value with &quot;quotes&quot; &amp; &lt;brackets&gt;"
        );
    }

    #[test]
    fn test_xml_element_simple() {
        let element = XmlElement::new("test").with_content("Hello World");

        assert_eq!(element.to_string(), "<test>Hello World</test>");
    }

    #[test]
    fn test_xml_element_with_attributes() {
        let element = XmlElement::new("user")
            .with_attribute("id", "123")
            .with_attribute("name", "Alice")
            .with_content("User data");

        let xml = element.to_string();
        // Attributes can be in any order
        assert!(xml.contains("id=\"123\""));
        assert!(xml.contains("name=\"Alice\""));
        assert!(xml.starts_with("<user"));
        assert!(xml.ends_with(">User data</user>"));
    }

    #[test]
    fn test_xml_element_self_closing() {
        let element = XmlElement::new("br").self_closing();
        assert_eq!(element.to_string(), "<br/>");
    }

    #[test]
    fn test_xml_element_with_children() {
        let child = XmlElement::new("item").with_content("Child content");
        let parent = XmlElement::new("list").with_child(child);

        assert_eq!(
            parent.to_string(),
            "<list><item>Child content</item></list>"
        );
    }

    #[test]
    fn test_parse_simple_element() {
        let xml = "<test>Hello World</test>";
        let element = parse_simple_element(xml).unwrap();

        assert_eq!(element.name, "test");
        assert_eq!(element.content, Some("Hello World".to_string()));
    }

    #[test]
    fn test_parse_self_closing_element() {
        let xml = "<br/>";
        let element = parse_simple_element(xml).unwrap();

        assert_eq!(element.name, "br");
        assert!(element.self_closing);
        assert!(element.content.is_none());
    }

    #[test]
    fn test_parse_element_with_attributes() {
        let xml = r#"<user id="123" name="Alice">User data</user>"#;
        let element = parse_simple_element(xml).unwrap();

        assert_eq!(element.name, "user");
        assert_eq!(element.attributes.get("id"), Some(&"123".to_string()));
        assert_eq!(element.attributes.get("name"), Some(&"Alice".to_string()));
        assert_eq!(element.content, Some("User data".to_string()));
    }

    #[test]
    fn test_pretty_printing() {
        let child = XmlElement::new("item").with_content("Child content");
        let parent = XmlElement::new("list").with_child(child);

        let pretty = parent.to_pretty_string(0);

        // Should have proper indentation and newlines
        assert!(pretty.contains("  <item>"));
        assert!(pretty.contains("</list>"));
    }
}
