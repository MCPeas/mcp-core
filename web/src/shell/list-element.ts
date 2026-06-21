// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

import { LitElement } from "lit";
import { property } from "lit/decorators.js";
import { navigate } from "./navigate.js";
import type { ContentRef } from "./types.js";

/**
 * Base class for the list pane of a `list` view. Subclasses render their items and
 * call `this.select(id, contentRef)` when one is chosen; the shell routes that
 * {@link ContentRef} into the content pane. Renders into light DOM so Bootstrap's
 * global stylesheet applies.
 */
export class ListElement extends LitElement {
  @property({ type: String }) selectedId = "";

  createRenderRoot(): this {
    return this;
  }

  /** Select an item: remember it and ask the shell to route to its content (`mcp-navigate`). */
  select(id: string, content: ContentRef): void {
    this.selectedId = id;
    navigate(this, content);
  }
}
