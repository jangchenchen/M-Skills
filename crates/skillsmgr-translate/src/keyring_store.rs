use keyring::Entry;
use skillsmgr_core::{Result, SkillsMgrError};

const SERVICE: &str = "m-skills.translate";

fn entry(provider_id: &str) -> Result<Entry> {
    Entry::new(SERVICE, provider_id).map_err(|e| SkillsMgrError::Keyring {
        message: format!("open keyring entry: {e}"),
    })
}

pub fn get_api_key(provider_id: &str) -> Result<Option<String>> {
    let entry = entry(provider_id)?;
    match entry.get_password() {
        Ok(secret) => Ok(Some(secret)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(SkillsMgrError::Keyring {
            message: format!("read keyring entry: {e}"),
        }),
    }
}

pub fn set_api_key(provider_id: &str, secret: &str) -> Result<()> {
    let entry = entry(provider_id)?;
    entry
        .set_password(secret)
        .map_err(|e| SkillsMgrError::Keyring {
            message: format!("write keyring entry: {e}"),
        })
}

pub fn clear_api_key(provider_id: &str) -> Result<()> {
    let entry = entry(provider_id)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(SkillsMgrError::Keyring {
            message: format!("delete keyring entry: {e}"),
        }),
    }
}

pub fn has_api_key(provider_id: &str) -> Result<bool> {
    Ok(get_api_key(provider_id)?.is_some())
}
