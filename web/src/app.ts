// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

// Shell entry point. Registers every shell component + built-in renderer, mounts the
// shell, then loads the consumer MCP's declarative manifest (URL from the
// `mcp-ui-manifest` meta tag, default `/app/components.js`) and hands it to the shell.
// It also probes the catalog endpoint so the built-in Catalog view is shown only when
// the server actually has one.

import "./shell/app-shell.js";
import "./renderers/esm.js";
import "./renderers/html.js";
import "./renderers/md.js";
import "./renderers/json.js";
import "./renderers/catalog-item.js";
// Typed-catalog explorer: registers the `entity` content kind + <mcp-entity>/<mcp-entity-list>
// and the built-in field renderers, so a server with a DataCatalog is explorable with no code.
import "./shell/entity.js";
import "./shell/entity-list.js";
// Safe sandboxed-iframe + markdown elements (used by the html/md renderers and markdown fields).
import "./shell/frame.js";
import "./shell/markdown.js";
import { apiBase, isReachable } from "./shell/endpoints.js";
import type { Manifest, ViewDescriptor } from "./shell/types.js";

interface ShellElement extends HTMLElement {
  setManifest(
    manifest: Manifest,
    opts?: { catalog?: boolean; search?: boolean; typedCatalog?: ViewDescriptor[] },
  ): void;
}

/** Built-in sidebar views for a server with a typed DataCatalog: one list per browsable type
 *  (from `{api-base}/catalog/schema`). Empty when the server has no typed catalog. */
async function typedCatalogViews(): Promise<ViewDescriptor[]> {
  try {
    const res = await fetch(`${apiBase()}/catalog/schema`);
    if (!res.ok) {
      return [];
    }
    const types = (await res.json()) as { name: string; title: string; list?: boolean }[];
    return types
      .filter((t) => t.list)
      .map(
        (t): ViewDescriptor => ({
          id: `catalog-${t.name}`,
          title: t.title,
          layout: "list",
          element: "mcp-entity-list",
          props: { entityType: t.name },
        }),
      );
  } catch {
    return [];
  }
}

const shell = document.createElement("mcp-app-shell") as ShellElement;
document.body.appendChild(shell);

const [catalog, search, typedCatalog] = await Promise.all([
  isReachable(`${apiBase()}/catalog/items`),
  isReachable(`${apiBase()}/search`),
  typedCatalogViews(),
]);

const meta = document.querySelector('meta[name="mcp-ui-manifest"]');
const url = meta?.getAttribute("content") ?? "/app/components.js";

try {
  const mod = (await import(url)) as { default?: Manifest; manifest?: Manifest };
  const manifest = mod.default ?? mod.manifest;
  if (!manifest) {
    throw new Error("module has no default export (the manifest)");
  }
  shell.setManifest(manifest, { catalog, search, typedCatalog });
} catch (err) {
  console.error(`mcp-ui: failed to load manifest from ${url}`, err);
  shell.setManifest({ title: "MCP", views: [] }, { catalog, search, typedCatalog });
}
