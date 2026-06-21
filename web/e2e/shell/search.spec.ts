// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

import { test, expect } from "@playwright/test";
import { openView } from "./helpers";

// The built-in Search view: a debounced query posts to /api/search; selecting a hit opens its
// `content` ref. The demo's hits carry an `entity` content ref, so a selection opens the typed
// detail view -- the search -> entity path real consumers (BMF / gesetze) depend on.
test.describe("Search", () => {
  test("a query lists hits and selecting one opens the entity", async ({ page }) => {
    await openView(page, /Search/);

    await page.getByRole("searchbox", { name: "Search" }).fill("beta");

    const hit = page.locator(".list-group-item").filter({ hasText: "Beta record" });
    await expect(hit).toBeVisible();
    await hit.click();

    // The hit's content ref ({ type: entity, entityType: record, id: beta }) opens the detail.
    await expect(
      page.locator(".mcp-entity").getByRole("heading", { name: "Beta record" }),
    ).toBeVisible();
  });
});
