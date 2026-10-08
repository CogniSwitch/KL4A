//! Bearer-token generation/persistence. Stored as a sibling of
//! `sopkb_config::settings_path()` (i.e. the same `~/.sopkb` directory settings.json
//! already lives in) so it survives restarts without inventing a new config location.

use std::fs;
use std::path::PathBuf;

use rand::RngCore;

const TOKEN_FILENAME: &str = "server_token";

fn token_path() -> PathBuf {
    sopkb_config::settings_path().with_file_name(TOKEN_FILENAME)
}

fn generate() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Reads the persisted token if present; generates, persists (best-effort directory
/// creation), and returns a fresh one otherwise. `force_regenerate` always writes a
/// new one, overwriting any existing file (`--regenerate-token`).
pub fn load_or_create(force_regenerate: bool) -> std::io::Result<String> {
    let path = token_path();
    if !force_regenerate {
        if let Ok(existing) = fs::read_to_string(&path) {
            let trimmed = existing.trim();
            if !trimmed.is_empty() {
                return Ok(trimmed.to_string());
            }
        }
    }
    let token = generate();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(&path, &token)?;
    Ok(token)
}

pub fn path_for_display() -> PathBuf {
    token_path()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_produces_a_64_char_hex_string() {
        let token = generate();
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn generate_is_not_deterministic() {
        assert_ne!(generate(), generate());
    }
}
