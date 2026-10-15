use std::path::{Path, PathBuf};

pub fn default_save_root() -> Option<PathBuf> {
    // Resolves the Windows Documents known folder, including OneDrive redirects.
    dirs::document_dir().map(|documents| {
        documents
            .join("My Games")
            .join("Borderlands 4")
            .join("Saved")
            .join("SaveGames")
    })
}

pub fn account_from_path(path: &Path) -> Option<String> {
    let parts: Vec<_> = path.components().collect();
    parts.windows(3).find_map(|parts| {
        let root = parts[0].as_os_str().to_str()?;
        let account = parts[1].as_os_str().to_str()?;
        let profiles = parts[2].as_os_str().to_str()?;
        (root.eq_ignore_ascii_case("SaveGames")
            && profiles.eq_ignore_ascii_case("Profiles")
            && safe_account_component(account))
        .then(|| account.to_owned())
    })
}

fn safe_account_component(account: &str) -> bool {
    !account.is_empty()
        && account
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

/// Prefer an explicitly entered account, then a sole discovered client folder.
/// With multiple accounts, show SaveGames so the user chooses the account.
pub fn initial_directory(root: &Path, account: &str) -> PathBuf {
    if safe_account_component(account) {
        let client = root.join(account).join("Profiles").join("client");
        if client.is_dir() {
            return client;
        }
    }
    let candidates: Vec<_> = std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(safe_account_component)
        })
        .map(|entry| entry.path().join("Profiles").join("client"))
        .filter(|path| path.is_dir())
        .take(2)
        .collect();
    match candidates.as_slice() {
        [client] if account.is_empty() => client.clone(),
        _ => root.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_the_account_before_profiles_for_both_account_shapes() {
        for id in ["72623859790382856", "0123456789abcdef0123456789abcdef"] {
            let path = Path::new("SaveGames")
                .join(id)
                .join("Profiles/client/1.sav");
            assert_eq!(account_from_path(&path).as_deref(), Some(id));
        }
        assert_eq!(account_from_path(Path::new("elsewhere/client/1.sav")), None);
        assert_eq!(
            account_from_path(Path::new("SaveGames/id/other/1.sav")),
            None
        );
        assert!(!safe_account_component("../other"));
    }

    #[test]
    fn folder_selection_handles_one_account_multiple_accounts_and_manual_override() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("bl4-location-{}-{unique}", std::process::id()));
        let first = root.join("synthetic-steam/Profiles/client");
        let second = root.join("synthetic-epic/Profiles/client");
        std::fs::create_dir_all(&first).unwrap();
        assert_eq!(initial_directory(&root, ""), first);
        assert_eq!(initial_directory(&root, "missing-account"), root);
        std::fs::create_dir_all(&second).unwrap();
        assert_eq!(initial_directory(&root, ""), root);
        assert_eq!(initial_directory(&root, "synthetic-epic"), second);
        assert_eq!(initial_directory(&root, "../outside"), root);
        // Remove only the empty directories created by this test.
        for client in [first, second] {
            std::fs::remove_dir(&client).unwrap();
            std::fs::remove_dir(client.parent().unwrap()).unwrap();
            std::fs::remove_dir(client.parent().unwrap().parent().unwrap()).unwrap();
        }
        std::fs::remove_dir(root).unwrap();
    }
}
