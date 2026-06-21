// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

import { LitElement, html, type TemplateResult } from "lit";
import { property } from "lit/decorators.js";

/**
 * `<mcp-frame>` -- a sandboxed iframe for rendering untrusted HTML safely. Loads `srcdoc`
 * (inline HTML) or `src` (a URL) into an iframe that starts invisible and collapsed
 * (`opacity: 0; max-height: 0`), measures the loaded content's height, sizes itself to fit,
 * then reveals -- so embedded HTML reads as part of the page rather than a fixed-height box.
 *
 * Safety: the default sandbox is `allow-same-origin` with NO `allow-scripts`, so no script in
 * the document ever runs (untrusted content stays inert); same-origin only lets the parent read
 * the laid-out height to auto-size. The two are deliberately never combined -- that pairing would
 * let a script remove its own sandbox. `allowScripts` is an explicit opt-in for trusted content;
 * it uses `allow-scripts` (an opaque origin that cannot be measured), so it fills the host.
 */
export class Frame extends LitElement {
  /** URL to load (used when `srcdoc` is empty). */
  @property({ type: String }) src = "";
  /** Inline HTML document to load (takes precedence over `src`). */
  @property({ type: String }) srcdoc = "";
  /** Opt trusted scripts into the sandbox (off by default). Disables auto-height. */
  @property({ type: Boolean, attribute: "allow-scripts" }) allowScripts = false;
  /** Auto-size to the content height (default). When false, fill the host (`height: 100%`). */
  @property({ type: Boolean }) fit = true;

  createRenderRoot(): this {
    return this;
  }

  connectedCallback(): void {
    super.connectedCallback();
    window.addEventListener("resize", this.remeasure);
  }

  disconnectedCallback(): void {
    window.removeEventListener("resize", this.remeasure);
    super.disconnectedCallback();
  }

  render(): TemplateResult {
    return html`<iframe class="mcp-sandbox w-100 border-0"></iframe>`;
  }

  updated(): void {
    this.apply();
  }

  /** Whether the loaded document can be measured for auto-height (same-origin + script-free). */
  private get measurable(): boolean {
    return this.fit && !this.allowScripts;
  }

  private get iframe(): HTMLIFrameElement | null {
    return this.querySelector("iframe");
  }

  /** (Re)configure the iframe for the current properties and (re)load its content. */
  private apply(): void {
    const frame = this.iframe;
    if (!frame) {
      return;
    }
    frame.setAttribute("sandbox", this.allowScripts ? "allow-scripts" : "allow-same-origin");
    frame.style.cssText = this.measurable
      ? "width:100%;border:0;opacity:0;max-height:0;transition:opacity 0.15s"
      : "width:100%;border:0;height:100%";
    frame.onload = (): void => this.reveal(frame);
    if (this.srcdoc) {
      frame.removeAttribute("src");
      frame.srcdoc = this.srcdoc;
    } else if (this.src) {
      frame.removeAttribute("srcdoc");
      frame.src = this.src;
    } else {
      frame.removeAttribute("srcdoc");
      frame.removeAttribute("src");
    }
  }

  /** Size the (loaded) iframe to its content and make it visible. */
  private reveal(frame: HTMLIFrameElement): void {
    if (this.measurable) {
      try {
        const height = frame.contentDocument?.documentElement.scrollHeight ?? 0;
        frame.style.maxHeight = "none";
        frame.style.height = `${height}px`;
      } catch {
        // Cross-origin document: cannot measure -- fall back to filling the host.
        frame.style.maxHeight = "none";
        frame.style.height = "100%";
      }
    }
    frame.style.opacity = "1";
  }

  /** Re-measure on viewport resize (content height changes with width). */
  private readonly remeasure = (): void => {
    const frame = this.iframe;
    if (frame && this.measurable) {
      this.reveal(frame);
    }
  };
}

customElements.define("mcp-frame", Frame);
