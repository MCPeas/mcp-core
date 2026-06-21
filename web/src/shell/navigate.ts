// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

import type { ContentRef } from "./types.js";

/**
 * Ask the shell to show `content` in the active view's content pane, by dispatching the
 * bubbling, composed `mcp-navigate` event that the list/detail layout listens for at its
 * root. A list selection, a search result, and a content renderer (e.g. a "next paragraph"
 * link or a cross-reference) all drive the detail pane through this one event.
 */
export function navigate(source: EventTarget, content: ContentRef): void {
  source.dispatchEvent(
    new CustomEvent("mcp-navigate", {
      detail: { content },
      bubbles: true,
      composed: true,
    }),
  );
}
