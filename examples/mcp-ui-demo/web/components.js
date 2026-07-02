// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

// The consumer MCP's registration manifest, default-exported. Plain ES module - no build
// step. Bare specifiers (`lit`, `mcp-ui`) resolve through the shell's import map.
//
// Demonstrates the data-viewing core: a list view whose items render via the JSON
// fallback (an unknown `record` type) or a custom `esm` override. The built-in
// "Operations" view is added by the shell automatically.

export default {
  title: "MCP UI Demo",
  views: [
    {
      id: "records",
      title: "Records",
      icon: "\u{1F4C4}",
      layout: "list",
      element: "demo-items",
      list: () => import("/app/items-list.js"),
    },
  ],
};
