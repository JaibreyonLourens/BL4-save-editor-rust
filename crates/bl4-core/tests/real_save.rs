use std::{env, fs};

use bl4_core::save::container::{ContainerContext, Platform, decode};

#[test]
#[ignore = "requires a private Borderlands 4 Steam save"]
fn decodes_private_steam_save() {
    let save_path = env::var("BL4_SAVE_PATH").expect("Set BL4_SAVE_PATH to a copied save file");

    let steam_id = env::var("BL4_STEAM_ID").expect("Set BL4_STEAM_ID to your Steam ID");

    let ciphertext = fs::read(save_path).expect("Read copied save file");

    let context = ContainerContext {
        account_id: steam_id,
        platform: Platform::Steam,
    };

    let decoded = decode(&ciphertext, &context).expect("Decode Steam save");

    assert!(!decoded.yaml_bytes.is_empty());
    assert_eq!(decoded.platform, Platform::Steam);

    if let Ok(output_path) = env::var("BL4_DECODED_YAML_PATH") {
        fs::write(&output_path, &decoded.yaml_bytes).expect("Write decoded YAML file");
        println!("Wrote decoded YAML to {output_path}");
    }

    println!(
        "Decoded {} YAML bytes successfully.",
        decoded.yaml_bytes.len()
    );
}
