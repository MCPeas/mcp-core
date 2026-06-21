// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Configuration management with environment variable support.

mod base;
pub mod safe_path;
mod token;

pub use base::BaseConfig;
pub use safe_path::{safe_resolve, SafePathError};
pub use token::generate_random_token;
