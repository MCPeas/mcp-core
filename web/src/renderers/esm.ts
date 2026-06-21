// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

import { registerRenderer } from "../shell/registry.js";
import type { ContentRef, EsmRef } from "../shell/types.js";

/** Whether `spec` resolves to the shell's own origin. An `esm` module runs in the main
 *  document, so we refuse to import a cross-origin URL. */
function isSameOriginModule(spec: string): boolean {
  try {
    return new URL(spec, location.href).origin === location.origin;
  } catch {
    return false;
  }
}

registerRenderer("esm", {
  async render(ref: ContentRef, host: HTMLElement): Promise<void> {
    const r = ref as EsmRef;
    if (!isSameOriginModule(r.module)) {
      host.replaceChildren(
        Object.assign(document.createElement("div"), {
          className: "alert alert-danger m-3",
          textContent: `Refusing to load cross-origin module: ${r.module}`,
        }),
      );
      return;
    }
    await import(r.module);
    const el = document.createElement(r.element) as HTMLElement & Record<string, unknown>;
    if (r.props) {
      Object.assign(el, r.props);
    }
    if (r.selection !== undefined) {
      el.selection = r.selection;
    }
    host.replaceChildren(el);
  },
});
