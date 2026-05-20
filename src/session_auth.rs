use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rand_distr::Alphanumeric;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

static PASSWORD_SALT: &str = "agfg34g";

pub type Password = [u8; 32];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionAuth {
    pub username: String,
    pub hashed_password: Password,
}

impl SessionAuth {
    pub fn new(username: String, password: String) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(format!("{password}{PASSWORD_SALT}"));
        let hashed_password = hasher.finalize().to_vec()[..]
            .try_into()
            .expect("Hash should be 32 bytes long.");

        Self {
            username,
            hashed_password,
        }
    }

    pub fn check_password(&self, password: Password) -> bool {
        self.hashed_password == password
    }
}

pub fn generate_user_id() -> String {
    let buf_id = ChaCha8Rng::from_rng(&mut rand::rng())
        .sample_iter(&Alphanumeric)
        .take(8)
        .collect::<Vec<u8>>()
        .to_ascii_lowercase();

    std::str::from_utf8(buf_id.as_slice())
        .expect("Failed to generate user id string")
        .to_string()
}
