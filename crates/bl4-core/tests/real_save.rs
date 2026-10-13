use std::{
    env,
    fmt::Debug,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use bl4_core::save::container::{ContainerContext, Platform, decode, encode};

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

// Every run gets its own ignored directory. Create artifacts exclusively so a
// test never overwrites an existing save, export or previous compatibility result.
struct Results {
    directory: PathBuf,
    report: File,
}

impl Results {
    fn new(test_name: &str) -> Self {
        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/private/test-results");
        fs::create_dir_all(&root).expect("Create private results root");
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Read clock")
            .as_nanos();
        let directory = root.join(format!("{test_name}-{timestamp}-{}", std::process::id()));
        fs::create_dir(&directory).expect("Create a fresh results directory");
        let report = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.join("results.txt"))
            .expect("Create results report");
        let mut results = Self { directory, report };
        println!("Private test results: {}", results.directory.display());
        results.record("status=started");
        results.record("platform=steam");
        results.record("editor_compatibility=not_tested");
        results.record("in_game_compatibility=not_tested");
        results
    }

    fn record(&mut self, message: &str) {
        writeln!(self.report, "{message}").expect("Write results report");
        self.report.flush().expect("Flush results report");
    }

    fn step<T, E: Debug>(&mut self, name: &str, result: Result<T, E>) -> T {
        match result {
            Ok(value) => {
                self.record(&format!("{name}=passed"));
                value
            }
            Err(error) => {
                self.record(&format!("{name}=failed: {error:?}"));
                self.record("status=failed");
                panic!("{name} failed; see {}", self.directory.display());
            }
        }
    }

    fn save(&mut self, name: &str, bytes: &[u8]) {
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(self.directory.join(name))?;
            file.write_all(bytes)
        })();
        self.step(&format!("write_{name}"), result);
        self.record(&format!("{name}_bytes={}", bytes.len()));
    }

    fn check(&mut self, name: &str, passed: bool) {
        self.record(&format!("{name}={passed}"));
        if !passed {
            self.record("status=failed");
        }
        // Avoid assert_eq! on private bytes: failures must not dump their contents.
        assert!(passed, "{name} failed; see {}", self.directory.display());
    }
}

fn private_steam_context() -> ContainerContext {
    ContainerContext {
        account_id: env::var("BL4_STEAM_ID").expect("Set BL4_STEAM_ID to your Steam ID"),
        platform: Platform::Steam,
    }
}

#[test]
#[ignore = "requires private exported YAML and Steam ID; writes ignored test artifacts"]
fn encodes_private_steam_yaml() {
    let yaml_path = env::var("BL4_YAML_PATH")
        .or_else(|_| env::var("BL4_DECODED_YAML_PATH"))
        .expect("Set BL4_YAML_PATH to your exported YAML file");
    let context = private_steam_context();
    let mut results = Results::new("encode-yaml");
    let yaml = results.step("read_source_yaml", fs::read(&yaml_path));
    results.save("source.yaml", &yaml);
    results.check("source_yaml_nonempty", !yaml.is_empty());

    let ciphertext = results.step("encode", encode(&yaml, &context));
    results.save("encoded.sav", &ciphertext);
    let decoded = results.step("decode_encoded_save", decode(&ciphertext, &context));
    results.save("decoded.yaml", &decoded.yaml_bytes);
    results.check("yaml_bytes_identical", decoded.yaml_bytes == yaml);
    results.check("platform_identical", decoded.platform == context.platform);

    let current_source = results.step("reread_source_yaml", fs::read(&yaml_path));
    results.check("source_file_unchanged", current_source == yaml);
    results.record("status=passed");
}

#[test]
#[ignore = "requires private Steam save and ID; writes ignored test artifacts"]
fn decodes_and_reencodes_private_steam_save() {
    let save_path = env::var("BL4_SAVE_PATH").expect("Set BL4_SAVE_PATH to a copied save file");
    let context = private_steam_context();
    let mut results = Results::new("decode-encode-save");
    let source = results.step("read_source_save", fs::read(&save_path));
    results.save("source.sav", &source);

    let original = results.step("decode_source_save", decode(&source, &context));
    results.save("source.yaml", &original.yaml_bytes);
    results.check("source_yaml_nonempty", !original.yaml_bytes.is_empty());

    // Force reconstruction from YAML rather than copying original ciphertext.
    let rebuilt = results.step("encode", encode(&original.yaml_bytes, &context));
    results.save("reencoded.sav", &rebuilt);
    results.record(&format!("ciphertext_bytes_identical={}", source == rebuilt));
    let decoded = results.step("decode_reencoded_save", decode(&rebuilt, &context));
    results.save("redecoded.yaml", &decoded.yaml_bytes);
    results.check(
        "yaml_bytes_identical",
        decoded.yaml_bytes == original.yaml_bytes,
    );
    results.check("platform_identical", decoded.platform == original.platform);

    // Ciphertext equality is informational: compression can legitimately differ.
    let current_source = results.step("reread_source_save", fs::read(&save_path));
    results.check("source_file_unchanged", current_source == source);
    results.record("status=passed");
}
