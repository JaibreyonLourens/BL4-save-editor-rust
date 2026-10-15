//! Read-only Borderlands 4 item-serial decoding.
//!
//! A serial is custom Base85 text with an `@U` marker. Decoding produces a
//! bit-packed token stream; it does not establish game-specific meanings for
//! header values or part indexes.

const BASE85_ALPHABET: &[u8; 85] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!#$%&()*+-;<=>?@^_`{/}~";
const MAX_DECODED_BYTES: usize = 4 * 1024;
const MAX_TOKENS: usize = 1024;
const MAX_PART_LIST_VALUES: usize = 256;
const MAX_STRING_BYTES: usize = 256;
const MAGIC: u32 = 0b001_0000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemSerial<'a> {
    text: &'a str,
}

impl<'a> ItemSerial<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { text }
    }

    pub fn as_str(self) -> &'a str {
        self.text
    }

    /// Decodes only the documented wire representation; it never guesses item
    /// names, categories, rarity, or the meaning of part indexes.
    pub fn decode(self) -> Result<DecodedItemSerial, ItemSerialError> {
        let encoded = self
            .text
            .strip_prefix("@U")
            .ok_or(ItemSerialError::InvalidPrefix)?;
        let bytes = decode_base85(encoded, MAX_DECODED_BYTES)?;
        let mirrored: Vec<u8> = bytes.into_iter().map(u8::reverse_bits).collect();
        parse_bitstream(&mirrored)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedItemSerial {
    pub tokens: Vec<SerialToken>,
    /// Zero bits after the final hard separator, used only for byte alignment.
    pub trailing_zero_bits: u8,
}

/// The repeating two-section shape observed in public weapon/equipment serial
/// vectors. It is structural: neither numeric section has game semantics here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TwoSectionItemLayout {
    pub first_section: [NumberToken; 4],
    pub second_section: [NumberToken; 2],
    pub parts: Vec<Part>,
}

impl TwoSectionItemLayout {
    /// The fourth value in the first section is an observed level candidate.
    /// Keep it separate from a verified game-level field until private fixture
    /// evidence establishes that semantics for each supported item family.
    pub fn level_candidate(&self) -> u32 {
        self.first_section[3].value
    }
}

impl DecodedItemSerial {
    /// Returns the known six-number/two-section layout when the complete token
    /// sequence matches it. Other structurally valid serials remain available
    /// as raw tokens rather than being forced into this shape.
    pub fn two_section_layout(&self) -> Option<TwoSectionItemLayout> {
        fn number(token: Option<&SerialToken>) -> Option<NumberToken> {
            match token {
                Some(SerialToken::Number(value)) => Some(*value),
                _ => None,
            }
        }
        fn separator(token: Option<&SerialToken>, expected: SeparatorKind) -> bool {
            matches!(token, Some(SerialToken::Separator(value)) if *value == expected)
        }

        let first_section = [
            number(self.tokens.first())?,
            number(self.tokens.get(2))?,
            number(self.tokens.get(4))?,
            number(self.tokens.get(6))?,
        ];
        let second_section = [number(self.tokens.get(8))?, number(self.tokens.get(10))?];
        if !separator(self.tokens.get(1), SeparatorKind::Soft)
            || !separator(self.tokens.get(3), SeparatorKind::Soft)
            || !separator(self.tokens.get(5), SeparatorKind::Soft)
            || !separator(self.tokens.get(7), SeparatorKind::Hard)
            || !separator(self.tokens.get(9), SeparatorKind::Soft)
            || !separator(self.tokens.get(11), SeparatorKind::Hard)
            || !separator(self.tokens.get(12), SeparatorKind::Hard)
        {
            return None;
        }

        let (final_token, parts) = self.tokens[13..].split_last()?;
        if !matches!(final_token, SerialToken::Separator(SeparatorKind::Hard)) {
            return None;
        }
        let parts = parts
            .iter()
            .map(|token| match token {
                SerialToken::Part(part) => Some(part.clone()),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;

        Some(TwoSectionItemLayout {
            first_section,
            second_section,
            parts,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SerialToken {
    Separator(SeparatorKind),
    Number(NumberToken),
    Part(Part),
    Text(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeparatorKind {
    /// `,` in the human-readable form.
    Soft,
    /// `|` in the human-readable form. The last one terminates the stream.
    Hard,
}

/// Keep the wire form: the game can normalize between these encodings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NumberToken {
    pub encoding: NumberEncoding,
    pub value: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumberEncoding {
    VarInt,
    VarBit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Part {
    pub index: u16,
    pub value: PartValue,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PartValue {
    None,
    Integer(u16),
    List(Vec<NumberToken>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemSerialError {
    InvalidPrefix,
    InvalidTextEncoding,
    Truncated,
    InvalidToken,
    ResourceLimit,
    UnsupportedVersion,
}

fn base85_digit(byte: u8) -> Result<u8, ItemSerialError> {
    BASE85_ALPHABET
        .iter()
        .position(|candidate| *candidate == byte)
        .map(|position| position as u8)
        .ok_or(ItemSerialError::InvalidTextEncoding)
}

/// Decodes BL4's custom, big-endian Base85 representation.
fn decode_base85(text: &str, output_limit: usize) -> Result<Vec<u8>, ItemSerialError> {
    let input = text.as_bytes();
    let remainder = input.len() % 5;
    if input.is_empty() || remainder == 1 {
        return Err(ItemSerialError::InvalidTextEncoding);
    }

    let output_len = (input.len() / 5) * 4 + remainder.saturating_sub(1);
    if output_len > output_limit {
        return Err(ItemSerialError::ResourceLimit);
    }

    let mut output = Vec::with_capacity(output_len);
    for chunk in input.chunks(5) {
        let bytes_to_write = if chunk.len() == 5 { 4 } else { chunk.len() - 1 };
        let mut value = 0_u64;
        for index in 0..5 {
            let digit = if index < chunk.len() {
                u64::from(base85_digit(chunk[index])?)
            } else {
                84 // Partial groups are high-padded by the last alphabet digit.
            };
            value = value * 85 + digit;
        }
        let value: u32 = value
            .try_into()
            .map_err(|_| ItemSerialError::InvalidTextEncoding)?;
        output.extend_from_slice(&value.to_be_bytes()[..bytes_to_write]);
    }
    Ok(output)
}

struct BitReader<'a> {
    bytes: &'a [u8],
    bit_index: usize,
}

impl<'a> BitReader<'a> {
    fn read_bit(&mut self) -> Result<u8, ItemSerialError> {
        let byte = *self
            .bytes
            .get(self.bit_index / 8)
            .ok_or(ItemSerialError::Truncated)?;
        let shift = 7 - (self.bit_index % 8);
        self.bit_index += 1;
        Ok((byte >> shift) & 1)
    }

    fn read_bits(&mut self, count: usize) -> Result<u32, ItemSerialError> {
        let mut value = 0;
        for _ in 0..count {
            value = (value << 1) | u32::from(self.read_bit()?);
        }
        Ok(value)
    }

    fn read_lsb_bits(&mut self, count: usize) -> Result<u32, ItemSerialError> {
        let mut value = 0;
        for shift in 0..count {
            value |= u32::from(self.read_bit()?) << shift;
        }
        Ok(value)
    }

    fn remaining_are_zero(&self) -> bool {
        (self.bit_index..self.bytes.len() * 8).all(|index| {
            let byte = self.bytes[index / 8];
            let shift = 7 - (index % 8);
            ((byte >> shift) & 1) == 0
        })
    }

    fn remaining_bits(&self) -> usize {
        self.bytes.len() * 8 - self.bit_index
    }
}

fn parse_bitstream(bytes: &[u8]) -> Result<DecodedItemSerial, ItemSerialError> {
    let mut bits = BitReader {
        bytes,
        bit_index: 0,
    };
    if bits.read_bits(7)? != MAGIC {
        return Err(ItemSerialError::InvalidToken);
    }

    let mut tokens = Vec::new();
    while tokens.len() < MAX_TOKENS {
        let token = parse_token(&mut bits)?;
        let is_final_separator = matches!(token, SerialToken::Separator(SeparatorKind::Hard))
            && bits.remaining_are_zero();
        tokens.push(token);
        if is_final_separator {
            return Ok(DecodedItemSerial {
                tokens,
                trailing_zero_bits: bits.remaining_bits() as u8,
            });
        }
    }
    Err(ItemSerialError::ResourceLimit)
}

fn parse_token(bits: &mut BitReader<'_>) -> Result<SerialToken, ItemSerialError> {
    match bits.read_bit()? {
        0 => match bits.read_bit()? {
            0 => Ok(SerialToken::Separator(SeparatorKind::Hard)),
            1 => Ok(SerialToken::Separator(SeparatorKind::Soft)),
            _ => unreachable!(),
        },
        1 => match bits.read_bits(2)? {
            0b00 => Ok(SerialToken::Number(parse_varint(bits)?)),
            0b01 => Ok(SerialToken::Part(parse_part(bits)?)),
            0b10 => Ok(SerialToken::Number(parse_varbit(bits)?)),
            0b11 => Ok(SerialToken::Text(parse_string(bits)?)),
            _ => unreachable!(),
        },
        _ => unreachable!(),
    }
}

fn parse_raw_varint(bits: &mut BitReader<'_>) -> Result<u16, ItemSerialError> {
    let mut value = 0_u16;
    for group in 0..4 {
        let nibble = bits.read_lsb_bits(4)? as u16;
        let continues = bits.read_bit()? == 1;
        value |= nibble << (group * 4);
        if !continues {
            return Ok(value);
        }
    }
    Err(ItemSerialError::InvalidToken)
}

fn parse_varint(bits: &mut BitReader<'_>) -> Result<NumberToken, ItemSerialError> {
    Ok(NumberToken {
        encoding: NumberEncoding::VarInt,
        value: u32::from(parse_raw_varint(bits)?),
    })
}

fn parse_varbit(bits: &mut BitReader<'_>) -> Result<NumberToken, ItemSerialError> {
    let bit_length = bits.read_lsb_bits(5)? as usize;
    Ok(NumberToken {
        encoding: NumberEncoding::VarBit,
        // Public vectors prove that a zero bit length can represent zero.
        value: bits.read_lsb_bits(bit_length)?,
    })
}

fn parse_part(bits: &mut BitReader<'_>) -> Result<Part, ItemSerialError> {
    let index = parse_raw_varint(bits)?;
    let value = if bits.read_bit()? == 1 {
        let value = parse_raw_varint(bits)?;
        if bits.read_bits(3)? != 0 {
            return Err(ItemSerialError::InvalidToken);
        }
        PartValue::Integer(value)
    } else {
        match bits.read_bits(2)? {
            0b10 => PartValue::None,
            0b01 => {
                // List subtype is followed by a separate soft-separator token.
                if bits.read_bits(2)? != 0b01 {
                    return Err(ItemSerialError::InvalidToken);
                }
                let mut values = Vec::new();
                loop {
                    match bits.read_bits(2)? {
                        0b00 => break,
                        0b01 => return Err(ItemSerialError::InvalidToken),
                        0b10 => match bits.read_bit()? {
                            0 => values.push(parse_varint(bits)?),
                            1 => return Err(ItemSerialError::InvalidToken),
                            _ => return Err(ItemSerialError::InvalidToken),
                        },
                        0b11 => match bits.read_bit()? {
                            0 => values.push(parse_varbit(bits)?),
                            1 => return Err(ItemSerialError::InvalidToken),
                            _ => return Err(ItemSerialError::InvalidToken),
                        },
                        _ => unreachable!(),
                    }
                    if values.len() > MAX_PART_LIST_VALUES {
                        return Err(ItemSerialError::ResourceLimit);
                    }
                }
                PartValue::List(values)
            }
            _ => return Err(ItemSerialError::InvalidToken),
        }
    };
    Ok(Part { index, value })
}

fn parse_string(bits: &mut BitReader<'_>) -> Result<String, ItemSerialError> {
    let length = usize::from(parse_raw_varint(bits)?);
    if length > MAX_STRING_BYTES {
        return Err(ItemSerialError::ResourceLimit);
    }
    let mut output = String::with_capacity(length);
    for _ in 0..length {
        let byte: u8 = bits
            .read_lsb_bits(7)?
            .try_into()
            .map_err(|_| ItemSerialError::InvalidToken)?;
        if !byte.is_ascii() {
            return Err(ItemSerialError::InvalidToken);
        }
        output.push(char::from(byte));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct BitBuilder {
        bits: Vec<u8>,
    }

    impl BitBuilder {
        fn push_bit(&mut self, bit: u8) {
            self.bits.push(bit);
        }

        fn push_bits(&mut self, value: u32, count: usize) {
            for shift in (0..count).rev() {
                self.push_bit(((value >> shift) & 1) as u8);
            }
        }

        fn push_lsb_bits(&mut self, value: u32, count: usize) {
            for shift in 0..count {
                self.push_bit(((value >> shift) & 1) as u8);
            }
        }

        fn push_raw_varint(&mut self, mut value: u16) {
            loop {
                self.push_lsb_bits(u32::from(value & 0x0f), 4);
                value >>= 4;
                self.push_bit(u8::from(value != 0));
                if value == 0 {
                    return;
                }
            }
        }

        fn push_varint(&mut self, value: u16) {
            self.push_bits(0b100, 3);
            self.push_raw_varint(value);
        }

        fn push_varbit(&mut self, value: u32, bit_length: usize) {
            self.push_bits(0b110, 3);
            self.push_lsb_bits(bit_length as u32, 5);
            self.push_lsb_bits(value, bit_length);
        }

        fn finish(mut self) -> Vec<u8> {
            while !self.bits.len().is_multiple_of(8) {
                self.push_bit(0);
            }
            self.bits
                .chunks(8)
                .map(|chunk| chunk.iter().fold(0_u8, |byte, bit| (byte << 1) | bit))
                .collect()
        }
    }

    fn displayed_tokens(serial: &DecodedItemSerial) -> Vec<String> {
        serial
            .tokens
            .iter()
            .map(|token| match token {
                SerialToken::Separator(SeparatorKind::Soft) => ",".into(),
                SerialToken::Separator(SeparatorKind::Hard) => "|".into(),
                SerialToken::Number(number) => number.value.to_string(),
                SerialToken::Part(part) => match &part.value {
                    PartValue::None => format!("{{{}}}", part.index),
                    PartValue::Integer(value) => format!("{{{}:{value}}}", part.index),
                    PartValue::List(values) => format!(
                        "{{{}:[{}]}}",
                        part.index,
                        values
                            .iter()
                            .map(|value| value.value.to_string())
                            .collect::<Vec<_>>()
                            .join(" ")
                    ),
                },
                SerialToken::Text(value) => format!("{value:?}"),
            })
            .collect()
    }

    #[test]
    fn base85_decodes_the_known_partial_final_group() {
        assert_eq!(
            decode_base85("gydOV%h><", 64),
            Ok(vec![0x84, 0xe4, 0x59, 0x5c, 0xcb, 0xd9, 0x00])
        );
    }

    #[test]
    fn base85_rejects_bad_text_and_resource_exhaustion() {
        assert_eq!(
            decode_base85("a", 64),
            Err(ItemSerialError::InvalidTextEncoding)
        );
        assert_eq!(
            decode_base85("!!!!!", 3),
            Err(ItemSerialError::ResourceLimit)
        );
        assert_eq!(
            decode_base85("abc~ ", 64),
            Err(ItemSerialError::InvalidTextEncoding)
        );
    }

    #[test]
    fn parses_every_supported_token_form_from_a_synthetic_bitstream() {
        let mut stream = BitBuilder::default();
        stream.push_bits(MAGIC, 7);
        stream.push_varint(60);
        stream.push_bits(0b01, 2);
        stream.push_varbit(2125, 12);
        stream.push_bits(0b111, 3);
        stream.push_raw_varint(2);
        stream.push_lsb_bits(u32::from(b'O'), 7);
        stream.push_lsb_bits(u32::from(b'K'), 7);
        // {19}
        stream.push_bits(0b101, 3);
        stream.push_raw_varint(19);
        stream.push_bit(0);
        stream.push_bits(0b10, 2);
        // {20:7}
        stream.push_bits(0b101, 3);
        stream.push_raw_varint(20);
        stream.push_bit(1);
        stream.push_raw_varint(7);
        stream.push_bits(0, 3);
        // {21:[1 2]}
        stream.push_bits(0b101, 3);
        stream.push_raw_varint(21);
        stream.push_bit(0);
        stream.push_bits(0b01, 2);
        stream.push_bits(0b01, 2); // Required list-start separator.
        stream.push_varint(1);
        stream.push_varbit(2, 2);
        stream.push_bits(0b00, 2); // End list.
        stream.push_bits(0b00, 2); // Final hard separator.

        assert_eq!(
            parse_bitstream(&stream.finish()),
            Ok(DecodedItemSerial {
                tokens: vec![
                    SerialToken::Number(NumberToken {
                        encoding: NumberEncoding::VarInt,
                        value: 60,
                    }),
                    SerialToken::Separator(SeparatorKind::Soft),
                    SerialToken::Number(NumberToken {
                        encoding: NumberEncoding::VarBit,
                        value: 2125,
                    }),
                    SerialToken::Text("OK".into()),
                    SerialToken::Part(Part {
                        index: 19,
                        value: PartValue::None,
                    }),
                    SerialToken::Part(Part {
                        index: 20,
                        value: PartValue::Integer(7),
                    }),
                    SerialToken::Part(Part {
                        index: 21,
                        value: PartValue::List(vec![
                            NumberToken {
                                encoding: NumberEncoding::VarInt,
                                value: 1,
                            },
                            NumberToken {
                                encoding: NumberEncoding::VarBit,
                                value: 2,
                            },
                        ]),
                    }),
                    SerialToken::Separator(SeparatorKind::Hard),
                ],
                trailing_zero_bits: 2,
            })
        );
    }

    #[test]
    fn parses_the_supplied_equipment_serial_structurally() {
        let decoded = ItemSerial::new("@Ugd77*FnkbUIz`YKRG/`es4}RMsG6vgs4=KLs6VJss9LCU2m")
            .decode()
            .expect("known public serial parses");
        assert_eq!(
            displayed_tokens(&decoded),
            [
                "7", ",", "0", ",", "1", ",", "60", "|", "2", ",", "2125", "|", "|", "{19}", "{2}",
                "{4}", "{3}", "{18}", "{66}", "{74}", "{67}", "{17}", "{29}", "{31}", "{32}",
                "{42}", "{50}", "|",
            ]
        );
        assert!(decoded.trailing_zero_bits <= 7);
        let layout = decoded.two_section_layout().expect("known layout");
        assert_eq!(
            layout.first_section.map(|number| number.value),
            [7, 0, 1, 60]
        );
        assert_eq!(layout.second_section.map(|number| number.value), [2, 2125]);
        assert_eq!(layout.level_candidate(), 60);
        assert_eq!(layout.parts.len(), 14);
    }

    #[test]
    fn parses_an_independent_public_weapon_vector_and_its_layout() {
        let decoded = ItemSerial::new("@Ugy3L+2}TYg%$yC%i7M2gZldO)@}cgb!l34$a-qf{00")
            .decode()
            .expect("known public serial parses");

        assert_eq!(
            displayed_tokens(&decoded),
            [
                "24", ",", "0", ",", "1", ",", "50", "|", "2", ",", "3379", "|", "|", "{76}",
                "{2}", "{3}", "{75}", "{57}", "{60}", "{59}", "{16}", "{25}", "{44}", "{49}", "|",
            ]
        );
        let layout = decoded.two_section_layout().expect("known layout");
        assert_eq!(
            layout.first_section.map(|number| number.value),
            [24, 0, 1, 50]
        );
        assert_eq!(layout.second_section.map(|number| number.value), [2, 3379]);
        assert_eq!(layout.level_candidate(), 50);
        assert_eq!(layout.parts.len(), 11);
    }

    #[test]
    fn requires_the_serial_marker() {
        assert_eq!(
            ItemSerial::new("gydOV%h><").decode(),
            Err(ItemSerialError::InvalidPrefix)
        );
    }

    #[test]
    fn public_serial_with_a_list_part_decodes_the_documented_values() {
        // Published serial-library vector, independently specified human form:
        // 267,0,1,22|2,274||{7}{1}{245:[23 39 69 79]}|
        let decoded = ItemSerial::new("@Ugr$WBm/$!m!X=5&qXxA;nj3OOD#<4R")
            .decode()
            .expect("public list-part vector");
        assert_eq!(
            displayed_tokens(&decoded),
            [
                "267",
                ",",
                "0",
                ",",
                "1",
                ",",
                "22",
                "|",
                "2",
                ",",
                "274",
                "|",
                "|",
                "{7}",
                "{1}",
                "{245:[23 39 69 79]}",
                "|",
            ]
        );
    }

    #[test]
    fn part_list_requires_its_start_separator() {
        let mut stream = BitBuilder::default();
        stream.push_raw_varint(21);
        stream.push_bit(0);
        stream.push_bits(0b01, 2); // Subtype.
        stream.push_varint(1); // Missing list-start separator.
        let bytes = stream.finish();
        assert_eq!(
            parse_part(&mut BitReader {
                bytes: &bytes,
                bit_index: 0
            }),
            Err(ItemSerialError::InvalidToken)
        );
    }
}
