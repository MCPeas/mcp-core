// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

import { registerRenderer } from "../shell/registry.js";
import "../shell/markdown.js";
import type { ContentRef, MdRef } from "../shell/types.js";

registerRenderer("md", {
  async render(ref: ContentRef, host: HTMLElement): Promise<void> {
    const r = ref as MdRef;
    let text = r.text ?? "";
    if (!text && r.url) {
      const res = await fetch(r.url);
      text = await res.text();
    }
    // <mcp-markdown> converts + sandboxes when the `web-markdown` feature is built in, and falls
    // back to preformatted text otherwise -- the single markdown code path.
    const el = document.createElement("mcp-markdown");
    el.text = text;
    host.replaceChildren(el);
  },
});
