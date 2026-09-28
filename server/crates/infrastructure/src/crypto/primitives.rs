use domain::{
    passkey::PublicKeyAlgorithm,
    secret::Secret,
    security::{Crypto, CryptoError, TokenError},
};
use ring::{
    aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey},
    digest, hkdf, hmac,
    rand::{SecureRandom, SystemRandom},
    signature::{self, UnparsedPublicKey},
};
use thiserror::Error;
use zeroize::Zeroizing;

pub const SECRET_KEY_BYTES: usize = 32;

#[derive(Debug, Error)]
#[error("{0}")]
struct RingError(&'static str);

/// The format byte of sealed values: `[version][key fingerprint: 4][nonce: 12][ciphertext+tag]`.
/// Version 2 encrypts with an AES key derived from `SECRET_KEY` (HKDF, so no two primitives share
/// key material); version 1 used `SECRET_KEY` itself and still opens. Change the layout only by
/// adding a new version byte that `open` learns, never by reinterpreting an existing one: sealed
/// values live in the database indefinitely.
const SEAL_V1: u8 = 1;
const SEAL_V2: u8 = 2;
const FINGERPRINT_LEN: usize = 4;

/// One `SECRET_KEY`: the AEAD key, a fingerprint that tells sealed values which key sealed them,
/// and the pepper for keyed digests, each derived for its own purpose.
struct KeySet {
    fingerprint: [u8; FINGERPRINT_LEN],
    aead: LessSafeKey,
    aead_v1: LessSafeKey,
    pepper: hmac::Key,
}

impl KeySet {
    fn new(secret_key: &[u8; SECRET_KEY_BYTES]) -> Self {
        #[expect(
            clippy::expect_used,
            reason = "cannot fail: the key has exactly the length AES-256 needs"
        )]
        let aead_v1 = UnboundKey::new(&AES_256_GCM, secret_key).expect("AES-256 keys are 32 bytes");
        #[expect(
            clippy::expect_used,
            reason = "cannot fail: HKDF-SHA256 expands to up to 8160 bytes, AES-256 needs 32"
        )]
        let aead: UnboundKey = hkdf::Salt::new(hkdf::HKDF_SHA256, &[])
            .extract(secret_key)
            .expand(&[b"aead v2"], &AES_256_GCM)
            .expect("HKDF-SHA256 expands to 32 bytes")
            .into();
        let derive =
            |label: &[u8]| hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, secret_key), label);
        let mut fingerprint = [0u8; FINGERPRINT_LEN];
        fingerprint.copy_from_slice(&derive(b"key fingerprint").as_ref()[..FINGERPRINT_LEN]);
        Self {
            fingerprint,
            aead: LessSafeKey::new(aead),
            aead_v1: LessSafeKey::new(aead_v1),
            pepper: hmac::Key::new(hmac::HMAC_SHA256, derive(b"keyed digest pepper").as_ref()),
        }
    }

    fn digest(&self, purpose: &str, data: &[u8]) -> [u8; 32] {
        let mut context = hmac::Context::with_key(&self.pepper);
        context.update(purpose.as_bytes());
        context.update(&[0]);
        context.update(data);
        let mut out = [0u8; 32];
        out.copy_from_slice(context.sign().as_ref());
        out
    }

    fn open(
        &self,
        version: u8,
        nonce: &[u8],
        ciphertext: &[u8],
        aad: &[u8],
    ) -> Option<Zeroizing<Vec<u8>>> {
        let key = match version {
            SEAL_V2 => &self.aead,
            SEAL_V1 => &self.aead_v1,
            _ => return None,
        };
        let nonce = Nonce::try_assume_unique_for_key(nonce).ok()?;
        let mut buffer = Zeroizing::new(ciphertext.to_vec());
        let plaintext = key.open_in_place(nonce, Aad::from(aad), &mut buffer).ok()?;
        Some(Zeroizing::new(plaintext.to_vec()))
    }
}

/// Cryptography on ring, with the server's key for sealing secrets at rest and keying digests.
/// During a key rotation the previous key still opens and matches what it made.
///
/// From the one `SECRET_KEY` it derives separate keys for AES-256-GCM sealing, a pepper for keyed
/// digests, and a 4-byte fingerprint stored in each sealed value. `open` picks the current or
/// previous key by that fingerprint, so a value sealed by an unknown key fails to open instead of
/// being tried against the wrong one. `seal` always uses the current key with a fresh random
/// nonce, and binds the value to its `context`.
///
/// Key rotation: give the new key to [`RingCrypto::new`] and the old one to
/// [`RingCrypto::with_previous_key`]; values sealed under the old key keep opening while it is
/// configured, and `previous_keyed_digest` lets long-lived digests still match.
pub struct RingCrypto {
    current: KeySet,
    previous: Option<KeySet>,
    random: SystemRandom,
}

impl RingCrypto {
    pub fn new(secret_key: &[u8; SECRET_KEY_BYTES]) -> Self {
        Self {
            current: KeySet::new(secret_key),
            previous: None,
            random: SystemRandom::new(),
        }
    }

    /// Also opens values sealed with `previous_key`, the `SECRET_KEY` before a rotation.
    #[must_use]
    pub fn with_previous_key(mut self, previous_key: &[u8; SECRET_KEY_BYTES]) -> Self {
        self.previous = Some(KeySet::new(previous_key));
        self
    }

    fn keys(&self) -> impl Iterator<Item = &KeySet> {
        std::iter::once(&self.current).chain(&self.previous)
    }

    fn fill(&self, bytes: &mut [u8]) -> Result<(), TokenError> {
        self.random
            .fill(bytes)
            .map_err(|_| TokenError::new(RingError("the random number generator failed")))
    }
}

impl std::fmt::Debug for RingCrypto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RingCrypto(<key redacted>)")
    }
}

impl Crypto for RingCrypto {
    fn random_bytes(&self, len: usize) -> Result<Vec<u8>, TokenError> {
        let mut bytes = vec![0u8; len];
        self.fill(&mut bytes)?;
        Ok(bytes)
    }

    fn random_digits(&self, digits: u32) -> Result<Secret, TokenError> {
        let modulus = 10u64.pow(digits.min(18));
        // Rejection sampling: values in the incomplete last block would make low codes slightly
        // more likely.
        let limit = u64::MAX - u64::MAX % modulus;
        loop {
            let mut bytes = [0u8; 8];
            self.fill(&mut bytes)?;
            let value = u64::from_le_bytes(bytes);
            if value < limit {
                let width = digits as usize;
                return Ok(Secret::new(format!("{:0width$}", value % modulus)));
            }
        }
    }

    fn sha256(&self, data: &[u8]) -> [u8; 32] {
        let mut out = [0u8; 32];
        out.copy_from_slice(digest::digest(&digest::SHA256, data).as_ref());
        out
    }

    fn hmac_sha1(&self, key: &[u8], message: &[u8]) -> [u8; 20] {
        let key = hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, key);
        let mut out = [0u8; 20];
        out.copy_from_slice(hmac::sign(&key, message).as_ref());
        out
    }

    fn keyed_digest(&self, purpose: &str, data: &[u8]) -> [u8; 32] {
        self.current.digest(purpose, data)
    }

    fn previous_keyed_digest(&self, purpose: &str, data: &[u8]) -> Option<[u8; 32]> {
        self.previous.as_ref().map(|key| key.digest(purpose, data))
    }

    fn seal(&self, plaintext: &[u8], context: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let mut nonce = [0u8; NONCE_LEN];
        self.fill(&mut nonce).map_err(CryptoError::new)?;
        let mut sealed = plaintext.to_vec();
        self.current
            .aead
            .seal_in_place_append_tag(
                Nonce::assume_unique_for_key(nonce),
                Aad::from(context),
                &mut sealed,
            )
            .map_err(|_| CryptoError::new(RingError("encryption failed")))?;
        let mut out = Vec::with_capacity(1 + FINGERPRINT_LEN + NONCE_LEN + sealed.len());
        out.push(SEAL_V2);
        out.extend_from_slice(&self.current.fingerprint);
        out.extend_from_slice(&nonce);
        out.append(&mut sealed);
        Ok(out)
    }

    fn open(&self, sealed: &[u8], context: &[u8]) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
        sealed
            .split_first()
            .and_then(|(&version, rest)| {
                let (fingerprint, rest) = rest.split_at_checked(FINGERPRINT_LEN)?;
                let (nonce, ciphertext) = rest.split_at_checked(NONCE_LEN)?;
                let key = self.keys().find(|key| key.fingerprint == fingerprint)?;
                key.open(version, nonce, ciphertext, context)
            })
            .ok_or_else(|| CryptoError::new(RingError("decryption failed")))
    }

    fn is_valid_public_key(&self, algorithm: PublicKeyAlgorithm, public_key: &[u8]) -> bool {
        raw_public_key(algorithm, public_key).is_some()
    }

    fn verify_signature(
        &self,
        algorithm: PublicKeyAlgorithm,
        public_key: &[u8],
        message: &[u8],
        signature: &[u8],
    ) -> bool {
        let Some(raw) = raw_public_key(algorithm, public_key) else {
            return false;
        };
        let verification: &dyn signature::VerificationAlgorithm = match algorithm {
            PublicKeyAlgorithm::Es256 => &signature::ECDSA_P256_SHA256_ASN1,
            PublicKeyAlgorithm::Rs256 => &signature::RSA_PKCS1_2048_8192_SHA256,
            PublicKeyAlgorithm::EdDsa => &signature::ED25519,
        };
        UnparsedPublicKey::new(verification, raw)
            .verify(message, signature)
            .is_ok()
    }
}

const OID_EC_PUBLIC_KEY: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01];
const OID_P256: &[u8] = &[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07];
const OID_RSA_ENCRYPTION: &[u8] = &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01];
const OID_ED25519: &[u8] = &[0x2b, 0x65, 0x70];

fn raw_public_key(algorithm: PublicKeyAlgorithm, spki: &[u8]) -> Option<&[u8]> {
    let mut outer = Der(spki);
    let mut info = Der(outer.read(0x30)?);
    if !outer.0.is_empty() {
        return None;
    }
    let mut identifier = Der(info.read(0x30)?);
    let oid = identifier.read(0x06)?;
    let key = info.read(0x03)?.strip_prefix(&[0])?;
    if !info.0.is_empty() {
        return None;
    }

    let matches = match algorithm {
        PublicKeyAlgorithm::Es256 => {
            oid == OID_EC_PUBLIC_KEY
                && identifier.read(0x06)? == OID_P256
                && key.len() == 65
                && key[0] == 0x04
        }
        PublicKeyAlgorithm::Rs256 => oid == OID_RSA_ENCRYPTION,
        PublicKeyAlgorithm::EdDsa => oid == OID_ED25519 && key.len() == 32,
    };
    matches.then_some(key)
}

struct Der<'a>(&'a [u8]);

impl<'a> Der<'a> {
    fn read(&mut self, tag: u8) -> Option<&'a [u8]> {
        let (&actual, rest) = self.0.split_first()?;
        if actual != tag {
            return None;
        }
        let (&first, mut rest) = rest.split_first()?;
        let len = if first < 0x80 {
            usize::from(first)
        } else {
            let count = usize::from(first & 0x7f);
            if count == 0 || count > 4 {
                return None;
            }
            let (bytes, tail) = rest.split_at_checked(count)?;
            rest = tail;
            bytes
                .iter()
                .fold(0usize, |len, &byte| (len << 8) | usize::from(byte))
        };
        let (contents, tail) = rest.split_at_checked(len)?;
        self.0 = tail;
        Some(contents)
    }
}
