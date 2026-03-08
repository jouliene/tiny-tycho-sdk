use anyhow::{Context, Result, anyhow};
use bip39::Mnemonic;
use ed25519_dalek::{SECRET_KEY_LENGTH, Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use tiny_hderive::bip32::ExtendedPrivKey;
use zeroize::Zeroizing;

pub struct Keys {
    secret: SigningKey,
    public: VerifyingKey,
}

impl Keys {
    fn from_signing_key(secret: SigningKey) -> Self {
        let public = secret.verifying_key();
        Self { secret, public }
    }

    pub fn generate_keys() -> Self {
        let mut rng = OsRng;
        let secret = SigningKey::generate(&mut rng);
        Self::from_signing_key(secret)
    }

    pub fn generate_seed_phrase() -> Result<String> {
        let mnemonic = Mnemonic::generate(12).context("failed to generate 12-word seed phrase")?;
        Ok(mnemonic.to_string())
    }

    pub fn from_secret_hex_str(secret_hex_str: &str) -> Result<Self> {
        let mut bytes = Zeroizing::new([0u8; SECRET_KEY_LENGTH]);
        hex::decode_to_slice(secret_hex_str, &mut *bytes)
            .context("secret key must be exactly 64 hex chars (32 bytes)")?;
        let secret = SigningKey::from_bytes(&bytes);
        Ok(Self::from_signing_key(secret))
    }

    pub fn from_seed_phrase(phrase: &str) -> Result<Self> {
        Self::from_seed_phrase_with_index(phrase, 0)
    }

    pub fn from_seed_phrase_with_index(phrase: &str, index: u32) -> Result<Self> {
        let path = format!("m/44'/396'/0'/0/{index}");
        Self::from_seed_phrase_path(phrase, &path)
    }

    fn from_seed_phrase_path(phrase: &str, path: &str) -> Result<Self> {
        let mnemonic: Mnemonic = phrase.parse().context("invalid seed phrase")?;
        let seed = Zeroizing::new(mnemonic.to_seed(""));
        let ext = ExtendedPrivKey::derive(seed.as_ref(), path)
            .map_err(|e| anyhow!("failed to derive path {path}: {e:?}"))?;
        let secret_bytes = Zeroizing::new(ext.secret());
        let secret = SigningKey::from_bytes(&secret_bytes);
        Ok(Self::from_signing_key(secret))
    }

    pub fn public_key(&self) -> &VerifyingKey {
        &self.public
    }

    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.public.to_bytes()
    }

    pub fn public_key_hex(&self) -> String {
        hex::encode(self.public.to_bytes())
    }

    pub fn secret_key_bytes(&self) -> [u8; 32] {
        self.secret.to_bytes()
    }

    pub fn secret_key_hex(&self) -> String {
        hex::encode(self.secret.to_bytes())
    }

    pub fn sign(&self, message: &[u8]) -> Signature {
        self.secret.sign(message)
    }

    pub fn verify(&self, message: &[u8], signature: &Signature) -> bool {
        self.public.verify(message, signature).is_ok()
    }
}
