use keyring::Entry;

const SERVICE: &str = "Stash";
const ACCOUNT: &str = "github_token";

/// GitHub tokens are stored in the OS credential store (Windows Credential
/// Manager) rather than in our own JSON files on disk — it's a secret, not
/// app state, and this gets us encryption-at-rest and Windows account
/// isolation for free.
fn entry_for(account: &str) -> Result<Entry, String> {
    Entry::new(SERVICE, account).map_err(|e| e.to_string())
}

fn get_token_for(account: &str) -> Option<String> {
    entry_for(account).ok()?.get_password().ok()
}

fn set_token_for(account: &str, token: &str) -> Result<(), String> {
    entry_for(account)?.set_password(token).map_err(|e| e.to_string())
}

fn clear_token_for(account: &str) -> Result<(), String> {
    match entry_for(account)?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

pub fn get_token() -> Option<String> {
    get_token_for(ACCOUNT)
}

pub fn set_token(token: &str) -> Result<(), String> {
    set_token_for(ACCOUNT, token)
}

pub fn clear_token() -> Result<(), String> {
    clear_token_for(ACCOUNT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_a_token_through_the_os_credential_store() {
        // A distinct account name so this never touches whatever real
        // token a developer running the suite may have connected.
        let account = "github_token_test_roundtrip";
        clear_token_for(account).unwrap();

        assert_eq!(get_token_for(account), None);
        set_token_for(account, "test-token-value").unwrap();
        assert_eq!(get_token_for(account), Some("test-token-value".to_string()));

        clear_token_for(account).unwrap();
        assert_eq!(get_token_for(account), None);
    }
}
