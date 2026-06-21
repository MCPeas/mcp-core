// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

import { test, expect } from "@playwright/test";
import { openView, toolCard } from "./helpers";

// The Operations console: MCP tools/list -> a schema-driven form -> tools/call over /mcp.
test.describe("Operations console", () => {
  test("invokes a tool through its schema-driven form", async ({ page }) => {
    await openView(page, /Operations/);

    const add = toolCard(page, "add");
    const numbers = add.locator('input[type="number"]');
    await numbers.nth(0).fill("2");
    await numbers.nth(1).fill("3");
    await add.getByRole("button", { name: "Run" }).click();

    // add(2, 3) -> 5, rendered in the result panel.
    await expect(add.locator(".mcp-json")).toContainText("5");
  });

  test("blocks an empty required submit with native validation (no call)", async ({ page }) => {
    await openView(page, /Operations/);

    const add = toolCard(page, "add");
    await add.getByRole("button", { name: "Run" }).click();

    // The required field is invalid and nothing was invoked, so no result is shown.
    await expect(add.locator('input[type="number"]').first()).toHaveJSProperty(
      "validity.valid",
      false,
    );
    await expect(add.locator(".mcp-json")).toHaveCount(0);
  });
});
