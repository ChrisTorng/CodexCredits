//! Credential Manager adapter for long Codex tokens.
//! Windows Credential Manager limits a single UTF-16 password attribute to
//! 2,560 bytes. Segments <= 800 UTF-16 code units avoid that platform limit.
use sha2::{Digest, Sha256};

const SERVICE: &str = "CodexCredits";
const MAIN: &str = "codex-access-token";
const MARKER: &str = "cc2";
const CHUNK_UTF16_LIMIT: usize = 800;
const MAX_CHUNKS: usize = 32;

trait SecretStore {
    fn get(&self, name: &str) -> Result<Option<String>, String>;
    fn put(&self, name: &str, secret: &str) -> Result<(), String>;
    fn remove(&self, name: &str) -> Result<(), String>;
}

#[cfg(not(target_os="android"))]
struct SystemStore;
#[cfg(not(target_os="android"))]
impl SystemStore {
    fn entry(name: &str) -> Result<keyring::Entry, String> {
        keyring::Entry::new(SERVICE, name).map_err(|e| format!("無法使用系統憑證庫: {e}"))
    }
}
#[cfg(not(target_os="android"))]
impl SecretStore for SystemStore {
    fn get(&self, name: &str) -> Result<Option<String>, String> {
        match Self::entry(name)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(format!("讀取系統憑證失敗: {e}")),
        }
    }
    fn put(&self, name: &str, secret: &str) -> Result<(), String> {
        Self::entry(name)?.set_password(secret)
            .map_err(|e| format!("寫入系統憑證失敗: {e}"))
    }
    fn remove(&self, name: &str) -> Result<(), String> {
        match Self::entry(name)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(format!("清除系統憑證失敗: {e}")),
        }
    }
}

#[cfg(target_os="android")]
struct SystemStore;
#[cfg(target_os="android")]
impl SecretStore for SystemStore {
    fn get(&self,name:&str)->Result<Option<String>,String>{crate::android_bridge::credential_get(name)}
    fn put(&self,name:&str,value:&str)->Result<(),String>{crate::android_bridge::credential_put(name,value)}
    fn remove(&self,name:&str)->Result<(),String>{crate::android_bridge::credential_remove(name)}
}

fn chunk_name(bank: char, index: usize) -> String {
    format!("{MAIN}-{bank}-{index:02}")
}
fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn split_utf16(value: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut part = String::new();
    let mut units = 0;
    for c in value.chars() {
        let required = c.len_utf16();
        if units + required > CHUNK_UTF16_LIMIT && !part.is_empty() {
            out.push(std::mem::take(&mut part));
            units = 0;
        }
        part.push(c);
        units += required;
    }
    if !part.is_empty() {
        out.push(part);
    }
    out
}
fn parse_manifest(value: &str) -> Result<Option<(char, usize, &str)>, String> {
    if !value.starts_with("cc2:") {
        return Ok(None); // Backward compatibility with old short tokens.
    }
    let parts: Vec<_> = value.split(':').collect();
    if parts.len() != 4 || parts[0] != MARKER {
        return Err("憑證索引格式不正確，請重新儲存 Token".into());
    }
    let bank = match parts[1] {
        "a" => 'a',
        "b" => 'b',
        _ => return Err("憑證索引無效，請重新儲存 Token".into()),
    };
    let count = parts[2].parse::<usize>().map_err(|_| "憑證片段數量無效")?;
    if !(1..=MAX_CHUNKS).contains(&count)
        || parts[3].len() != 64
        || !parts[3].bytes().all(|c| c.is_ascii_hexdigit())
    {
        return Err("憑證索引檢查失敗，請重新儲存 Token".into());
    }
    Ok(Some((bank, count, parts[3])))
}
fn read_segments(store: &impl SecretStore, bank: char, count: usize, expected: &str) -> Result<String, String> {
    let mut result = String::new();
    for index in 0..count {
        let segment = store.get(&chunk_name(bank, index))?
            .ok_or("憑證片段不完整，請重新儲存 Token")?;
        if segment.encode_utf16().count() > CHUNK_UTF16_LIMIT {
            return Err("憑證片段長度異常，請重新儲存 Token".into());
        }
        result.push_str(&segment);
    }
    if digest(&result) != expected {
        return Err("憑證完整性驗證失敗，請重新儲存 Token".into());
    }
    Ok(result)
}
fn load_from(store: &impl SecretStore) -> Result<String, String> {
    let header = store.get(MAIN)?.ok_or("請先登入")?;
    match parse_manifest(&header)? {
        Some((bank, count, checksum)) => read_segments(store, bank, count, checksum),
        None => Ok(header),
    }
}
fn save_to(store: &impl SecretStore, token: &str) -> Result<(), String> {
    let chunks = split_utf16(token);
    if chunks.is_empty() || chunks.len() > MAX_CHUNKS {
        return Err("Access Token 長度超出可儲存範圍".into());
    }
    let old_bank = store.get(MAIN)?.as_deref()
        .and_then(|v| parse_manifest(v).ok().flatten().map(|(bank, _, _)| bank));
    // Write into the inactive bank; commit by updating the short manifest last.
    // If any write fails, the old manifest and its segments remain intact.
    let new_bank = if old_bank == Some('a') { 'b' } else { 'a' };
    for (index, part) in chunks.iter().enumerate() {
        store.put(&chunk_name(new_bank, index), part)?;
    }
    let checksum = digest(token);
    read_segments(store, new_bank, chunks.len(), &checksum)?;
    let manifest = format!("{MARKER}:{new_bank}:{}:{checksum}", chunks.len());
    store.put(MAIN, &manifest)?;

    // Cleanup is best effort after the new credentials are committed.
    for index in chunks.len()..MAX_CHUNKS {
        let _ = store.remove(&chunk_name(new_bank, index));
    }
    if let Some(bank) = old_bank {
        for index in 0..MAX_CHUNKS {
            let _ = store.remove(&chunk_name(bank, index));
        }
    }
    Ok(())
}
fn sign_out_from(store: &impl SecretStore) -> Result<(), String> {
    // Delete manifest first so any failed cleanup cannot keep an active login.
    let mut first_error = store.remove(MAIN).err();
    for bank in ['a', 'b'] {
        for index in 0..MAX_CHUNKS {
            if let Err(e) = store.remove(&chunk_name(bank, index)) {
                if first_error.is_none() { first_error = Some(e); }
            }
        }
    }
    if let Some(e) = first_error { Err(e) } else { Ok(()) }
}
pub fn load() -> Result<String, String> {
    load_from(&SystemStore)
}
pub fn save(token: &str) -> Result<(), String> {
    save_to(&SystemStore, token)
}
pub fn sign_out() -> Result<(), String> {
    sign_out_from(&SystemStore)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, collections::BTreeMap};

    #[derive(Default)]
    struct MockStore {
        entries: RefCell<BTreeMap<String, String>>,
        fail_put: RefCell<Option<String>>,
    }
    impl SecretStore for MockStore {
        fn get(&self, name: &str) -> Result<Option<String>, String> {
            Ok(self.entries.borrow().get(name).cloned())
        }
        fn put(&self, name: &str, secret: &str) -> Result<(), String> {
            if self.fail_put.borrow().as_deref() == Some(name) {
                return Err("injected credential write failure".into());
            }
            // Emulates the Windows per-entry password-size limit.
            if secret.encode_utf16().count() * 2 > 2560 {
                return Err("Windows UTF-16 2560-byte platform limit".into());
            }
            self.entries.borrow_mut().insert(name.to_owned(), secret.to_owned());
            Ok(())
        }
        fn remove(&self, name: &str) -> Result<(), String> {
            self.entries.borrow_mut().remove(name);
            Ok(())
        }
    }

    #[test]
    fn long_1665_character_token_round_trips_under_windows_limit() {
        let store = MockStore::default();
        let token = "x".repeat(1665);
        save_to(&store, &token).unwrap();
        assert_eq!(load_from(&store).unwrap(), token);
        assert_eq!(store.entries.borrow().len(), 4); // manifest + 3 pieces
        for secret in store.entries.borrow().values() {
            assert!(secret.encode_utf16().count() * 2 <= 2560);
        }
    }
    #[test]
    fn multi_byte_unicode_tokens_split_at_character_boundaries() {
        let store = MockStore::default();
        let token = "🔑中文".repeat(500);
        save_to(&store, &token).unwrap();
        assert_eq!(load_from(&store).unwrap(), token);
    }
    #[test]
    fn replacing_token_preserves_previous_value_if_commit_fails() {
        let store = MockStore::default();
        let old = "old-token-".repeat(180);
        let new = "new-token-".repeat(200);
        save_to(&store, &old).unwrap();
        *store.fail_put.borrow_mut() = Some(MAIN.into());
        assert!(save_to(&store, &new).is_err());
        assert_eq!(load_from(&store).unwrap(), old);
        *store.fail_put.borrow_mut() = None;
        save_to(&store, &new).unwrap();
        assert_eq!(load_from(&store).unwrap(), new);
    }
    #[test]
    fn legacy_short_token_is_still_readable_and_migrates() {
        let store = MockStore::default();
        store.put(MAIN, "legacy-token-12345").unwrap();
        assert_eq!(load_from(&store).unwrap(), "legacy-token-12345");
        save_to(&store, "replacement-token-67890").unwrap();
        assert_eq!(load_from(&store).unwrap(), "replacement-token-67890");
    }
    #[test]
    fn signout_clears_both_banks() {
        let store = MockStore::default();
        save_to(&store, &"t".repeat(1665)).unwrap();
        save_to(&store, &"z".repeat(1720)).unwrap();
        sign_out_from(&store).unwrap();
        assert!(store.entries.borrow().is_empty());
        assert!(load_from(&store).is_err());
    }
    #[test]
    fn missing_or_incorrect_segment_is_rejected() {
        let store = MockStore::default();
        save_to(&store, &"q".repeat(1665)).unwrap();
        store.remove(&chunk_name('a', 1)).unwrap();
        assert!(load_from(&store).is_err());
    }
}
