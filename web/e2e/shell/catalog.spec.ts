// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

import { test, expect } from "@playwright/test";
import { openView } from "./helpers";

// The typed catalog (DataCatalog): a faceted, schema-driven list of entities, a generic detail
// view (attributes placed by x-mcp-kind, plus navigable relations), and per-item actions that call
// MCP tools. This is the shape real consumers (the BMF / gesetze servers) wire, so the shell's
// typed path stays covered end to end.
test.describe("Typed catalog", () => {
  test("lists entities and a facet narrows them", async ({ page }) => {
    // Anchor the name: the demo also ships a custom "Records" view at /app, so match the
    // built-in typed-catalog link ("Record") exactly.
    await openView(page, /^Record$/);

    const rows = page.locator(".list-group-item");
    await expect(rows.filter({ hasText: "Alpha record" })).toBeVisible();
    await expect(rows.filter({ hasText: "Beta record" })).toBeVisible();
    await expect(rows.filter({ hasText: "Gamma record" })).toBeVisible();

    // The `kind` facet (an enum -> a dropdown built from the schema) narrows to one value.
    await page.getByRole("combobox", { name: "kind" }).selectOption("secondary");
    await expect(rows.filter({ hasText: "Beta record" })).toBeVisible();
    await expect(rows.filter({ hasText: "Alpha record" })).toHaveCount(0);
  });

  test("opens an entity, renders its fields by kind, and follows a relation", async ({ page }) => {
    await openView(page, /^Record$/);
    await page.locator(".list-group-item").filter({ hasText: "Alpha record" }).click();

    const detail = page.locator(".mcp-entity");
    await expect(detail.getByRole("heading", { name: "Alpha record" })).toBeVisible();
    // x-mcp-kind: table -> a Bootstrap table; markdown -> the summary text flows in the body.
    await expect(detail.locator("table")).toContainText("score");
    await expect(detail).toContainText("first");

    // The to-one `related` relation navigates to the linked entity.
    await detail.getByRole("button", { name: "Beta record" }).click();
    await expect(
      page.locator(".mcp-entity").getByRole("heading", { name: "Beta record" }),
    ).toBeVisible();
  });

  test("an item action invokes its MCP tool and shows the result inline", async ({ page }) => {
    await openView(page, /^Record$/);
    // Per-item actions are revealed by the edit (gear) toggle.
    await page.locator('button[title="Bearbeiten"]').click();

    const alpha = page.locator(".list-group-item").filter({ hasText: "Alpha record" });
    await alpha.getByRole("button", { name: "Echo" }).click();

    // echo(message: "alpha") -> "alpha", shown as inline success feedback on the row.
    await expect(alpha.locator(".text-success")).toContainText("alpha");
  });
});
