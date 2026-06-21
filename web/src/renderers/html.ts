// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

import { registerRenderer } from "../shell/registry.js";
import "../shell/frame.js";
import type { ContentRef, HtmlRef } from "../shell/types.js";

/** Render an HTML document into a sandboxed `<mcp-frame>`. Fills `host` by default (the content
 *  pane); pass `fit` to auto-size to the content instead. Script-free unless `allowScripts`. */
export function renderHtml(
  host: HTMLElement,
  opts: { url?: string; srcdoc?: string; allowScripts?: boolean; fit?: boolean },
): void {
  const frame = document.createElement("mcp-frame");
  if (opts.srcdoc != null) {
    frame.srcdoc = opts.srcdoc;
  } else if (opts.url) {
    frame.src = opts.url;
  }
  frame.allowScripts = opts.allowScripts === true;
  frame.fit = opts.fit ?? false;
  host.replaceChildren(frame);
}

registerRenderer("html", {
  render(ref: ContentRef, host: HTMLElement): void {
    const r = ref as HtmlRef;
    renderHtml(host, {
      url: r.url,
      srcdoc: r.srcdoc,
      allowScripts: r.allowScripts === true,
    });
  },
});
