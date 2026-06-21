// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

import { defineConfig, devices } from "@playwright/test";

// End-to-end tests for the shell, driven against the example MCP server
// (examples/mcp-ui-demo): it serves the baked shell plus an open /mcp endpoint with echo/add
// tools and a sample catalog action. The server binary must be built first
// (`cargo build -p mcp-ui-demo`); CI builds it, then runs `npm run test:e2e`. Locally an
// already-running demo on :8080 is reused.
export default defineConfig({
  testDir: "./e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? "github" : "list",
  use: {
    baseURL: "http://127.0.0.1:8080",
    trace: "on-first-retry",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: "../target/debug/mcp-ui-demo",
    url: "http://127.0.0.1:8080/",
    reuseExistingServer: !process.env.CI,
    timeout: 30_000,
  },
});
