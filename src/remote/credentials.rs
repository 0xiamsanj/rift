use keyring::Entry;
use tracing::{debug, warn};

const SERVICE_NAME: &str = "rift-sftp-client";

pub struct CredentialStore;

impl CredentialStore {
    /// Saves a secret (password or passphrase) for a given profile ID into OS secure store.
    pub fn save_password(profile_id: &str, secret: &str) -> Result<(), String> {
        let entry = Entry::new(SERVICE_NAME, profile_id)
            .map_err(|e| format!("Failed to create keyring entry: {e}"))?;
        entry
            .set_password(secret)
            .map_err(|e| format!("Failed to store credential securely: {e}"))?;
        debug!(profile_id = %profile_id, "Credential saved to secure keyring");
        Ok(())
    }

    /// Retrieves a secret from OS secure store.
    pub fn get_password(profile_id: &str) -> Option<String> {
        let entry = match Entry::new(SERVICE_NAME, profile_id) {
            Ok(e) => e,
            Err(e) => {
                warn!(profile_id = %profile_id, "Could not open keyring: {e}");
                return None;
            }
        };
        match entry.get_password() {
            Ok(p) => Some(p),
            Err(e) => {
                debug!(profile_id = %profile_id, "No stored password found in keyring ({e})");
                None
            }
        }
    }

    /// Deletes a secret from OS secure store.
    pub fn delete_password(profile_id: &str) -> Result<(), String> {
        let entry = Entry::new(SERVICE_NAME, profile_id)
            .map_err(|e| format!("Failed to create keyring entry: {e}"))?;
        match entry.delete_credential() {
            Ok(_) => {
                debug!(profile_id = %profile_id, "Credential deleted from keyring");
                Ok(())
            }
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(format!("Failed to delete credential: {e}")),
        }
    }
}
