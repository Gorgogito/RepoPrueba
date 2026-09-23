use super::Provider;
use keyring::Entry;

const SERVICE: &str = "Stash";

/// Each provider's credential is stored as an opaque string under its own
/// account name in the OS credential store (Windows Credential Manager).
/// GitHub's is just the PAT; Bitbucket's is `username\napp_password`
/// (see [`super::bitbucket::encode_credential`]) — callers that need the
/// parts split back out do that themselves.
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

pub fn get_token(provider: Provider) -> Option<String> {
    get_token_for(provider.token_account())
}

pub fn set_token(provider: Provider, token: &str) -> Result<(), String> {
    set_token_for(provider.token_account(), token)
}

pub fn clear_token(provider: Provider) -> Result<(), String> {
    clear_token_for(provider.token_account())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_a_token_through_the_os_credential_store_per_provider() {
        for (account, value) in [
            ("stash_test_provider_token_a", "value-a"),
            ("stash_test_provider_token_b", "value-b"),
        ] {
            clear_token_for(account).unwrap();
            assert_eq!(get_token_for(account), None);
            set_token_for(account, value).unwrap();
            assert_eq!(get_token_for(account), Some(value.to_string()));
            clear_token_for(account).unwrap();
            assert_eq!(get_token_for(account), None);
        }
    }
}
