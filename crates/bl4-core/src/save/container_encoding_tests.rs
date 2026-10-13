//! Encoding requirements. Expected results come from published vectors, fixed
//! padding examples and the verified container layout, rather than encoder output.

use std::io::Read;

use aes::Aes256;
use aes::cipher::{BlockCipherDecrypt, KeyInit};
use flate2::bufread::ZlibDecoder;

use super::{
    ContainerContext, ContainerError, Platform, apply_pkcs7_padding, build_trailer, compress_zlib,
    decode, decrypt_blocks, encode, encrypt_blocks, remove_pkcs7_padding,
};

fn hex_bytes(hex: &str) -> Vec<u8> {
    assert!(hex.len().is_multiple_of(2));
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
        .collect()
}

#[test]
fn aes256_ecb_matches_all_four_nist_blocks_without_adding_padding() {
    // NIST SP 800-38A, F.1.5 ECB-AES256.Encrypt:
    // https://nvlpubs.nist.gov/nistpubs/Legacy/SP/nistspecialpublication800-38a.pdf
    let key: [u8; 32] = hex_bytes(concat!(
        "603deb1015ca71be2b73aef0857d7781",
        "1f352c073b6108d72d9810a30914dff4",
    ))
    .try_into()
    .unwrap();
    let plaintext = hex_bytes(concat!(
        "6bc1bee22e409f96e93d7e117393172a",
        "ae2d8a571e03ac9c9eb76fac45af8e51",
        "30c81c46a35ce411e5fbc1191a0a52ef",
        "f69f2445df4f9b17ad2b417be66c3710",
    ));
    let expected = hex_bytes(concat!(
        "f3eed1bdb5d2a03c064b5a7e3db181f8",
        "591ccb10d410ed26dc5ba74a31362870",
        "b6ed21b99ca6f4f9f153e7b1beafed1d",
        "23304b7a39f9f3ff067d8d8f9e24ecc7",
    ));

    assert_eq!(encrypt_blocks(&plaintext, &key).unwrap(), expected);
}

#[test]
fn encryption_rejects_empty_input() {
    assert!(matches!(
        encrypt_blocks(&[], &[0; 32]),
        Err(ContainerError::InvalidCiphertext(_))
    ));
}

#[test]
fn encryption_rejects_every_partial_block_length() {
    for length in (1..=47).filter(|length| length % 16 != 0) {
        assert!(
            matches!(
                encrypt_blocks(&vec![0; length], &[0; 32]),
                Err(ContainerError::InvalidCiphertext(_))
            ),
            "accepted {length} bytes"
        );
    }
}

#[test]
fn encryption_rejects_input_above_the_32_mib_policy() {
    let input = vec![0; 32 * 1024 * 1024 + 16];
    assert!(matches!(
        encrypt_blocks(&input, &[0; 32]),
        Err(ContainerError::InvalidCiphertext(_))
    ));
}

#[test]
fn encryption_and_decryption_preserve_multiple_blocks_exactly() {
    let input: Vec<u8> = (0..=255).collect();
    let key = [0x42; 32];
    let ciphertext = encrypt_blocks(&input, &key).unwrap();

    assert_eq!(ciphertext.len(), input.len());
    assert_eq!(decrypt_blocks(&ciphertext, &key).unwrap(), input);
}

#[test]
fn pkcs7_matches_fixed_examples_including_empty_and_aligned_input() {
    // 14 bytes need two 0x02 bytes. Already aligned input needs 16 0x10 bytes.
    let cases: &[(&[u8], &[u8])] = &[
        (b"", &[0x10; 16]),
        (b"12345678901234", b"12345678901234\x02\x02"),
        (b"123456789012345", b"123456789012345\x01"),
        (
            b"1234567890123456",
            b"1234567890123456\x10\x10\x10\x10\x10\x10\x10\x10\x10\x10\x10\x10\x10\x10\x10\x10",
        ),
    ];

    for &(input, expected) in cases {
        assert_eq!(apply_pkcs7_padding(input.to_vec()).unwrap(), expected);
    }
}

#[test]
fn padding_and_removal_preserve_payloads_at_every_block_offset() {
    for length in 0..=64 {
        let input = vec![0x10; length];
        let padded = apply_pkcs7_padding(input.clone()).unwrap();
        assert!(padded.len().is_multiple_of(16));
        assert!(padded.len() > input.len());
        assert_eq!(remove_pkcs7_padding(&padded).unwrap(), input);
    }
}

#[test]
fn length_field_is_four_little_endian_bytes_and_counts_utf8_bytes() {
    assert_eq!(build_trailer(b""), Ok([0, 0, 0, 0]));
    assert_eq!(build_trailer(&vec![0; 258]), Ok([0x02, 0x01, 0, 0]));
    assert_eq!(build_trailer("name: Zoë\n".as_bytes()), Ok([11, 0, 0, 0]));
}

#[test]
fn compressor_produces_complete_zlib_readable_by_a_separate_reader() {
    // Large varied input exercises multiple output buffers as well as empty input.
    let mut state = 0x1234_5678_u32;
    let varied: Vec<u8> = (0..65_537)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect();
    for input in [b"".as_slice(), b"name: Hunter\n", varied.as_slice()] {
        let compressed = compress_zlib(input).unwrap();
        let mut reader = ZlibDecoder::new(compressed.as_slice());
        let mut actual = Vec::new();
        reader.read_to_end(&mut actual).unwrap();

        assert_eq!(actual, input);
        assert_eq!(reader.total_in() as usize, compressed.len());
    }
}

#[test]
fn encoding_rejects_yaml_above_the_16_mib_policy() {
    let context = ContainerContext {
        account_id: "72623859790382856".into(),
        platform: Platform::Steam,
    };
    assert!(matches!(
        encode(&vec![0; 16 * 1024 * 1024 + 1], &context),
        Err(ContainerError::DecompressedDataTooLarge(_))
    ));
}

#[test]
fn encoded_steam_container_has_only_zlib_length_and_pkcs7_padding() {
    let context = ContainerContext {
        // Synthetic numeric input 0x0102030405060708, not a private account ID.
        account_id: "72623859790382856".into(),
        platform: Platform::Steam,
    };
    // Expected key calculated from the documented little-endian XOR rule.
    // Do not use derive_key here: a matching derivation bug could hide an error.
    let key: [u8; 32] = hex_bytes(concat!(
        "3deb3572f75eb2ebbe6b83115403ebfb",
        "2725642ed54906290578bd60ba4aa787",
    ))
    .try_into()
    .unwrap();
    let yaml = "name: Zoë\n".as_bytes();
    let mut plaintext = encode(yaml, &context).unwrap();
    assert!(!plaintext.is_empty());
    assert!(plaintext.len().is_multiple_of(16));

    // Inspect the wire format directly, bypassing all production decode helpers.
    let cipher = Aes256::new(&key.into());
    for block in plaintext.as_chunks_mut::<16>().0 {
        cipher.decrypt_block(block.into());
    }
    let padding_length = usize::from(*plaintext.last().unwrap());
    assert!((1..=16).contains(&padding_length));
    let data_end = plaintext.len() - padding_length;
    assert!(
        plaintext[data_end..]
            .iter()
            .all(|&byte| usize::from(byte) == padding_length)
    );
    let stream_end = data_end - 4;
    assert_eq!(&plaintext[stream_end..data_end], &[11, 0, 0, 0]);

    let compressed = &plaintext[..stream_end];
    let mut reader = ZlibDecoder::new(compressed);
    let mut actual = Vec::new();
    reader.read_to_end(&mut actual).unwrap();
    assert_eq!(actual, yaml);
    // An extra checksum or second stream would leave bytes unconsumed here.
    assert_eq!(reader.total_in() as usize, compressed.len());
}

#[test]
fn complete_round_trips_preserve_exact_bytes_for_synthetic_platforms() {
    let large_yaml = format!("description: {}\n", "Vault Hunter ".repeat(4096));
    for context in [
        ContainerContext {
            account_id: "72623859790382856".into(),
            platform: Platform::Steam,
        },
        ContainerContext {
            account_id: "synthetic-epic-id".into(),
            platform: Platform::Epic,
        },
    ] {
        // These test byte preservation; accepting empty bytes does not claim that
        // empty YAML is a playable save or that genuine Epic saves are verified.
        for yaml in [
            b"".as_slice(),
            "name: Zoë\n".as_bytes(),
            large_yaml.as_bytes(),
        ] {
            let ciphertext = encode(yaml, &context).unwrap();
            let decoded = decode(&ciphertext, &context).unwrap();
            assert_eq!(decoded.yaml_bytes, yaml);
            assert_eq!(decoded.platform, context.platform);
        }
    }
}
