use sha2::{Digest, Sha256};

use super::{
    container::{ContainerContext, ContainerError, decode, encode},
    document::{DocumentError, SaveDocument},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionError {
    Container(ContainerError),
    Document(DocumentError),
    RevisionOverflow,
}

#[derive(Clone, Debug)]
pub struct SessionSnapshot {
    document: SaveDocument,
    context: ContainerContext,
    source_fingerprint: [u8; 32],
    revision: u64,
}

impl SessionSnapshot {
    pub fn document(&self) -> &SaveDocument {
        &self.document
    }

    pub fn context(&self) -> &ContainerContext {
        &self.context
    }

    pub fn source_fingerprint(&self) -> &[u8; 32] {
        &self.source_fingerprint
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn reconstruct(&self) -> Result<Vec<u8>, SessionError> {
        let yaml = self
            .document
            .to_yaml_bytes()
            .map_err(SessionError::Document)?;

        encode(&yaml, &self.context).map_err(SessionError::Container)
    }
}

#[derive(Clone, Debug)]
pub struct SaveSession {
    original_ciphertext: Vec<u8>,
    original_document: SaveDocument,
    working_document: SaveDocument,
    context: ContainerContext,
    revision: u64,
    source_fingerprint: [u8; 32],
}

impl SaveSession {
    pub fn new(
        original_ciphertext: Vec<u8>,
        document: SaveDocument,
        context: ContainerContext,
    ) -> Self {
        let source_fingerprint: [u8; 32] = Sha256::digest(&original_ciphertext).into();
        Self {
            original_ciphertext,
            original_document: document.clone(),
            working_document: document,
            context,
            revision: 0,
            source_fingerprint,
        }
    }

    pub fn original_ciphertext(&self) -> &[u8] {
        &self.original_ciphertext
    }

    pub fn original_document(&self) -> &SaveDocument {
        &self.original_document
    }

    pub fn working_document(&self) -> &SaveDocument {
        &self.working_document
    }

    pub fn context(&self) -> &ContainerContext {
        &self.context
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn is_dirty(&self) -> bool {
        self.working_document != self.original_document
    }

    pub fn source_fingerprint(&self) -> &[u8; 32] {
        &self.source_fingerprint
    }

    pub fn source_matches(&self, ciphertext: &[u8]) -> bool {
        let fingerprint: [u8; 32] = Sha256::digest(ciphertext).into();

        fingerprint == self.source_fingerprint
    }

    pub fn snapshot(&self) -> SessionSnapshot {
        SessionSnapshot {
            document: self.working_document.clone(),
            context: self.context.clone(),
            source_fingerprint: self.source_fingerprint,
            revision: self.revision,
        }
    }

    pub fn snapshot_is_current(&self, snapshot: &SessionSnapshot) -> bool {
        snapshot.source_fingerprint == self.source_fingerprint && snapshot.revision == self.revision
    }

    pub fn replace_working_document(
        &mut self,
        document: SaveDocument,
    ) -> Result<bool, SessionError> {
        if document == self.working_document {
            return Ok(false);
        }

        let next_revision = self
            .revision
            .checked_add(1)
            .ok_or(SessionError::RevisionOverflow)?;

        self.working_document = document;
        self.revision = next_revision;

        Ok(true)
    }

    pub fn open(
        original_ciphertext: Vec<u8>,
        context: ContainerContext,
    ) -> Result<Self, SessionError> {
        let decoded = decode(&original_ciphertext, &context).map_err(SessionError::Container)?;

        let document = SaveDocument::parse(&decoded.yaml_bytes).map_err(SessionError::Document)?;
        Ok(Self::new(original_ciphertext, document, context))
    }

    pub fn unchanged_ciphertext(&self) -> Option<&[u8]> {
        if self.is_dirty() {
            None
        } else {
            Some(self.original_ciphertext.as_slice())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::container::{Platform, encode};

    #[test]
    fn new_session_retains_source_and_starts_clean() {
        let ciphertext = vec![1, 2, 3, 4];
        let document = SaveDocument::parse(b"name: hero\n").expect("valid document");
        let context = ContainerContext {
            account_id: "synthetic-account".into(),
            platform: Platform::Epic,
        };

        let session = SaveSession::new(ciphertext.clone(), document.clone(), context.clone());

        assert_eq!(session.original_ciphertext(), ciphertext);
        assert_eq!(session.original_document(), &document);
        assert_eq!(session.working_document(), &document);
        assert_eq!(session.context(), &context);
        assert_eq!(session.revision(), 0);
        assert_eq!(
            session.source_fingerprint(),
            &[
                0x9f, 0x64, 0xa7, 0x47, 0xe1, 0xb9, 0x7f, 0x13, 0x1f, 0xab, 0xb6, 0xb4, 0x47, 0x29,
                0x6c, 0x9b, 0x6f, 0x02, 0x01, 0xe7, 0x9f, 0xb3, 0xc5, 0x35, 0x6e, 0x6c, 0x77, 0xe8,
                0x9b, 0x6a, 0x80, 0x6a,
            ]
        );
        assert!(session.source_matches(&ciphertext));
        assert!(!session.source_matches(&[1, 2, 3, 5]));
        assert!(!session.is_dirty());
    }

    #[test]
    fn replacing_working_document_updates_revision_and_dirty_state() {
        let original = SaveDocument::parse(b"name: hero\n").expect("valid original");
        let updated = SaveDocument::parse(b"name: vault hunter\n").expect("valid update");

        let context = ContainerContext {
            account_id: "synthetic-account".into(),
            platform: Platform::Epic,
        };

        let mut session = SaveSession::new(vec![1, 2, 3, 4], original.clone(), context);

        assert_eq!(session.replace_working_document(updated.clone()), Ok(true));
        assert_eq!(session.revision(), 1);
        assert!(session.is_dirty());
        assert_eq!(session.original_document(), &original);
        assert_eq!(session.working_document(), &updated);

        assert_eq!(session.replace_working_document(updated), Ok(false));
        assert_eq!(session.revision(), 1);

        assert_eq!(session.replace_working_document(original), Ok(true));
        assert_eq!(session.revision(), 2);
        assert!(!session.is_dirty());
    }

    #[test]
    fn revision_overflow_does_not_change_the_working_document() {
        let original = SaveDocument::parse(b"name: hero\n").expect("valid original");
        let updated = SaveDocument::parse(b"name: vault hunter\n").expect("valid update");

        let context = ContainerContext {
            account_id: "synthetic-account".into(),
            platform: Platform::Epic,
        };

        let mut session = SaveSession::new(vec![1, 2, 3, 4], original.clone(), context);

        session.revision = u64::MAX;

        let result = session.replace_working_document(updated);

        assert_eq!(result, Err(SessionError::RevisionOverflow));
        assert_eq!(session.revision(), u64::MAX);
        assert_eq!(session.working_document(), &original);
        assert!(!session.is_dirty());
    }

    #[test]
    fn snapshot_retains_the_state_and_revision_that_created_it() {
        let original = SaveDocument::parse(b"name: hero\n").expect("valid original");
        let updated = SaveDocument::parse(b"name: vault hunter\n").expect("valid update");

        let context = ContainerContext {
            account_id: "synthetic-account".into(),
            platform: Platform::Epic,
        };

        let mut session = SaveSession::new(vec![1, 2, 3, 4], original.clone(), context.clone());

        let snapshot = session.snapshot();

        assert_eq!(snapshot.document(), &original);
        assert_eq!(snapshot.context(), &context);
        assert_eq!(snapshot.revision(), 0);
        assert!(session.snapshot_is_current(&snapshot));

        session
            .replace_working_document(updated)
            .expect("replace working document");

        assert_eq!(snapshot.document(), &original);
        assert_eq!(snapshot.revision(), 0);
        assert!(!session.snapshot_is_current(&snapshot));

        let current_snapshot = session.snapshot();

        assert_eq!(current_snapshot.revision(), 1);
        assert!(session.snapshot_is_current(&current_snapshot));
    }

    #[test]
    fn opens_encoded_container_as_a_clean_session() {
        let context = ContainerContext {
            account_id: "72623859790382856".into(),
            platform: Platform::Steam,
        };

        let yaml = b"name: hero\nunknown: [true, 42]\n";
        let ciphertext = encode(yaml, &context).expect("encode synthetic save");

        let session =
            SaveSession::open(ciphertext.clone(), context.clone()).expect("open synthetic save");

        assert_eq!(session.original_ciphertext(), ciphertext);
        assert_eq!(session.context(), &context);
        assert_eq!(
            session
                .working_document()
                .root()
                .get("name")
                .and_then(serde_yaml_ng::Value::as_str),
            Some("hero")
        );
        assert!(session.source_matches(&ciphertext));
        assert_eq!(session.revision(), 0);
        assert!(!session.is_dirty());
    }
    #[test]
    fn unchanged_ciphertext_is_available_only_while_clean() {
        let original = SaveDocument::parse(b"name: hero\n").expect("valid original");
        let updated = SaveDocument::parse(b"name: vault hunter\n").expect("valid update");

        let ciphertext = vec![1, 2, 3, 4];
        let context = ContainerContext {
            account_id: "synthetic-account".into(),
            platform: Platform::Epic,
        };

        let mut session = SaveSession::new(ciphertext.clone(), original.clone(), context);

        assert_eq!(session.unchanged_ciphertext(), Some(ciphertext.as_slice()));

        session
            .replace_working_document(updated)
            .expect("apply update");

        assert_eq!(session.unchanged_ciphertext(), None);

        session
            .replace_working_document(original)
            .expect("restore original");

        assert_eq!(session.unchanged_ciphertext(), Some(ciphertext.as_slice()));
    }
}
