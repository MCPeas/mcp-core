// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

import { type Locator, type Page } from "@playwright/test";

/** The Operations card for a given tool name. */
export function toolCard(page: Page, name: string): Locator {
  return page
    .locator(".card")
    .filter({ has: page.getByRole("heading", { name, exact: true }) });
}

/** Open the shell (served at /ui/) and click the named sidebar view. */
export async function openView(page: Page, name: RegExp): Promise<void> {
  await page.goto("/ui/");
  await page.getByRole("link", { name }).click();
}
