//! Implementations of the [`SecureStorage`] service for the macOS platform.

use std::{cell::OnceCell, path::PathBuf};

use anyhow::{anyhow, Context};
use rand::RngCore;
use ring::aead;
use security_framework::os::macos::{
    keychain::SecKeychain, keychain_item::SecKeychainItem, passwords::SecKeychainItemPassword,
};

use super::Error;

/// Implementation of the SecureStorage service using macOS Security
/// framework keychains.
pub struct SecureStorage {
    /// The name of the service under which to store the values.
    service_name: String,
}

impl SecureStorage {
    pub fn new(service_name: &str) -> Self {
        Self {
            service_name: service_name.to_owned(),
        }
    }
}

impl super::SecureStorage for SecureStorage {
    fn write_value(&self, key: &str, value: &str) -> Result<(), Error> {
        let keychain = SecKeychain::default()?;
        keychain
            .set_generic_password(self.service_name.as_str(), key, value.as_bytes())
            .map_err(Into::into)
    }

    fn read_value(&self, key: &str) -> Result<String, Error> {
        let (password, _) = self.get_password_item(key)?;
        String::from_utf8(password.as_ref().to_vec())
            .map_err(|err| Error::DecodeError(err.utf8_error()))
    }

    fn remove_value(&self, key: &str) -> Result<(), Error> {
        let (_, item) = self.get_password_item(key)?;
        item.delete();
        Ok(())
    }
}

impl SecureStorage {
    fn get_password_item(
        &self,
        key: &str,
    ) -> Result<(SecKeychainItemPassword, SecKeychainItem), Error> {
        let keychain = SecKeychain::default()?;
        keychain
            .find_generic_password(&self.service_name, key)
            .map_err(|_| Error::NotFound)
    }
}

impl From<security_framework::base::Error> for Error {
    fn from(value: security_framework::base::Error) -> Self {
        Error::Unknown(anyhow!(value))
    }
}

// TODO(dedupe): the encrypt/decrypt + per-key file logic below is a near-verbatim
// copy of the fallback path in linux.rs. Lift into a shared module when a
// second platform needs it.
//
// File-backed AES-256-GCM secret storage. Selected at runtime in
// `app/src/lib.rs` when `WARP_SECURE_STORAGE_FILE` is set, to avoid the
// macOS Keychain prompt on dev workstations. Default macOS path is still the
// Keychain via [`SecureStorage`] above.
pub struct FileSecureStorage {
    service_name: String,
    storage_dir: PathBuf,
    encryption_key: OnceCell<Option<aead::LessSafeKey>>,
}

impl FileSecureStorage {
    pub fn new(service_name: &str, storage_dir: PathBuf) -> Self {
        Self {
            service_name: service_name.to_owned(),
            storage_dir,
            encryption_key: OnceCell::new(),
        }
    }

    fn encryption_key(&self) -> Result<&aead::LessSafeKey, Error> {
        self.encryption_key
            .get_or_init(|| {
                let mut key_bytes = Vec::from("https://releases.warp.dev/channel_versions.json");
                key_bytes.resize(aead::AES_256_GCM.key_len(), 0);
                match aead::UnboundKey::new(&aead::AES_256_GCM, key_bytes.as_slice()) {
                    Ok(key) => Some(aead::LessSafeKey::new(key)),
                    Err(_) => {
                        log::error!("Failed to initialize file secure storage encryption key");
                        None
                    }
                }
            })
            .as_ref()
            .ok_or_else(|| Error::Unknown(anyhow!("Invalid encryption key")))
    }

    fn storage_file(&self, key: &str) -> PathBuf {
        let filename = format!("{}-{key}", self.service_name);
        self.storage_dir.join(filename)
    }

    fn encrypt(&self, value: &str) -> Result<Vec<u8>, Error> {
        let encryption_key = self.encryption_key()?;

        let mut rng = rand::thread_rng();
        let mut nonce_bytes = [0u8; aead::NONCE_LEN];
        rng.fill_bytes(&mut nonce_bytes);
        let nonce = aead::Nonce::assume_unique_for_key(nonce_bytes);

        let mut data = value.as_bytes().to_vec();
        encryption_key
            .seal_in_place_append_tag(nonce, aead::Aad::empty(), &mut data)
            .map_err(Into::<Error>::into)
            .context("File secure storage encryption failed")?;

        let mut output = Vec::<u8>::with_capacity(aead::NONCE_LEN + data.len());
        output.extend_from_slice(&nonce_bytes);
        output.append(&mut data);

        Ok(output)
    }

    fn decrypt(&self, value: &[u8]) -> Result<String, Error> {
        if value.len() < aead::NONCE_LEN + 1 {
            return Err(Error::Unknown(anyhow!(
                "File secure storage value too small to decrypt"
            )));
        }

        let encryption_key = self.encryption_key()?;

        let nonce_bytes = &value[0..aead::NONCE_LEN];
        let nonce = aead::Nonce::try_assume_unique_for_key(nonce_bytes)
            .map_err(Into::<Error>::into)
            .context("Failed to parse nonce for file secure storage decryption")?;

        let mut data_bytes = value[aead::NONCE_LEN..].to_owned();
        let decrypted_length = encryption_key
            .open_in_place(nonce, aead::Aad::empty(), &mut data_bytes)
            .map_err(Into::<Error>::into)
            .context("File secure storage decryption failed")?
            .len();
        data_bytes.resize(decrypted_length, 0);

        String::from_utf8(data_bytes).map_err(|err| Error::DecodeError(err.utf8_error()))
    }
}

impl super::SecureStorage for FileSecureStorage {
    fn write_value(&self, key: &str, value: &str) -> Result<(), Error> {
        let storage_file = self.storage_file(key);
        // Ensure the parent directory exists; state_dir() is shared with sqlite
        // and other writers, but be defensive in case we're the first to land.
        if let Some(parent) = storage_file.parent() {
            std::fs::create_dir_all(parent).map_err(|err| Error::Unknown(err.into()))?;
        }
        let encrypted = self.encrypt(value)?;
        std::fs::write(storage_file, encrypted).map_err(|err| Error::Unknown(err.into()))
    }

    fn read_value(&self, key: &str) -> Result<String, Error> {
        let storage_file = self.storage_file(key);
        let data = std::fs::read(storage_file).map_err(|_| Error::NotFound)?;
        self.decrypt(&data)
    }

    fn remove_value(&self, key: &str) -> Result<(), Error> {
        let storage_file = self.storage_file(key);
        std::fs::remove_file(storage_file).map_err(|err| match err {
            ref io_error if io_error.kind() == std::io::ErrorKind::NotFound => Error::NotFound,
            io_error => Error::Unknown(io_error.into()),
        })
    }
}

impl From<ring::error::Unspecified> for Error {
    fn from(value: ring::error::Unspecified) -> Self {
        Error::Unknown(anyhow!(value))
    }
}

#[cfg(test)]
mod file_secure_storage_tests {
    use super::*;
    use crate::secure_storage::SecureStorage as _;

    #[test]
    fn round_trip_value() {
        let dir = tempfile::tempdir().unwrap();
        let storage = FileSecureStorage::new("dev.test.svc", dir.path().to_owned());

        assert!(matches!(storage.read_value("k"), Err(Error::NotFound)));

        storage.write_value("k", "secret").unwrap();
        assert_eq!(storage.read_value("k").unwrap(), "secret");

        storage.write_value("k", "rotated").unwrap();
        assert_eq!(storage.read_value("k").unwrap(), "rotated");

        storage.remove_value("k").unwrap();
        assert!(matches!(storage.read_value("k"), Err(Error::NotFound)));
    }
}
