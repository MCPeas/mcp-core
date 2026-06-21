// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Tracing initialization utilities.

use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Initialize tracing with the given default filter.
///
/// The filter can be overridden by the `RUST_LOG` environment variable.
///
/// # Example
///
/// ```rust
/// use mcp_core::init_tracing;
///
/// // Enable debug logging for your crate, info for everything else
/// init_tracing("my_server=debug,info");
/// ```
///
/// # Filter Syntax
///
/// The filter follows the `tracing_subscriber::EnvFilter` syntax:
/// - `info` - Enable info level for all targets
/// - `my_crate=debug` - Enable debug level for `my_crate`
/// - `my_crate=debug,info` - Debug for `my_crate`, info for everything else
/// - `my_crate::module=trace` - Trace level for a specific module
pub fn init_tracing(default_filter: &str) {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));

    tracing_subscriber::registry()
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(filter)
        .init();
}

#[cfg(test)]
mod tests {
    // Note: tracing can only be initialized once per process,
    // so we can't really unit test init_tracing without affecting other tests.
    // Integration tests should verify this works correctly.
}
