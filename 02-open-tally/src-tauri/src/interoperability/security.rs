use crate::errors::OpenTallyError;

pub const MAX_XML_SIZE_BYTES: usize = 50 * 1024 * 1024; // 50 MB

/// Security pre-flight verification:
/// 1. Enforces strict input size bounds (preventing memory exhaustion).
/// 2. Decodes UTF-8 / UTF-16 (with BOM handling).
/// 3. Rejects XML external entities (XXE) and DTD entity expansions (Billion Laughs attacks).
#[allow(clippy::chunks_exact_to_as_chunks)]
pub fn sanitize_and_decode_xml(raw_bytes: &[u8]) -> Result<String, OpenTallyError> {
    if raw_bytes.len() > MAX_XML_SIZE_BYTES {
        return Err(OpenTallyError::Validation(format!(
            "XML input exceeds maximum allowed size of {} MB",
            MAX_XML_SIZE_BYTES / (1024 * 1024)
        )));
    }

    // Handle UTF-16 BOM if present
    let content = if raw_bytes.starts_with(&[0xFE, 0xFF]) {
        let u16s: Vec<u16> = raw_bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16(&u16s)
            .map_err(|e| OpenTallyError::Validation(format!("Invalid UTF-16BE encoding: {}", e)))?
    } else if raw_bytes.starts_with(&[0xFF, 0xFE]) {
        let u16s: Vec<u16> = raw_bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16(&u16s)
            .map_err(|e| OpenTallyError::Validation(format!("Invalid UTF-16LE encoding: {}", e)))?
    } else {
        let bytes = if raw_bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
            &raw_bytes[3..]
        } else {
            raw_bytes
        };
        std::str::from_utf8(bytes)
            .map_err(|e| OpenTallyError::Validation(format!("Invalid UTF-8 encoding: {}", e)))?
            .to_string()
    };

    // XXE & Billion Laughs prevention: Check for <!DOCTYPE, <!ENTITY
    let upper = content.to_ascii_uppercase();
    if upper.contains("<!DOCTYPE") || upper.contains("<!ENTITY") {
        return Err(OpenTallyError::Validation(
            "XML security violation: DTD declarations and external entities (<!DOCTYPE, <!ENTITY) are strictly prohibited"
                .to_string(),
        ));
    }

    Ok(content)
}

/// Escapes XML special characters for safe output serialization.
pub fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xxe_rejection() {
        let xxe = b"<?xml version=\"1.0\"?><!DOCTYPE foo [<!ENTITY xxe SYSTEM \"file:///etc/passwd\">]><foo>&xxe;</foo>";
        assert!(sanitize_and_decode_xml(xxe).is_err());
    }

    #[test]
    fn test_entity_expansion_rejection() {
        let billion_laughs =
            b"<?xml version=\"1.0\"?><!DOCTYPE lolz [<!ENTITY lol \"lol\">]><lolz>&lol;</lolz>";
        assert!(sanitize_and_decode_xml(billion_laughs).is_err());
    }

    #[test]
    fn test_utf8_bom_handling() {
        let mut with_bom = vec![0xEF, 0xBB, 0xBF];
        with_bom.extend_from_slice(b"<ENVELOPE><HEADER></HEADER></ENVELOPE>");
        let decoded = sanitize_and_decode_xml(&with_bom).unwrap();
        assert!(decoded.starts_with("<ENVELOPE>"));
    }
}
