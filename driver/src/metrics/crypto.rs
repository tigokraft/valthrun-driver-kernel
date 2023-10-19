use alloc::{
    string::String,
    vec::Vec,
};

use aes_gcm::{
    AeadCore,
    AeadInPlace,
    Aes256Gcm,
    Key,
    KeyInit,
};
use anyhow::anyhow;
use base64::prelude::*;
use obfstr::obfstr;
use rsa::{
    pkcs8::DecodePublicKey,
    Pkcs1v15Encrypt,
    RsaPublicKey,
};
use sha1::{
    Digest,
    Sha1,
};

use crate::util::Win32Rng;

pub struct MetricsCrypto {
    key_id: String,
    public_key: RsaPublicKey,

    rng: Win32Rng,
    aes_key: Key<Aes256Gcm>,
}

impl MetricsCrypto {
    pub fn new() -> anyhow::Result<Self> {
        let public_key = include_bytes!("pub_key.pem");
        let key_id = {
            let mut hasher = Sha1::new();
            hasher.update(public_key);

            let hash = hasher.finalize();
            BASE64_STANDARD.encode(&hash[..])
        };

        let public_key = RsaPublicKey::from_public_key_der(public_key)
            .map_err(|err| anyhow!("{}: {:#}", obfstr!("pub key"), err))?;

        let mut rng = Win32Rng::new();
        let aes_key = Aes256Gcm::generate_key(&mut rng);

        Ok(Self {
            key_id,
            public_key,

            rng,
            aes_key,
        })
    }

    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    pub fn encrypt(&mut self, mut payload: &mut [u8]) -> anyhow::Result<Vec<u8>> {
        let nonce = Aes256Gcm::generate_nonce(&mut self.rng);

        let cipher = Aes256Gcm::new(&self.aes_key);
        let tag = cipher
            .encrypt_in_place_detached(&nonce, b"", &mut payload)
            .map_err(|err| anyhow!("{}: {:#}", obfstr!("payload encrypt"), err))?;

        let mut crypto_header = [0u8; 0x20 + 0x0C + 0x10];
        crypto_header[0x00..0x20].copy_from_slice(self.aes_key.as_slice());
        crypto_header[0x20..0x2C].copy_from_slice(nonce.as_slice());
        crypto_header[0x2C..0x3C].copy_from_slice(tag.as_slice());

        let mut crypto_header = self
            .public_key
            .encrypt(&mut self.rng, Pkcs1v15Encrypt, &crypto_header)
            .map_err(|err| anyhow!("{}: {:#}", obfstr!("header encrypt"), err))?;

        crypto_header.extend_from_slice(&payload);
        Ok(crypto_header)
    }
}
