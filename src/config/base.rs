// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Base configuration for MCP and web servers.

use super::safe_path::{safe_resolve, SafePathError};
use super::token::generate_random_token;
use std::path::PathBuf;

/// Base configuration shared by MCP and web servers.
///
/// Reads from environment variables with sensible defaults:
///
/// | Variable | Default | Description |
/// |----------|---------|-------------|
/// | `HOST` | `127.0.0.1` | Server bind address |
/// | `PORT` | `3000` | Server port |
/// | `DATA_PATH` | `./data` | Base path for data files |
/// | `AUTH_TOKEN` | (none) | Optional auth token |
///
/// # Example
///
/// ```rust
/// use mcp_core::BaseConfig;
///
/// let config = BaseConfig::from_env();
/// let (token, was_generated) = config.get_or_generate_token();
///
/// if was_generated {
///     println!("Generated auth token: {}", token);
/// }
/// ```
#[derive(Clone, Debug)]
pub struct BaseConfig {
    /// Server bind address (default: 127.0.0.1)
    pub host: String,
    /// Server port (default: 3000)
    pub port: u16,
    /// Base path for data files (default: ./data)
    pub data_path: PathBuf,
    /// Optional authentication token
    pub auth_token: Option<String>,
}

impl BaseConfig {
    /// Create a new config from environment variables.
    pub fn from_env() -> Self {
        Self {
            host: std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            port: std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
            data_path: std::env::var("DATA_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("./data")),
            auth_token: std::env::var("AUTH_TOKEN").ok(),
        }
    }

    /// Check if authentication is enabled.
    pub fn auth_enabled(&self) -> bool {
        self.auth_token.is_some()
    }

    /// Get the configured token or generate a new one.
    ///
    /// Returns a tuple of (token, was_generated).
    pub fn get_or_generate_token(&self) -> (String, bool) {
        match &self.auth_token {
            Some(token) => (token.clone(), false),
            None => {
                let token = generate_random_token();
                (token, true)
            }
        }
    }

    /// Get the socket address for binding.
    pub fn socket_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// Safely resolve a user-provided path within the data directory.
    ///
    /// Returns the canonicalized path if it stays within [`data_path`](Self::data_path).
    /// Rejects `..` traversal, absolute paths, and symlinks pointing outside.
    pub fn resolve_data_path(&self, user_path: &str) -> Result<PathBuf, SafePathError> {
        safe_resolve(&self.data_path, user_path)
    }
}

impl Default for BaseConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_values() {
        // Clear env vars to test defaults
        std::env::remove_var("HOST");
        std::env::remove_var("PORT");
        std::env::remove_var("DATA_PATH");
        std::env::remove_var("AUTH_TOKEN");

        let config = BaseConfig::from_env();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 3000);
        assert_eq!(config.data_path, PathBuf::from("./data"));
        assert!(config.auth_token.is_none());
        assert!(!config.auth_enabled());
    }

    #[test]
    fn test_socket_addr() {
        let config = BaseConfig {
            host: "0.0.0.0".to_string(),
            port: 8080,
            data_path: PathBuf::from("./data"),
            auth_token: None,
        };
        assert_eq!(config.socket_addr(), "0.0.0.0:8080");
    }

    #[test]
    fn test_get_or_generate_token_with_existing() {
        let config = BaseConfig {
            host: "127.0.0.1".to_string(),
            port: 3000,
            data_path: PathBuf::from("./data"),
            auth_token: Some("my-token".to_string()),
        };
        let (token, generated) = config.get_or_generate_token();
        assert_eq!(token, "my-token");
        assert!(!generated);
    }

    #[test]
    fn test_get_or_generate_token_without_existing() {
        let config = BaseConfig {
            host: "127.0.0.1".to_string(),
            port: 3000,
            data_path: PathBuf::from("./data"),
            auth_token: None,
        };
        let (token, generated) = config.get_or_generate_token();
        assert_eq!(token.len(), 32);
        assert!(generated);
    }
}
