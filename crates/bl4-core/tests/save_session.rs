use bl4_core::save::{
    container::{ContainerContext, Platform, encode},
    document::DocumentError,
    session::{SaveSession, SessionError},
};

fn context() -> ContainerContext {
    ContainerContext {
        account_id: "72623859790382856".into(),
        platform: Platform::Steam,
    }
}

#[test]
fn malformed_container_returns_a_container_error() {
    let result = SaveSession::open(vec![0; 15], context());

    assert!(matches!(result, Err(SessionError::Container(_))));
}

#[test]
fn valid_container_with_invalid_yaml_returns_a_document_error() {
    let context = context();
    let ciphertext =
        encode(b"name: [unfinished", &context).expect("encode malformed YAML as a valid container");

    let result = SaveSession::open(ciphertext, context);

    assert!(matches!(
        result,
        Err(SessionError::Document(DocumentError::InvalidYaml))
    ));
}

#[test]
fn snapshot_reconstruction_preserves_the_complete_document() {
    let context = context();
    let yaml = concat!(
        "name: hero\n",
        "level: 42\n",
        "future: !Unknown\n",
        "  nested: [true, null, '42']\n",
    );

    let original_ciphertext = encode(yaml.as_bytes(), &context).expect("encode original container");
    let session =
        SaveSession::open(original_ciphertext, context.clone()).expect("open original session");

    let rebuilt_ciphertext = session
        .snapshot()
        .reconstruct()
        .expect("reconstruct snapshot");
    let reopened =
        SaveSession::open(rebuilt_ciphertext, context).expect("open reconstructed session");

    assert_eq!(reopened.working_document(), session.working_document());
}
