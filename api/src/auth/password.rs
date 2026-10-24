use argon2::password_hash::{PasswordHasher as _, PasswordVerifier as _, phc::PasswordHash};
use argon2::{Algorithm, Argon2, Params, Version};

use crate::error::AppError;

#[derive(Clone)]
pub struct PasswordHasher {
    argon: Argon2<'static>,
    dummy_hash: String,
}

impl PasswordHasher {
    pub fn new(memory_kib: u32, iterations: u32, parallelism: u32) -> Result<Self, AppError> {
        let params = Params::new(memory_kib, iterations, parallelism, None).map_err(|_| {
            AppError::validation("argon2 parameters are outside the supported range")
        })?;
        let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let dummy_hash = argon
            .hash_password(b"edurust-dummy-password")
            .map_err(|_| AppError::Internal)?
            .to_string();
        Ok(Self { argon, dummy_hash })
    }

    pub fn hash(&self, password: &str) -> Result<String, AppError> {
        self.argon
            .hash_password(password.as_bytes())
            .map_err(|_| AppError::Internal)
            .map(|hash| hash.to_string())
    }

    pub fn verify(&self, password: &str, encoded: &str) -> bool {
        let Ok(parsed) = PasswordHash::new(encoded) else {
            return false;
        };
        self.argon
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
    }

    pub fn verify_dummy(&self, password: &str) {
        let _ = self.verify(password, &self.dummy_hash);
    }
}
