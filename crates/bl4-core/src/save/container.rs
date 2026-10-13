//! Encrypted save-container decoding and encoding.
//!
//! Container operations preserve exact YAML bytes. Document parsing and filesystem
//! persistence belong to higher layers.
use aes::Aes256;
use aes::cipher::{Array, BlockCipherDecrypt, BlockCipherEncrypt, KeyInit};
use flate2::{Compress, Compression, Decompress, FlushCompress, FlushDecompress, Status};
/// Key material for the supported save-container format.
const PUBLIC_KEY: [u8; 32] = [
    0x35, 0xEC, 0x33, 0x77, 0xF3, 0x5D, 0xB0, 0xEA, 0xBE, 0x6B, 0x83, 0x11, 0x54, 0x03, 0xEB, 0xFB,
    0x27, 0x25, 0x64, 0x2E, 0xD5, 0x49, 0x06, 0x29, 0x05, 0x78, 0xBD, 0x60, 0xBA, 0x4A, 0xA7, 0x87,
];
const MAX_DECOMPRESSED_SIZE: usize = 16 * 1024 * 1024;
const DECOMPRESSION_CHUNK_SIZE: usize = 8 * 1024;
const MAX_CIPHERTEXT_SIZE: usize = 32 * 1024 * 1024;
/// The platform controls account-key derivation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Steam,
    Epic,
}

/// Inputs needed by container operations. Paths and parsed YAML belong to
/// later session and document modules.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContainerContext {
    pub account_id: String,
    pub platform: Platform,
}

/// Decrypted bytes for the document parser to handle later.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedContainer {
    pub yaml_bytes: Vec<u8>,
    pub platform: Platform,
}

/// The zlib Adler-32 checksum followed by the uncompressed byte length.
///
/// The checksum is part of the zlib stream; only the final four-byte length
/// sits after it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Trailer {
    adler32: u32,
    uncompressed_len: u32,
}

/// Add specific validation errors as the operations are implemented.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContainerError {
    InvalidAccountId(&'static str),
    InvalidCiphertext(&'static str),
    InvalidPadding(&'static str),
    InvalidTrailer(&'static str),
    InvalidCompression(&'static str),
    IncompleteZlibStream {
        consumed: usize,
        total: usize,
        produced: usize,
    },
    DecompressedDataTooLarge(&'static str),
    InvalidTrailerChecksum(&'static str),
    InvalidTrailerLength(&'static str),
    NotImplemented(&'static str),
}

/// Check the account ID for the selected platform.
///
/// Steam IDs must fit in `u64` because key derivation uses eight bytes. Epic's
/// accepted syntax still needs real-save checks.
pub fn validate_user_id_format(user_id: &str, platform: Platform) -> Result<(), ContainerError> {
    let user_id = user_id.trim();

    if user_id.is_empty() {
        return Err(ContainerError::InvalidAccountId("User ID cannot be empty"));
    }

    let length = user_id.chars().count();
    match platform {
        Platform::Steam => {
            if !user_id.chars().all(|c| c.is_ascii_digit()) {
                return Err(ContainerError::InvalidAccountId(
                    "Steam ID must contain only ASCII digits",
                ));
            }
            if !(10..=20).contains(&length) {
                return Err(ContainerError::InvalidAccountId(
                    "Steam ID should contain 10-20 digits",
                ));
            }
            user_id.parse::<u64>().map_err(|_| {
                ContainerError::InvalidAccountId("Steam ID is too large for a 64-bit number")
            })?;
        }
        Platform::Epic => {
            if !user_id
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
            {
                return Err(ContainerError::InvalidAccountId(
                    "Epic ID contains invalid characters",
                ));
            }
            if !(10..=50).contains(&length) {
                return Err(ContainerError::InvalidAccountId(
                    "Epic ID should contain 10-50 characters",
                ));
            }
        }
    }

    Ok(())
}

/// Derive the AES-256 key by XORing account-specific bytes into `PUBLIC_KEY`.
/// Golden key fixtures are still required for both platforms.
pub fn derive_key(context: &ContainerContext) -> Result<[u8; 32], ContainerError> {
    validate_user_id_format(&context.account_id, context.platform)?;
    let mut key = PUBLIC_KEY;
    let account_id = context.account_id.trim();
    match context.platform {
        Platform::Steam => {
            let steam_id = account_id.parse::<u64>().map_err(|_| {
                ContainerError::InvalidAccountId("Steam ID is too large for a 64-bit number")
            })?;
            for (key_byte, id_byte) in key.iter_mut().zip(steam_id.to_le_bytes()) {
                *key_byte ^= id_byte;
            }
        }
        Platform::Epic => {
            let id_bytes = account_id.encode_utf16().flat_map(u16::to_le_bytes);

            for (key_byte, id_byte) in key.iter_mut().zip(id_bytes) {
                *key_byte ^= id_byte;
            }
        }
    }
    Ok(key)
}

/// Decrypt a save container and validate its padding, zlib payload and length.
/// Synthetic container fixtures are still required before this is accepted.
pub fn decode(
    ciphertext: &[u8],
    context: &ContainerContext,
) -> Result<DecodedContainer, ContainerError> {
    let key = derive_key(context)?;
    let decrypted = decrypt_blocks(ciphertext, &key)?;
    let unpadded = remove_pkcs7_padding(&decrypted)?;
    let (compressed, trailer) = split_trailer(unpadded)?;
    let yaml_bytes = decompress_zlib(compressed)?;
    verify_integrity(&yaml_bytes, trailer)?;
    Ok(DecodedContainer {
        yaml_bytes,
        platform: context.platform,
    })
}

/// Reconstruct a container from exact YAML bytes.
/// File writing, backups and source replacement belong to later modules.
pub fn encode(yaml_bytes: &[u8], context: &ContainerContext) -> Result<Vec<u8>, ContainerError> {
    let key = derive_key(context)?;
    let mut payload = compress_zlib(yaml_bytes)?;
    payload.extend_from_slice(&build_trailer(yaml_bytes)?);
    let padded = apply_pkcs7_padding(payload)?;
    encrypt_blocks(&padded, &key)
}

/// Decrypt validated AES-256-ECB ciphertext into padded container bytes.
fn decrypt_blocks(ciphertext: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, ContainerError> {
    if ciphertext.is_empty() {
        return Err(ContainerError::InvalidCiphertext("Ciphertext is empty"));
    }
    if !ciphertext.len().is_multiple_of(16) {
        return Err(ContainerError::InvalidCiphertext(
            "Ciphertext length is not a multiple of 16 bytes",
        ));
    }
    if ciphertext.len() > MAX_CIPHERTEXT_SIZE {
        return Err(ContainerError::InvalidCiphertext(
            "Ciphertext exceeds the maximum supported size",
        ));
    }
    let cipher = Aes256::new(&(*key).into());

    // ECB decrypts each block independently; padding is checked later.
    let mut decrypted = Vec::with_capacity(ciphertext.len());
    for chunk in ciphertext.as_chunks::<16>().0 {
        let mut block = Array::from([0u8; 16]);
        block.copy_from_slice(chunk);
        cipher.decrypt_block(&mut block);
        decrypted.extend_from_slice(&block);
    }

    Ok(decrypted)
}

/// Validate and remove PKCS#7 padding from decrypted bytes.
fn remove_pkcs7_padding(decrypted: &[u8]) -> Result<&[u8], ContainerError> {
    let Some(&last_byte) = decrypted.last() else {
        return Err(ContainerError::InvalidCiphertext("Decrypted data is empty"));
    };
    let padding_length = last_byte as usize;
    if !(1..=16).contains(&padding_length) {
        return Err(ContainerError::InvalidPadding(
            "PKCS#7 padding length must be between 1 and 16",
        ));
    }
    if decrypted.len() < padding_length {
        return Err(ContainerError::InvalidPadding(
            "Decrypted data is too short for the specified PKCS#7 padding length",
        ));
    }

    let padding = &decrypted[decrypted.len() - padding_length..];
    if !padding.iter().all(|&b| b == last_byte) {
        return Err(ContainerError::InvalidPadding(
            "PKCS#7 padding bytes do not match the expected value",
        ));
    }
    Ok(&decrypted[..decrypted.len() - padding_length])
}

/// Read the zlib checksum and final little-endian length field.
///
/// A real Steam save proves that the checksum occupies the normal final four
/// bytes of the zlib stream. Therefore decompression receives every byte up to
/// (but not including) the length field.
fn split_trailer(unpadded: &[u8]) -> Result<(&[u8], Trailer), ContainerError> {
    if unpadded.len() < 8 {
        return Err(ContainerError::InvalidCiphertext(
            "Unpadded data is too short to contain a trailer",
        ));
    }
    // The final eight bytes hold zlib's Adler-32 then the save's byte length.
    let trailer_bytes = &unpadded[unpadded.len() - 8..];

    // Adler-32 in a zlib stream is always stored in network (big-endian) order.
    let adler32 = u32::from_be_bytes(trailer_bytes[0..4].try_into().unwrap());

    let uncompressed_len = u32::from_le_bytes(trailer_bytes[4..8].try_into().unwrap());

    Ok((
        // Keep the checksum in the zlib input and remove only the length.
        &unpadded[..unpadded.len() - 4],
        Trailer {
            adler32,
            uncompressed_len,
        },
    ))
}

/// Decode exactly one bounded zlib stream and reject trailing data.
fn decompress_zlib(compressed: &[u8]) -> Result<Vec<u8>, ContainerError> {
    if compressed.is_empty() {
        return Err(ContainerError::InvalidCompression(
            "Compressed data is empty",
        ));
    }
    let mut decompressor = Decompress::new(true);
    let mut yaml_bytes = Vec::new();

    loop {
        let mut output_chunk = [0u8; DECOMPRESSION_CHUNK_SIZE];
        let available_output = (MAX_DECOMPRESSED_SIZE - yaml_bytes.len()).min(output_chunk.len());

        let input_start = decompressor.total_in() as usize;
        let output_start = decompressor.total_out();
        // Output size check is done before decompression to avoid unnecessary work if the limit is exceeded.
        if available_output == 0 {
            return Err(ContainerError::DecompressedDataTooLarge(
                "Decompressed data exceeds maximum allowed size",
            ));
        }

        // Once all compressed bytes were supplied, tell zlib this is the final
        // call. It can then distinguish a complete stream from truncated input.
        let flush = if input_start == compressed.len() {
            FlushDecompress::Finish
        } else {
            FlushDecompress::None
        };
        let status = decompressor
            .decompress(
                &compressed[input_start..],
                &mut output_chunk[..available_output],
                flush,
            )
            .map_err(|_| ContainerError::InvalidCompression("Zlib decompression failed"))?;
        let bytes_consumed = decompressor.total_in() as usize - input_start;
        let bytes_written = (decompressor.total_out() - output_start) as usize;
        yaml_bytes.extend_from_slice(&output_chunk[..bytes_written]);
        match status {
            Status::StreamEnd => {
                if decompressor.total_in() as usize != compressed.len() {
                    return Err(ContainerError::InvalidCompression(
                        "Extra data after zlib stream",
                    ));
                }
                return Ok(yaml_bytes);
            }
            // A decoder can return either status after consuming input or filling
            // the current output chunk. Continue while it made progress. A later
            // no-progress iteration detects truncated input safely.
            Status::Ok | Status::BufError => {
                if bytes_consumed == 0 && bytes_written == 0 {
                    return Err(ContainerError::IncompleteZlibStream {
                        consumed: input_start,
                        total: compressed.len(),
                        produced: yaml_bytes.len(),
                    });
                }
            }
        }
    }
}

/// Verify the decompressed YAML byte length and zlib Adler-32 checksum.
fn verify_integrity(yaml_bytes: &[u8], trailer: Trailer) -> Result<(), ContainerError> {
    // Check the uncompressed length against the trailer.
    let actual_length = u32::try_from(yaml_bytes.len()).map_err(|_| {
        ContainerError::InvalidTrailerLength("Uncompressed length is too large for a 32-bit number")
    });
    if actual_length? != trailer.uncompressed_len {
        return Err(ContainerError::InvalidTrailerLength(
            "Uncompressed length does not match trailer",
        ));
    }
    // Check the Adler-32 checksum against the trailer.
    let adler32 = adler2::adler32_slice(yaml_bytes);
    if adler32 != trailer.adler32 {
        return Err(ContainerError::InvalidTrailerChecksum(
            "Adler-32 checksum does not match trailer",
        ));
    }
    Ok(())
}

/// Compress the exact YAML bytes as one complete level-9 zlib stream.
fn compress_zlib(yaml_bytes: &[u8]) -> Result<Vec<u8>, ContainerError> {
    if yaml_bytes.len() > MAX_DECOMPRESSED_SIZE {
        return Err(ContainerError::DecompressedDataTooLarge(
            "YAML bytes exceed maximum allowed size",
        ));
    }

    let mut compressor = Compress::new(Compression::best(), true);
    let mut compressed = Vec::new();
    loop {
        let mut output_chunk = [0u8; DECOMPRESSION_CHUNK_SIZE];
        let input_start = compressor.total_in();
        let output_start = compressor.total_out();
        let flush = if compressor.total_in() as usize == yaml_bytes.len() {
            FlushCompress::Finish
        } else {
            FlushCompress::None
        };
        let status = compressor
            .compress(
                &yaml_bytes[compressor.total_in() as usize..],
                &mut output_chunk,
                flush,
            )
            .map_err(|_| ContainerError::InvalidCompression("Zlib compression failed"))?;
        let bytes_consumed = compressor.total_in() - input_start;
        let bytes_written = (compressor.total_out() - output_start) as usize;
        compressed.extend_from_slice(&output_chunk[..bytes_written]);
        match status {
            Status::StreamEnd => return Ok(compressed),
            Status::Ok | Status::BufError => {
                if bytes_consumed == 0 && bytes_written == 0 {
                    return Err(ContainerError::InvalidCompression(
                        "Zlib compression made no progress",
                    ));
                }
            }
        }
    }
}

/// Encode the uncompressed YAML byte length after a complete zlib stream.
fn build_trailer(yaml_bytes: &[u8]) -> Result<[u8; 4], ContainerError> {
    let uncompressed_len = u32::try_from(yaml_bytes.len()).map_err(|_| {
        ContainerError::InvalidTrailerLength("YAML byte length does not fit in a 32-bit field")
    })?;

    Ok(uncompressed_len.to_le_bytes())
}

/// Apply PKCS#7 padding, including a full block when the input is already aligned.
fn apply_pkcs7_padding(payload: Vec<u8>) -> Result<Vec<u8>, ContainerError> {
    let padding_length = 16 - (payload.len() % 16);
    let mut padded = payload;
    padded.extend(std::iter::repeat_n(padding_length as u8, padding_length));
    Ok(padded)
}

/// Encrypt bounded, block-aligned input using AES-256-ECB after padding.
fn encrypt_blocks(padded: &[u8], key: &[u8; 32]) -> Result<Vec<u8>, ContainerError> {
    if !padded.len().is_multiple_of(16) {
        return Err(ContainerError::InvalidCiphertext(
            "Padded data length is not a multiple of 16 bytes",
        ));
    }

    if padded.is_empty() {
        return Err(ContainerError::InvalidCiphertext("Padded data is empty"));
    }

    if padded.len() > MAX_CIPHERTEXT_SIZE {
        return Err(ContainerError::InvalidCiphertext(
            "Padded data exceeds the maximum supported size",
        ));
    }

    let cipher = Aes256::new(&(*key).into());

    let mut ciphertext = Vec::with_capacity(padded.len());
    for chunk in padded.as_chunks::<16>().0 {
        let mut block = Array::from([0u8; 16]);
        block.copy_from_slice(chunk);
        cipher.encrypt_block(&mut block);
        ciphertext.extend_from_slice(&block);
    }
    Ok(ciphertext)
}

#[cfg(test)]
#[path = "container_encoding_tests.rs"]
mod encoding_tests;

#[cfg(test)]
mod tests {
    use std::{env, fs, io::Write};

    use flate2::{Compression, write::ZlibEncoder};

    use super::{
        ContainerContext, ContainerError, DECOMPRESSION_CHUNK_SIZE, MAX_CIPHERTEXT_SIZE, Platform,
        Trailer, build_trailer, compress_zlib, decompress_zlib, decrypt_blocks, derive_key,
        remove_pkcs7_padding, split_trailer, validate_user_id_format, verify_integrity,
    };

    fn zlib_bytes(input: &[u8]) -> Vec<u8> {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(input).expect("compress test input");
        encoder.finish().expect("finish zlib test stream")
    }

    #[test]
    fn accepts_steam_id_with_surrounding_whitespace() {
        assert_eq!(
            validate_user_id_format(" 76561198000000000 ", Platform::Steam),
            Ok(())
        );
    }

    #[test]
    fn rejects_steam_number_that_does_not_fit_the_key_format() {
        assert_eq!(
            validate_user_id_format("18446744073709551616", Platform::Steam),
            Err(ContainerError::InvalidAccountId(
                "Steam ID is too large for a 64-bit number"
            ))
        );
    }

    #[test]
    fn checks_selected_platform_and_epic_characters() {
        assert!(validate_user_id_format("account-name_42", Platform::Epic).is_ok());
        assert!(validate_user_id_format("account-name_42", Platform::Steam).is_err());
        assert!(validate_user_id_format("account name 42", Platform::Epic).is_err());
    }

    #[test]
    fn decrypts_nist_aes_256_block() {
        // FIPS 197, Appendix C.3: independently published AES-256 example.
        let key: [u8; 32] = std::array::from_fn(|index| index as u8);
        let ciphertext = [
            0x8e, 0xa2, 0xb7, 0xca, 0x51, 0x67, 0x45, 0xbf, 0xea, 0xfc, 0x49, 0x90, 0x4b, 0x49,
            0x60, 0x89,
        ];
        let expected: Vec<u8> = (0x00..=0x0f).map(|byte| byte * 0x11).collect();
        assert_eq!(decrypt_blocks(&ciphertext, &key), Ok(expected));
    }

    #[test]
    fn rejects_ciphertext_that_is_not_a_full_block() {
        let key = [0u8; 32];
        assert!(matches!(
            decrypt_blocks(&[0u8; 15], &key),
            Err(ContainerError::InvalidCiphertext(_))
        ));
    }

    #[test]
    fn rejects_ciphertext_larger_than_the_configured_limit_before_decryption() {
        let key = [0u8; 32];
        let ciphertext = vec![0u8; MAX_CIPHERTEXT_SIZE + 16];

        assert_eq!(
            decrypt_blocks(&ciphertext, &key),
            Err(ContainerError::InvalidCiphertext(
                "Ciphertext exceeds the maximum supported size"
            ))
        );
    }

    #[test]
    fn removes_valid_pkcs7_padding() {
        assert_eq!(remove_pkcs7_padding(&[10, 20, 1]).unwrap(), &[10, 20]);
        assert_eq!(remove_pkcs7_padding(&[16; 16]).unwrap(), &[]);
    }

    #[test]
    fn rejects_invalid_pkcs7_padding() {
        assert!(matches!(
            remove_pkcs7_padding(&[]),
            Err(ContainerError::InvalidCiphertext(_))
        ));
        assert!(matches!(
            remove_pkcs7_padding(&[10, 20, 0]),
            Err(ContainerError::InvalidPadding(_))
        ));
        assert!(matches!(
            remove_pkcs7_padding(&[17]),
            Err(ContainerError::InvalidPadding(_))
        ));
        assert!(matches!(
            remove_pkcs7_padding(&[10, 20, 3, 3, 2]),
            Err(ContainerError::InvalidPadding(_))
        ));
        assert!(matches!(
            remove_pkcs7_padding(&[16]),
            Err(ContainerError::InvalidPadding(_))
        ));
    }

    #[test]
    fn keeps_zlib_checksum_and_removes_only_final_length() {
        let input = [0x78, 0x9c, 0x04, 0x03, 0x02, 0x01, 0x09, 0x00, 0x00, 0x00];
        let (compressed, trailer) = split_trailer(&input).unwrap();

        assert_eq!(compressed, &[0x78, 0x9c, 0x04, 0x03, 0x02, 0x01]);
        assert_eq!(trailer.adler32, 0x0403_0201);
        assert_eq!(trailer.uncompressed_len, 9);
    }

    #[test]
    fn accepts_a_complete_zlib_stream_followed_by_its_length() {
        let yaml = b"name: Vault Hunter\n";
        let mut input = zlib_bytes(yaml);
        input.extend_from_slice(&(yaml.len() as u32).to_le_bytes());

        let (compressed, trailer) = split_trailer(&input).unwrap();
        let decoded = decompress_zlib(compressed).unwrap();

        assert_eq!(decoded, yaml);
        assert_eq!(verify_integrity(&decoded, trailer), Ok(()));
    }

    #[test]
    fn builds_a_little_endian_yaml_length_field() {
        assert_eq!(build_trailer(b"hello"), Ok([5, 0, 0, 0]));
    }

    #[test]
    fn rejects_a_missing_trailer() {
        assert!(matches!(
            split_trailer(&[0; 7]),
            Err(ContainerError::InvalidCiphertext(_))
        ));
    }

    #[test]
    fn decompresses_one_complete_zlib_stream() {
        let yaml = b"name: Vault Hunter\nlevel: 12\n";
        assert_eq!(decompress_zlib(&zlib_bytes(yaml)).unwrap(), yaml);
    }

    #[test]
    fn compresses_a_zlib_stream_that_the_decoder_accepts() {
        let yaml = b"name: Vault Hunter\nlevel: 12\n";
        let compressed = compress_zlib(yaml).expect("compress test YAML");

        assert_eq!(decompress_zlib(&compressed).unwrap(), yaml);
    }

    #[test]
    fn decompresses_a_stream_larger_than_one_output_chunk() {
        let yaml = vec![b'a'; DECOMPRESSION_CHUNK_SIZE * 2 + 1];
        assert_eq!(decompress_zlib(&zlib_bytes(&yaml)).unwrap(), yaml);
    }

    #[test]
    fn rejects_trailing_bytes_after_a_zlib_stream() {
        let mut compressed = zlib_bytes(b"test");
        compressed.extend_from_slice(&[0xde, 0xad]);

        assert!(matches!(
            decompress_zlib(&compressed),
            Err(ContainerError::InvalidCompression(
                "Extra data after zlib stream"
            ))
        ));
    }

    #[test]
    fn rejects_invalid_zlib_data() {
        assert!(matches!(
            decompress_zlib(&[0x78, 0x9c, 0x00]),
            Err(ContainerError::InvalidCompression(_))
                | Err(ContainerError::IncompleteZlibStream { .. })
        ));
    }

    #[test]
    fn verifies_matching_trailer_values() {
        let yaml = b"name: Vault Hunter\n";
        let trailer = Trailer {
            adler32: adler2::adler32_slice(yaml),
            uncompressed_len: yaml.len() as u32,
        };

        assert_eq!(verify_integrity(yaml, trailer), Ok(()));
    }

    #[test]
    fn rejects_a_mismatched_trailer_length() {
        let yaml = b"name: Vault Hunter\n";
        let trailer = Trailer {
            adler32: adler2::adler32_slice(yaml),
            uncompressed_len: (yaml.len() - 1) as u32,
        };

        assert!(matches!(
            verify_integrity(yaml, trailer),
            Err(ContainerError::InvalidTrailerLength(_))
        ));
    }

    #[test]
    fn rejects_a_mismatched_trailer_checksum() {
        let yaml = b"name: Vault Hunter\n";
        let trailer = Trailer {
            adler32: 0,
            uncompressed_len: yaml.len() as u32,
        };

        assert!(matches!(
            verify_integrity(yaml, trailer),
            Err(ContainerError::InvalidTrailerChecksum(_))
        ));
    }

    #[test]
    #[ignore = "requires a private Steam save and only prints container metadata"]
    fn diagnose_private_steam_container_layout() {
        let save_path = env::var("BL4_SAVE_PATH").expect("Set BL4_SAVE_PATH to a copied save");
        let account_id = env::var("BL4_STEAM_ID").expect("Set BL4_STEAM_ID to Steam ID");
        let context = ContainerContext {
            account_id,
            platform: Platform::Steam,
        };

        let ciphertext = fs::read(save_path).expect("read copied save");
        let key = derive_key(&context).expect("derive Steam key");
        let decrypted = decrypt_blocks(&ciphertext, &key).expect("decrypt save");
        let unpadded = remove_pkcs7_padding(&decrypted).expect("remove PKCS#7 padding");
        let (compressed, trailer) = split_trailer(unpadded).expect("split trailer");

        println!(
            "ciphertext={}, decrypted={}, unpadded={}, compressed={}, expected_yaml_length={}",
            ciphertext.len(),
            decrypted.len(),
            unpadded.len(),
            compressed.len(),
            trailer.uncompressed_len,
        );

        let yaml_bytes = decompress_zlib(compressed).expect("decompress complete zlib stream");
        verify_integrity(&yaml_bytes, trailer).expect("validate zlib checksum and length");
        println!("verified_yaml_length={}", yaml_bytes.len());
    }
}
