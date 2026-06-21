// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

import type { ContentRef } from "./types.js";
import type { KnownContentKind, Open } from "./elements.js";

/** A content renderer: given a {@link ContentRef}, populate `host` with its UI. */
export interface Renderer {
  render(ref: ContentRef, host: HTMLElement): void | Promise<void>;
}

const renderers = new Map<string, Renderer>();

/**
 * Register a renderer for a {@link ContentRef} `type`. The shell ships `esm`, `html`
 * and `md`; consumers call this to add their own kinds (typically at the top level of
 * their `/components.js` so it runs before the first render).
 */
export function registerRenderer(type: Open<KnownContentKind>, renderer: Renderer): void {
  renderers.set(type, renderer);
}

/** Look up the renderer for a `type`, if one is registered. */
export function getRenderer(type: Open<KnownContentKind>): Renderer | undefined {
  return renderers.get(type);
}
