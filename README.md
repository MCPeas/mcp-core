# mcp-core

Shared infrastructure for MCP and web servers in Rust.

## Features

- **auth**: Token-based authentication middleware (Bearer and Basic Auth)
- **config**: Configuration management with environment variable support
- **transport**: SSE transport for MCP HTTP mode
- **bootstrap**: Tracing initialization utilities

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
mcp-core = { git = "https://github.com/MCPeas/mcp-core" }

# Or with specific features
mcp-core = { git = "https://github.com/MCPeas/mcp-core", features = ["full"] }
```

### Authentication Middleware

```rust
use mcp_core::TokenAuthLayer;
use axum::{Router, routing::get};

let app = Router::new()
    .route("/api", get(handler))
    .layer(TokenAuthLayer::new("my-secret-token".to_string()));
```

### Configuration

```rust
use mcp_core::BaseConfig;

let config = BaseConfig::from_env();
let (token, was_generated) = config.get_or_generate_token();

if was_generated {
    println!("Generated auth token: {}", token);
}
```

Environment variables:
- `HOST` - Server bind address (default: `127.0.0.1`)
- `PORT` - Server port (default: `3000`)
- `DATA_PATH` - Base path for data files (default: `./data`)
- `AUTH_TOKEN` - Optional authentication token

### SSE Transport (MCP HTTP Mode)

```rust
use mcp_core::{AuthSseServer, TokenAuthLayer};

let (mut sse_server, sse_router) = AuthSseServer::new();

// Wrap with auth middleware
let protected_router = sse_router.layer(TokenAuthLayer::new("secret".to_string()));

// Accept connections
while let Some(transport) = sse_server.next_transport().await {
    // Handle the MCP session...
}
```

### Tracing

```rust
use mcp_core::init_tracing;

// Enable debug logging for your crate, info for everything else
init_tracing("my_server=debug,info");
```

## License

Copyright (C) 2025-2026 Stefan Grönke <stefan@gronke.net>.

mcp-core is free software: you can redistribute it and/or modify it under the
terms of the GNU Affero General Public License as published by the Free Software
Foundation, either version 3 of the License, or (at your option) any later version
(`AGPL-3.0-or-later`). See [LICENSE](LICENSE) for the full text.

Source: <https://github.com/MCPeas/mcp-core>
