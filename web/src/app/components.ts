// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

// Default manifest, baked into the shell dist and served at `/app/components.js`. A server
// with no consumer frontend loads this empty manifest, so the shell shows only its built-in
// views (Operations, plus Catalog when the server has one) with no failed manifest request.
// Consumers override it by serving their own `/app/components.js` (e.g. a `ServeDir` mounted
// at `/app`), which shadows this default.
import type { Manifest } from "../shell/types.js";

const manifest: Manifest = { views: [] };
export default manifest;
