//! Runtime secrets embedded as a dotenvx-encrypted file and decrypted at startup.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use std::{env, fmt};

/// The encrypted production configuration, embedded at compile time.
const ENCRYPTED_ENV: &str = include_str!("../../.env.production");
const PRIVATE_KEY_VAR: &str = "DOTENV_PRIVATE_KEY_PRODUCTION";
const REQUIRED_KEYS: [&str; 6] = [
    "IPFS_GATEWAY",
    "MERKLE_API_BEARER_TOKEN",
    "PINATA_ACCESS_TOKEN",
    "PINATA_API_KEY",
    "PINATA_API_SERVER",
    "PINATA_SECRET_API_KEY",
];

/// Reasons the embedded configuration cannot be used.
#[derive(Debug, PartialEq, Eq)]
pub enum SecretsError {
    MissingPrivateKey,
    InvalidPrivateKey,
    InvalidLine(usize),
    PrivateKeyInFile(String),
    Unencrypted(String),
    Decrypt(String),
    MissingKey(&'static str),
}

impl fmt::Display for SecretsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPrivateKey => write!(f, "{PRIVATE_KEY_VAR} must be set"),
            Self::InvalidPrivateKey => write!(f, "{PRIVATE_KEY_VAR} must be a 32-byte hex string"),
            Self::InvalidLine(line) => write!(f, "invalid encrypted env line {line}"),
            Self::PrivateKeyInFile(key) => write!(f, "private key {key} must not be in the encrypted env file"),
            Self::Unencrypted(key) => write!(f, "{key} is not encrypted"),
            Self::Decrypt(key) => write!(f, "cannot decrypt {key}"),
            Self::MissingKey(key) => write!(f, "{key} is missing from the encrypted env file"),
        }
    }
}

impl std::error::Error for SecretsError {}

/// Decrypts the embedded production configuration with `DOTENV_PRIVATE_KEY_PRODUCTION` and exports every value to
/// the process environment, overriding any existing value.
///
/// Deployed Vercel environments fail to start without the key. Local runs without it keep the process environment.
pub fn load() -> Result<(), SecretsError> {
    let Ok(private_key) = env::var(PRIVATE_KEY_VAR) else {
        return match env::var("VERCEL_ENV").as_deref() {
            Ok("production" | "preview") => Err(SecretsError::MissingPrivateKey),
            _ => Ok(()),
        };
    };
    for (key, value) in decrypt_env(ENCRYPTED_ENV, &private_key)? {
        env::set_var(key, value);
    }
    Ok(())
}

/// Parses a dotenvx-encrypted env file and decrypts every value with the hex-encoded secp256k1 private key.
///
/// Every value must be encrypted and every required key must be present; the public key entry is ignored.
pub fn decrypt_env(contents: &str, private_key_hex: &str) -> Result<Vec<(String, String)>, SecretsError> {
    let private_key = hex::decode(private_key_hex.trim()).map_err(|_| SecretsError::InvalidPrivateKey)?;
    if private_key.len() != 32 {
        return Err(SecretsError::InvalidPrivateKey);
    }
    let mut secrets = Vec::new();
    for (index, line) in contents.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(SecretsError::InvalidLine(index + 1));
        };
        if key.starts_with("DOTENV_PUBLIC_KEY") {
            continue;
        }
        if key.starts_with("DOTENV_PRIVATE_KEY") {
            return Err(SecretsError::PrivateKeyInFile(key.to_string()));
        }
        let Some(ciphertext) = value.trim_matches('"').strip_prefix("encrypted:") else {
            return Err(SecretsError::Unencrypted(key.to_string()));
        };
        let decrypt = || SecretsError::Decrypt(key.to_string());
        let ciphertext = STANDARD.decode(ciphertext).map_err(|_| decrypt())?;
        let plaintext = ecies::decrypt(&private_key, &ciphertext).map_err(|_| decrypt())?;
        let plaintext = String::from_utf8(plaintext).map_err(|_| decrypt())?;
        secrets.push((key.to_string(), plaintext));
    }
    for required in REQUIRED_KEYS {
        if !secrets.iter().any(|(key, _)| key == required) {
            return Err(SecretsError::MissingKey(required));
        }
    }
    Ok(secrets)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Throwaway key pair; the file was produced by `dotenvx encrypt` 2.38.2 to pin wire compatibility.
    const PRIVATE_KEY: &str = "48a1b73888827ebd8e13ba51b9811d0888a78a482cffb938190b423a35409de2";
    const ENCRYPTED: &str = r#"#/-------------------[DOTENV_PUBLIC_KEY]--------------------/
#/            public-key encryption for .env files          /
#/----------------------------------------------------------/
DOTENV_PUBLIC_KEY_PRODUCTION="02ffc44eb529b1a5e1da4fb49c599fb70f3b4fea1278f3e3819b4ffc2f376b4e57"

# .env.production
IPFS_GATEWAY="encrypted:BHGwjg0VI3k3uEZfQSnmpp/Ywx2v3aSKlMDHTPPi1qBO47B9t6xiuZIqeiQnhUJIC/glh5xuAh8Ubk6Beu+APpF7d3wA9M63Xl/lQwtR0sOn/oq18sAAVrvxsvYe4G24aXgjDfgPlguXNXqBVT4k7V41T6lN"
MERKLE_API_BEARER_TOKEN="encrypted:BIGrS8utTogROWeUCmRb4+gmxiF1ZX2rj5vQWH5Ze3cn/3QGBCAaubl4gN0Yak0nA4DTbETihbYvrSz08aJWmRPQsbvy2TMHHybWWbo3P/nYnYRwyijBd/ni+g4/7tnLK1aFd1CMtfn/B0qPjDOI"
PINATA_ACCESS_TOKEN="encrypted:BK0CsiIIwNP+S1perOeQPVoe8tE+HIWHBYWaSAhRWMFX9KeadC0VzIRBfLmHu/rbCm3EGwmdTI5Ryn+FNtM4U1b91LMXX8EN/jQH5YU+P63p5z+MZeGR4U+UOmvV4LyVehQRM8z4VlcXErrpA5AW"
PINATA_API_KEY="encrypted:BFT932ZmUL/WBrOh8O3RGnJoqFZ4fOwVpZ/h8N9ygGU0+g7dNNRJRu89mXOFYs/y4hGAZW5ELDkHNJp+74EBAksoMPoAPDZbvwt4ofvCi2D+Zzw4ARBFLbEC0bxtgEHGRIbZlfvXcHC+Bu5y"
PINATA_API_SERVER="encrypted:BHI8NYxFFNpx9Un5K4uuNAFMUFy1Qn53vq2eOU/Sar4d7kqcnVBO97NHfeo4EaE49xn6pX7OVA3VSfKtMsz27SZFIp+R8zomvadYYCcFNyLaNg3mb8eEDKg44GPsvN1rtQoxWzyVKuunF1ALKtryr1naH5o="
PINATA_SECRET_API_KEY="encrypted:BA64Z9B3vPwHnXSORSNnzS9Sjg9iVo6Iz9O97zm+9DpNuU1WDn+T2Kr85bw/ACnzJk+TDLqPWSztKS+xwkHBYaWvBRHJ5EScytvvLNk6XvGQabhUrVzUAhNATTKyF14zSzDDehe+C8l9Aa86692p"
"#;

    #[test]
    fn decrypts_dotenvx_cli_output() {
        let secrets = decrypt_env(ENCRYPTED, PRIVATE_KEY).unwrap();
        let expected = [
            ("IPFS_GATEWAY", "https://gateway.test"),
            ("MERKLE_API_BEARER_TOKEN", "fixture-bearer"),
            ("PINATA_ACCESS_TOKEN", "fixture-access"),
            ("PINATA_API_KEY", "fixture-key"),
            ("PINATA_API_SERVER", "https://pinata.test"),
            ("PINATA_SECRET_API_KEY", "fixture-secret"),
        ];
        let expected: Vec<(String, String)> =
            expected.iter().map(|(key, value)| (key.to_string(), value.to_string())).collect();
        assert_eq!(secrets, expected);
    }

    #[test]
    fn rejects_bad_private_keys() {
        assert_eq!(decrypt_env(ENCRYPTED, "nope"), Err(SecretsError::InvalidPrivateKey));
        assert_eq!(decrypt_env(ENCRYPTED, "ab"), Err(SecretsError::InvalidPrivateKey));
        assert_eq!(decrypt_env(ENCRYPTED, &"1".repeat(64)), Err(SecretsError::Decrypt("IPFS_GATEWAY".to_string())));
    }

    #[test]
    fn rejects_corrupt_ciphertext() {
        let corrupt = ENCRYPTED.replacen("encrypted:BHGw", "encrypted:BHGx", 1);
        assert_eq!(decrypt_env(&corrupt, PRIVATE_KEY), Err(SecretsError::Decrypt("IPFS_GATEWAY".to_string())));
        let truncated = ENCRYPTED.replace('/', "");
        assert_eq!(decrypt_env(&truncated, PRIVATE_KEY), Err(SecretsError::Decrypt("IPFS_GATEWAY".to_string())));
    }

    #[test]
    fn rejects_plaintext_private_keys_and_missing_keys() {
        let plaintext = ENCRYPTED.replacen("PINATA_API_KEY=\"encrypted:", "PINATA_API_KEY=\"", 1);
        assert_eq!(decrypt_env(&plaintext, PRIVATE_KEY), Err(SecretsError::Unencrypted("PINATA_API_KEY".to_string())));
        let leaked = format!("{ENCRYPTED}DOTENV_PRIVATE_KEY_PRODUCTION=\"{PRIVATE_KEY}\"\n");
        assert_eq!(
            decrypt_env(&leaked, PRIVATE_KEY),
            Err(SecretsError::PrivateKeyInFile("DOTENV_PRIVATE_KEY_PRODUCTION".to_string()))
        );
        let missing: String =
            ENCRYPTED.lines().filter(|l| !l.starts_with("PINATA_API_KEY=")).collect::<Vec<_>>().join("\n");
        assert_eq!(decrypt_env(&missing, PRIVATE_KEY), Err(SecretsError::MissingKey("PINATA_API_KEY")));
        assert_eq!(decrypt_env("garbage", PRIVATE_KEY), Err(SecretsError::InvalidLine(1)));
    }
}
