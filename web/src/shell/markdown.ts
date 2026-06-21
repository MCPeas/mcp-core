// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

import { LitElement, html, type TemplateResult, type PropertyValues } from "lit";
import { property, state } from "lit/decorators.js";
import { markdownEnabled } from "./endpoints.js";
import "./frame.js";

/**
 * Convert markdown to a self-contained, sandboxed HTML document, or `null` when markdown
 * rendering is not built in (the crate's `web-markdown` feature is off, so `marked` is not
 * vendored). `marked` is imported lazily, so it is only fetched when a server opted in and a
 * markdown value is actually rendered.
 */
export async function markdownToHtml(text: string): Promise<string | null> {
  if (!markdownEnabled()) {
    return null;
  }
  const { marked } = await import("marked");
  return wrapDocument(marked.parse(text) as string);
}

/** Wrap rendered-markdown HTML in a minimal document with a strict CSP (no scripts, inline
 *  styles only) -- defense in depth on top of the iframe sandbox. */
function wrapDocument(body: string): string {
  return `<!doctype html><html><head><meta charset="utf-8" />
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src data: https:; style-src 'unsafe-inline'" />
<style>
  html, body { margin: 0; }
  body { font: 16px/1.6 system-ui, sans-serif; color: #212529; overflow-wrap: break-word; }
  :first-child { margin-top: 0; }
  pre { background: #f6f8fa; padding: 0.75rem; border-radius: 0.375rem; overflow: auto; }
  code { font-family: ui-monospace, SFMono-Regular, monospace; }
  table { border-collapse: collapse; }
  th, td { border: 1px solid #dee2e6; padding: 0.25rem 0.5rem; }
  img { max-width: 100%; }
  a { color: #4c6ef5; }
</style></head><body>${body}</body></html>`;
}

/**
 * `<mcp-markdown text="...">` -- render a markdown string. When the `web-markdown` feature is
 * built in, the markdown is converted and shown in a sandboxed, auto-height `<mcp-frame>`; when
 * it is not, the raw text is shown preformatted (still readable, just unstyled). Field and
 * content renderers delegate here, so there is a single markdown code path.
 */
export class McpMarkdown extends LitElement {
  @property({ type: String }) text = "";
  /** Converted HTML document, or null when markdown is not built in. */
  @state() private doc: string | null = null;
  /** Whether conversion has resolved (so the fallback does not flash before the iframe). */
  @state() private ready = false;

  createRenderRoot(): this {
    return this;
  }

  willUpdate(changed: PropertyValues): void {
    if (changed.has("text")) {
      this.ready = false;
      void this.convert();
    }
  }

  private async convert(): Promise<void> {
    this.doc = await markdownToHtml(this.text);
    this.ready = true;
  }

  render(): TemplateResult {
    if (!this.ready) {
      return html``;
    }
    if (this.doc === null) {
      // Markdown not built in: show the raw text preformatted (escaped by Lit, so it is safe).
      return html`<div class="mcp-field-markdown" style="white-space: pre-wrap">${this.text}</div>`;
    }
    return html`<mcp-frame .srcdoc=${this.doc} .fit=${true}></mcp-frame>`;
  }
}

customElements.define("mcp-markdown", McpMarkdown);
