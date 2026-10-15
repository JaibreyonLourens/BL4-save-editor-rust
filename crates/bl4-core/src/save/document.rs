use serde_yaml_ng::Value;
use yaml_rust2::{
    parser::{Event, Parser, Tag},
    scanner::{Scanner, TScalarStyle, Token, TokenType},
};

#[derive(Clone, Debug, PartialEq)]
pub struct SaveDocument {
    root: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentError {
    TooLarge,
    TooDeep,
    TooManyNodes,
    InvalidUtf8,
    EmptyDocument,
    InvalidYaml,
    UnsupportedStructure,
    SerializationFailed,
}

const MAX_DOCUMENT_BYTES: usize = 16 * 1024 * 1024;
const MAX_DOCUMENT_DEPTH: usize = 128;
const MAX_DOCUMENT_NODES: usize = 100_000;

fn validate_text(bytes: &[u8]) -> Result<&str, DocumentError> {
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(DocumentError::TooLarge);
    }

    let text = std::str::from_utf8(bytes).map_err(|_| DocumentError::InvalidUtf8)?;

    if text.trim().is_empty() {
        return Err(DocumentError::EmptyDocument);
    }

    Ok(text)
}

#[derive(Clone, Copy)]
enum NodeKind {
    Scalar,
    Sequence,
    Mapping,
}

/// Only tags whose semantics the generic tree preserves are supported.
fn validate_tag(tag: Option<&Tag>, kind: NodeKind) -> Result<(), DocumentError> {
    let Some(tag) = tag else { return Ok(()) };
    let full = format!("{}{}", tag.handle, tag.suffix);
    if full.starts_with('!') && full.len() > 1 {
        return Ok(()); // Local tags survive as Value::Tagged.
    }
    let valid = match kind {
        NodeKind::Scalar => matches!(
            full.as_str(),
            "tag:yaml.org,2002:str"
                | "tag:yaml.org,2002:int"
                | "tag:yaml.org,2002:float"
                | "tag:yaml.org,2002:bool"
                | "tag:yaml.org,2002:null"
        ),
        NodeKind::Sequence => full == "tag:yaml.org,2002:seq",
        NodeKind::Mapping => full == "tag:yaml.org,2002:map",
    };
    if valid {
        Ok(())
    } else {
        Err(DocumentError::UnsupportedStructure)
    }
}

/// Reject integer overflow before the YAML library can fall back to binary64.
/// Finite floating-point literals must neither overflow nor underflow to zero.
fn validate_number(
    text: &str,
    style: TScalarStyle,
    tag: Option<&Tag>,
) -> Result<(), DocumentError> {
    let full_tag = tag.map(|tag| format!("{}{}", tag.handle, tag.suffix));
    let explicit_float = full_tag.as_deref() == Some("tag:yaml.org,2002:float");
    let explicit_int = full_tag.as_deref() == Some("tag:yaml.org,2002:int");
    let inferred = style == TScalarStyle::Plain
        && (tag.is_none() || full_tag.as_ref().is_some_and(|tag| tag.starts_with('!')));
    if !(explicit_float || explicit_int || inferred) {
        return Ok(());
    }
    let negative = text.starts_with('-');
    let magnitude = text.strip_prefix(['+', '-']).unwrap_or(text);
    let (digits, radix) = if let Some(digits) = magnitude.strip_prefix("0x") {
        (digits, 16)
    } else if let Some(digits) = magnitude.strip_prefix("0o") {
        (digits, 8)
    } else if let Some(digits) = magnitude.strip_prefix("0b") {
        (digits, 2)
    } else {
        (magnitude, 10)
    };
    if !explicit_float && !digits.is_empty() && digits.chars().all(|c| c.is_digit(radix)) {
        let value =
            u64::from_str_radix(digits, radix).map_err(|_| DocumentError::UnsupportedStructure)?;
        if negative && value > (i64::MAX as u64) + 1 {
            return Err(DocumentError::UnsupportedStructure);
        }
    }
    if !explicit_int && let Ok(value) = text.parse::<f64>() {
        let mantissa = text.split(['e', 'E']).next().unwrap_or(text);
        // YAML's explicit .inf/.nan forms are supported. Rust also recognizes
        // nonnumeric words such as "inf"; leave their YAML resolution alone.
        if !mantissa.bytes().any(|c| c.is_ascii_digit()) {
            return Ok(());
        }
        let nonzero = mantissa.bytes().any(|c| matches!(c, b'1'..=b'9'));
        if !value.is_finite() || (value == 0.0 && nonzero) {
            return Err(DocumentError::UnsupportedStructure);
        }
    }
    Ok(())
}

fn reject_unsupported_yaml(text: &str) -> Result<(), DocumentError> {
    let mut parser = Parser::new_from_str(text);
    // None denotes a sequence; Some(bool) denotes a map expecting a key/value.
    let mut parents: Vec<Option<bool>> = Vec::new();
    let mut nodes = 0;
    let mut documents = 0;
    loop {
        let (event, _) = parser
            .next_token()
            .map_err(|_| DocumentError::InvalidYaml)?;
        let node_kind = match &event {
            Event::Scalar(_, _, _, _) => Some(NodeKind::Scalar),
            Event::SequenceStart(_, _) => Some(NodeKind::Sequence),
            Event::MappingStart(_, _) => Some(NodeKind::Mapping),
            _ => None,
        };
        if node_kind.is_some() {
            nodes += 1;
            if nodes > MAX_DOCUMENT_NODES {
                return Err(DocumentError::TooManyNodes);
            }
        }
        match event {
            Event::StreamEnd => break,
            Event::DocumentStart => {
                documents += 1;
                if documents > 1 {
                    return Err(DocumentError::InvalidYaml);
                }
            }
            Event::Alias(_) => return Err(DocumentError::UnsupportedStructure),
            Event::Scalar(value, style, _, tag) => {
                validate_tag(tag.as_ref(), NodeKind::Scalar)?;
                // The emitter does not keep << keys quoted, so even a literal
                // quoted key could acquire merge semantics on reconstruction.
                if parents.last() == Some(&Some(true)) && value == "<<" {
                    return Err(DocumentError::UnsupportedStructure);
                }
                validate_number(&value, style, tag.as_ref())?;
            }
            Event::SequenceStart(_, ref tag) | Event::MappingStart(_, ref tag) => {
                validate_tag(tag.as_ref(), node_kind.expect("collection node"))?;
                if parents.len() >= MAX_DOCUMENT_DEPTH {
                    return Err(DocumentError::TooDeep);
                }
            }
            Event::SequenceEnd | Event::MappingEnd => {
                parents.pop();
            }
            _ => {}
        }
        if let Some(kind) = node_kind {
            if let Some(Some(expecting_key)) = parents.last_mut() {
                *expecting_key = !*expecting_key;
            }
            match kind {
                NodeKind::Sequence => parents.push(None),
                NodeKind::Mapping => parents.push(Some(true)),
                NodeKind::Scalar => {}
            }
        }
    }
    Ok(())
}

/// The library emits numeric-overflow strings without quotes because it resolves
/// them as text. Our stricter input policy rejects those plain scalars. Quote
/// only those emitted tokens, using scanner positions rather than text searches
/// so identical text inside another scalar or a tag cannot be changed.
fn quote_unsupported_plain_numbers(yaml: &str) -> Result<String, DocumentError> {
    let mut positions = yaml.char_indices().enumerate();
    let mut scanner = Scanner::new(yaml.chars());
    let mut output = String::new();
    let mut copied = 0;
    while let Some(token) = scanner
        .next_token()
        .map_err(|_| DocumentError::SerializationFailed)?
    {
        if let Token(marker, TokenType::Scalar(TScalarStyle::Plain, value)) = token
            && validate_number(&value, TScalarStyle::Plain, None).is_err()
        {
            let (_, (start, _)) = positions
                .find(|(index, _)| *index == marker.index())
                .ok_or(DocumentError::SerializationFailed)?;
            let end = start
                .checked_add(value.len())
                .ok_or(DocumentError::SerializationFailed)?;
            if start < copied || yaml.get(start..end) != Some(value.as_str()) {
                return Err(DocumentError::SerializationFailed);
            }
            output.push_str(&yaml[copied..start]);
            output.push('\'');
            output.push_str(&value.replace('\'', "''"));
            output.push('\'');
            copied = end;
        }
    }
    output.push_str(&yaml[copied..]);
    Ok(output)
}

impl SaveDocument {
    pub fn parse(bytes: &[u8]) -> Result<Self, DocumentError> {
        let text = validate_text(bytes)?;

        reject_unsupported_yaml(text)?;

        let root: Value = serde_yaml_ng::from_str(text).map_err(|_| DocumentError::InvalidYaml)?;

        if !root.is_mapping() {
            return Err(DocumentError::UnsupportedStructure);
        }

        Ok(Self { root })
    }

    pub fn root(&self) -> &Value {
        &self.root
    }

    pub fn to_yaml_bytes(&self) -> Result<Vec<u8>, DocumentError> {
        let yaml =
            serde_yaml_ng::to_string(&self.root).map_err(|_| DocumentError::SerializationFailed)?;

        if yaml.len() > MAX_DOCUMENT_BYTES {
            return Err(DocumentError::TooLarge);
        }

        let yaml = quote_unsupported_plain_numbers(&yaml)?;

        if yaml.len() > MAX_DOCUMENT_BYTES {
            return Err(DocumentError::TooLarge);
        }

        Ok(yaml.into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_text_is_preserved_exactly() {
        let input = "name: 'h\u{00e9}llo'\n\n";

        assert_eq!(validate_text(input.as_bytes()), Ok(input));
    }

    #[test]
    fn parses_a_complete_yaml_tree() {
        let document = SaveDocument::parse(b"name: hero\nlevel: 42\nunknown: [true, null]\n");

        let document = document.expect("valid document");

        assert_eq!(
            document.root().get("name").and_then(Value::as_str),
            Some("hero")
        );
        assert_eq!(
            document.root().get("level").and_then(Value::as_u64),
            Some(42)
        );
        assert_eq!(
            document.root().get("unknown"),
            Some(&serde_yaml_ng::from_str("[true, null]").unwrap())
        );
    }

    #[test]
    fn malformed_yaml_is_rejected() {
        let result = SaveDocument::parse(b"name: [unfinished");

        assert!(matches!(result, Err(DocumentError::InvalidYaml)));
    }
    #[test]
    fn oversized_document_is_rejected() {
        let input = vec![b'x'; MAX_DOCUMENT_BYTES + 1];

        let result = SaveDocument::parse(&input);

        assert!(matches!(result, Err(DocumentError::TooLarge)));
    }

    #[test]
    fn invalid_utf8_is_rejected() {
        let input = [0xff, 0xfe];

        let result = SaveDocument::parse(&input);

        assert!(matches!(result, Err(DocumentError::InvalidUtf8)));
    }

    #[test]
    fn blank_document_is_rejected() {
        for input in [b"".as_slice(), b"   \n\t".as_slice()] {
            let result = SaveDocument::parse(input);

            assert!(matches!(result, Err(DocumentError::EmptyDocument)));
        }
    }

    #[test]
    fn non_mapping_root_is_rejected() {
        for input in [
            b"- first\n- second\n".as_slice(),
            b"42\n".as_slice(),
            b"'text'\n".as_slice(),
        ] {
            let result = SaveDocument::parse(input);

            assert!(matches!(result, Err(DocumentError::UnsupportedStructure)));
        }
    }
    #[test]
    fn parse_serialize_parse_preserves_the_complete_tree() {
        let input = concat!(
            "name: hero\n",
            "level: 42\n",
            "future: !Unknown\n",
            "  nested:\n",
            "    - true\n",
            "    - null\n",
            "    - '42'\n",
        )
        .as_bytes();

        let original = SaveDocument::parse(input).expect("parse original");
        let future = original.root().get("future").expect("future field exists");

        assert!(matches!(future, Value::Tagged(_)));

        let serialized = original.to_yaml_bytes().expect("serialize document");
        let recovered = SaveDocument::parse(&serialized).expect("parse serialized document");

        assert_eq!(recovered, original);
    }
    #[test]
    fn aliases_are_rejected_until_identity_can_be_preserved() {
        let input = b"\
    base: &shared
    value: 42
    copy: *shared
    ";

        let result = SaveDocument::parse(input);

        assert!(matches!(result, Err(DocumentError::UnsupportedStructure)));
    }

    #[test]
    fn alias_characters_inside_strings_are_allowed() {
        let document = SaveDocument::parse(
            br#"message: "use & and * as ordinary characters"
    "#,
        );

        assert!(document.is_ok());
    }

    #[test]
    fn excessive_nesting_is_rejected_before_building_the_value_tree() {
        let input = format!(
            "root: {}0{}\n",
            "[".repeat(MAX_DOCUMENT_DEPTH),
            "]".repeat(MAX_DOCUMENT_DEPTH),
        );

        let result = SaveDocument::parse(input.as_bytes());

        assert!(matches!(result, Err(DocumentError::TooDeep)));
    }

    #[test]
    fn excessive_node_count_is_rejected_before_building_the_value_tree() {
        let input = format!("root: [{}]\n", "0,".repeat(MAX_DOCUMENT_NODES));

        let result = SaveDocument::parse(input.as_bytes());

        assert!(matches!(result, Err(DocumentError::TooManyNodes)));
    }
}
