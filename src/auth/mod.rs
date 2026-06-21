// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Token-based authentication middleware.
//!
//! Supports both Bearer token and Basic Auth (with token as password).

mod middleware;

pub use middleware::{TokenAuthLayer, TokenAuthService};
