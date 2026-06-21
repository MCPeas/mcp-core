// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

// Small shared Lit templates for the list elements' loading / empty / error / action states,
// plus one tool-result helper. Extracted so catalog-list / search-list / entity-list render
// these the same way instead of each repeating the markup.

import { html, type TemplateResult } from "lit";
import type { ToolResult } from "./mcp-client.js";

/** The shell's standard error surface (a warning alert). */
export function errorAlert(message: unknown): TemplateResult {
  return html`<div class="alert alert-warning m-3">${String(message)}</div>`;
}

/** A muted loading note for a list pane. */
export function loadingNote(message = "Loading..."): TemplateResult {
  return html`<div class="p-3 text-muted">${message}</div>`;
}

/** A muted empty-state note for a list pane. */
export function emptyNote(message: string): TemplateResult {
  return html`<div class="p-3 text-muted">${message}</div>`;
}

/** The inline "an action is running" note shown on a row. */
export function workingNote(): TemplateResult {
  return html`<div class="small text-muted">Working...</div>`;
}

/** The first text block of a tool result (its human-readable message), if any. */
export function toolResultText(result: ToolResult): string | undefined {
  return result.content.find((b) => typeof b.text === "string")?.text;
}
