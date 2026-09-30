//! Fakes for the non-storage ports: a clock that only moves when told to, a hasher that is fast
//! and transparent, predictable tokens and cryptography, a mailer, a texter and a social sign-in
//! provider that record.

use std::{
    collections::{BTreeMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
};

use domain::{
    clock::Clock,
    identity::{
        AuthorizationRequest, IdentityProviders, OAuthError, OAuthFuture, ProviderInfo,
        ProviderProfile,
    },
    mail::{Mail, MailFuture, Mailer},
    object_store::{ByteStream, NewObject, Object, ObjectKey, ObjectStore, ObjectStoreError},
    one_time_code::CodeChannel,
    passkey::PublicKeyAlgorithm,
    secret::{Secret, TokenHash},
    security::{Crypto, CryptoError, HashError, PasswordHasher, TokenError, TokenGenerator},
    text::{TextFuture, TextMessage, TextSender},
    user::PasswordHash,
};
use time::{Duration, OffsetDateTime, macros::datetime};
use zeroize::Zeroizing;

pub const START: OffsetDateTime = datetime!(2026-01-01 09:00 UTC);

/// Objects in memory. `put` reads the body to its end and refuses one of another length than
/// announced, like the real store; `set_down` makes every call fail.
#[derive(Clone, Default)]
pub struct FakeObjects {
    objects: Arc<Mutex<BTreeMap<ObjectKey, Vec<u8>>>>,
    down: Arc<Mutex<bool>>,
}

impl FakeObjects {
    pub fn set_down(&self, down: bool) {
        *self.down.lock().unwrap() = down;
    }

    pub fn keys(&self) -> Vec<ObjectKey> {
        self.objects.lock().unwrap().keys().cloned().collect()
    }

    pub fn contents(&self, key: &ObjectKey) -> Option<Vec<u8>> {
        self.objects.lock().unwrap().get(key).cloned()
    }

    fn check_up(&self) -> Result<(), ObjectStoreError> {
        if *self.down.lock().unwrap() {
            Err(ObjectStoreError::backend(std::io::Error::other(
                "the store is down",
            )))
        } else {
            Ok(())
        }
    }
}

impl ObjectStore for FakeObjects {
    async fn put(&self, key: &ObjectKey, object: NewObject) -> Result<(), ObjectStoreError> {
        use futures_util::StreamExt;

        self.check_up()?;
        let mut body = object.body;
        let mut contents = Vec::new();
        while let Some(chunk) = body.next().await {
            contents.extend_from_slice(&chunk?);
        }
        if contents.len() as u64 != object.length {
            return Err(ObjectStoreError::body(std::io::Error::other(
                "wrong length",
            )));
        }
        self.objects.lock().unwrap().insert(key.clone(), contents);
        Ok(())
    }

    async fn get(&self, key: &ObjectKey) -> Result<Option<Object>, ObjectStoreError> {
        self.check_up()?;
        Ok(self.objects.lock().unwrap().get(key).map(|contents| {
            let body: ByteStream = Box::pin(futures_util::stream::iter([Ok(bytes::Bytes::from(
                contents.clone(),
            ))]));
            Object {
                length: contents.len() as u64,
                body,
            }
        }))
    }

    async fn delete(&self, key: &ObjectKey) -> Result<(), ObjectStoreError> {
        self.check_up()?;
        self.objects.lock().unwrap().remove(key);
        Ok(())
    }
}

#[derive(Clone)]
pub struct FakeClock(Arc<Mutex<OffsetDateTime>>);

impl Default for FakeClock {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(START)))
    }
}

impl FakeClock {
    pub fn advance(&self, by: Duration) {
        *self.0.lock().unwrap() += by;
    }
}

impl Clock for FakeClock {
    fn now(&self) -> OffsetDateTime {
        *self.0.lock().unwrap()
    }
}

/// "Hashes" by prefixing, and counts verifications so tests can check that unknown accounts still
/// cost a verification.
#[derive(Clone, Default)]
pub struct FakeHasher {
    verifications: Arc<AtomicUsize>,
}

impl FakeHasher {
    pub fn verifications(&self) -> usize {
        self.verifications.load(Ordering::SeqCst)
    }
}

impl PasswordHasher for FakeHasher {
    async fn hash(&self, password: &Secret) -> Result<PasswordHash, HashError> {
        Ok(PasswordHash::new(format!("hashed:{}", password.expose())))
    }

    async fn verify(
        &self,
        password: &Secret,
        hash: Option<&PasswordHash>,
    ) -> Result<bool, HashError> {
        self.verifications.fetch_add(1, Ordering::SeqCst);
        let expected = format!("hashed:{}", password.expose());
        Ok(hash.is_some_and(|hash| {
            hash.as_str() == expected || hash.as_str() == format!("old-{expected}")
        }))
    }

    fn needs_rehash(&self, hash: &PasswordHash) -> bool {
        hash.as_str().starts_with("old-")
    }
}

pub fn fake_digest(data: &[u8]) -> [u8; 32] {
    let mut digest = [0u8; 32];
    for (seed, chunk) in digest.chunks_mut(8).enumerate() {
        let mut hasher = DefaultHasher::new();
        seed.hash(&mut hasher);
        data.hash(&mut hasher);
        chunk.copy_from_slice(&hasher.finish().to_le_bytes());
    }
    digest
}

/// Transparent stand-ins for the cryptography: counters for randomness, [`fake_digest`] for
/// hashes, "sealing" that only adds a prefix, and a "signature" that is the digest of the public
/// key and the message, so tests can sign with [`FakeCrypto::sign`].
#[derive(Clone, Default)]
pub struct FakeCrypto(Arc<AtomicU64>);

impl FakeCrypto {
    pub const SEAL_PREFIX: &[u8] = b"sealed:";

    pub fn sign(public_key: &[u8], message: &[u8]) -> Vec<u8> {
        let mut input = public_key.to_vec();
        input.extend_from_slice(message);
        fake_digest(&input).to_vec()
    }

    fn next(&self) -> u64 {
        self.0.fetch_add(1, Ordering::SeqCst) + 1
    }
}

impl Crypto for FakeCrypto {
    fn random_bytes(&self, len: usize) -> Result<Vec<u8>, TokenError> {
        let seed = self.next().to_le_bytes();
        Ok((0..len)
            .map(|i| seed[i % 8].wrapping_add(u8::try_from(i % 256).unwrap()))
            .collect())
    }

    fn random_digits(&self, digits: u32) -> Result<Secret, TokenError> {
        let n = self.next() % 10u64.pow(digits);
        Ok(Secret::new(format!("{n:0width$}", width = digits as usize)))
    }

    fn sha256(&self, data: &[u8]) -> [u8; 32] {
        fake_digest(data)
    }

    fn hmac_sha1(&self, key: &[u8], message: &[u8]) -> [u8; 20] {
        let mut input = key.to_vec();
        input.extend_from_slice(message);
        let digest = fake_digest(&input);
        let mut mac = [0u8; 20];
        mac.copy_from_slice(&digest[..20]);
        mac
    }

    fn keyed_digest(&self, purpose: &str, data: &[u8]) -> [u8; 32] {
        let mut input = b"pepper:".to_vec();
        input.extend_from_slice(purpose.as_bytes());
        input.push(0);
        input.extend_from_slice(data);
        fake_digest(&input)
    }

    fn previous_keyed_digest(&self, _purpose: &str, _data: &[u8]) -> Option<[u8; 32]> {
        None
    }

    fn seal(&self, plaintext: &[u8], context: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let mut sealed = Self::SEAL_PREFIX.to_vec();
        sealed.push(u8::try_from(context.len()).unwrap());
        sealed.extend_from_slice(context);
        sealed.extend_from_slice(plaintext);
        Ok(sealed)
    }

    fn open(&self, sealed: &[u8], context: &[u8]) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
        let rest = sealed
            .strip_prefix(Self::SEAL_PREFIX)
            .and_then(|rest| rest.split_first())
            .and_then(|(len, rest)| rest.split_at_checked(usize::from(*len)))
            .filter(|(sealed_for, _)| *sealed_for == context);
        rest.map(|(_, plain)| Zeroizing::new(plain.to_vec()))
            .ok_or_else(|| CryptoError::new(std::io::Error::other("not sealed")))
    }

    fn is_valid_public_key(&self, _algorithm: PublicKeyAlgorithm, public_key: &[u8]) -> bool {
        public_key.starts_with(b"pk")
    }

    fn verify_signature(
        &self,
        _algorithm: PublicKeyAlgorithm,
        public_key: &[u8],
        message: &[u8],
        signature: &[u8],
    ) -> bool {
        signature == Self::sign(public_key, message)
    }
}

#[derive(Clone, Default)]
pub struct FakeTexts(Arc<Mutex<Vec<TextMessage>>>);

impl FakeTexts {
    pub fn sent(&self) -> Vec<TextMessage> {
        self.0.lock().unwrap().clone()
    }

    pub fn last_code_to(&self, to: &str) -> String {
        let message = self
            .sent()
            .into_iter()
            .rev()
            .find(|message| message.to.as_str() == to)
            .expect("no text was sent");
        let words: Vec<&str> = message.body.split_whitespace().collect();
        let digits =
            |word: &str, len: usize| word.len() == len && word.chars().all(|c| c.is_ascii_digit());
        words
            .iter()
            .enumerate()
            .find_map(|(at, word)| {
                if digits(word, 6) {
                    Some((*word).to_owned())
                } else if digits(word, 3) && words.get(at + 1).is_some_and(|next| digits(next, 3)) {
                    Some(format!("{word}{}", words[at + 1]))
                } else {
                    None
                }
            })
            .expect("no code in the text")
    }
}

impl TextSender for FakeTexts {
    fn channels(&self) -> &[CodeChannel] {
        &[CodeChannel::Sms, CodeChannel::Whatsapp]
    }

    fn send(&self, message: TextMessage) -> TextFuture<'_> {
        self.0.lock().unwrap().push(message);
        Box::pin(async { Ok(()) })
    }
}

#[derive(Clone)]
pub struct FakeProviders {
    providers: Vec<ProviderInfo>,
    accounts: Arc<Mutex<Vec<(String, ProviderProfile)>>>,
    pub fail: Arc<std::sync::atomic::AtomicBool>,
}

impl Default for FakeProviders {
    fn default() -> Self {
        Self {
            providers: vec![ProviderInfo {
                id: "test".to_owned(),
                name: "Test".to_owned(),
            }],
            accounts: Arc::default(),
            fail: Arc::default(),
        }
    }
}

impl FakeProviders {
    pub fn add_account(&self, code: &str, profile: ProviderProfile) {
        self.accounts
            .lock()
            .unwrap()
            .push((code.to_owned(), profile));
    }
}

impl IdentityProviders for FakeProviders {
    fn providers(&self) -> &[ProviderInfo] {
        &self.providers
    }

    fn authorization_url(&self, request: &AuthorizationRequest<'_>) -> Result<String, OAuthError> {
        if request.provider != "test" {
            return Err(OAuthError::UnknownProvider);
        }
        Ok(format!("https://provider.test/?state={}", request.state))
    }

    fn exchange<'a>(
        &'a self,
        _provider: &'a str,
        code: &'a Secret,
        _pkce_verifier: &'a Secret,
        _nonce: &'a str,
    ) -> OAuthFuture<'a, ProviderProfile> {
        Box::pin(async move {
            if self.fail.load(Ordering::SeqCst) {
                return Err(OAuthError::failed(std::io::Error::other(
                    "provider is down",
                )));
            }
            self.accounts
                .lock()
                .unwrap()
                .iter()
                .find(|(known, _)| known == code.expose())
                .map(|(_, profile)| profile.clone())
                .ok_or_else(|| OAuthError::Rejected("unknown code".to_owned()))
        })
    }
}

#[derive(Clone, Default)]
pub struct FakeTokens(Arc<AtomicU64>);

impl TokenGenerator for FakeTokens {
    fn generate(&self) -> Result<Secret, TokenError> {
        let n = self.0.fetch_add(1, Ordering::SeqCst) + 1;
        Ok(Secret::new(format!("token-{n}")))
    }

    fn digest(&self, token: &Secret) -> TokenHash {
        TokenHash::new(fake_digest(token.expose().as_bytes()))
    }
}

#[derive(Clone, Default)]
pub struct FakeMailer(Arc<Mutex<Vec<Mail>>>);

impl FakeMailer {
    pub fn sent(&self) -> Vec<Mail> {
        self.0.lock().unwrap().clone()
    }

    pub fn last_to(&self, to: &str) -> Option<Mail> {
        self.sent()
            .into_iter()
            .rev()
            .find(|mail| mail.to.as_str() == to)
    }

    pub fn token_for(&self, to: &str) -> Secret {
        let mail = self.last_to(to).expect("no mail was sent");
        let (_, rest) = mail
            .body
            .split_once("#token=")
            .expect("no link in the mail");
        let token = rest.split_whitespace().next().unwrap();
        Secret::new(token)
    }
}

impl Mailer for FakeMailer {
    fn send(&self, mail: Mail) -> MailFuture<'_> {
        self.0.lock().unwrap().push(mail);
        Box::pin(async { Ok(()) })
    }
}
