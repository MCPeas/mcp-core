// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

// Field-renderer library: render one attribute value by its kind. mcp-core ships defaults for
// the common kinds; a consumer overrides/adds via registerFieldRenderer. A renderer returns a
// Lit template, so it can build markup inline OR delegate to a custom element -- the "Lit
// component or bare render method" of the design.

import { html, type TemplateResult } from "lit";
import { navigate } from "./navigate.js";
import "./markdown.js";
import type { PropSchema, ResourceRef } from "./catalog-types.js";

/** Context for a field renderer (`source` is the element to navigate from, for ref kinds). */
export interface FieldCtx {
  name: string;
  source: HTMLElement;
}

export type FieldRenderer = (value: unknown, schema: PropSchema, ctx: FieldCtx) => TemplateResult;

const renderers = new Map<string, FieldRenderer>();

/** Register or override the renderer for a field kind (an `x-mcp-kind`, or a JSON Schema
 *  `format`/`type`). */
export function registerFieldRenderer(kind: string, renderer: FieldRenderer): void {
  renderers.set(kind, renderer);
}

/** The render kind for a property: explicit `x-mcp-kind`, else a `format`/`type` mapping. */
export function fieldKind(schema: PropSchema): string {
  if (typeof schema["x-mcp-kind"] === "string") {
    return schema["x-mcp-kind"];
  }
  if (Array.isArray(schema.enum)) {
    return "enum";
  }
  if (schema.format === "date" || schema.format === "date-time") {
    return "date";
  }
  if (schema.type === "integer" || schema.type === "number") {
    return "number";
  }
  if (schema.type === "boolean") {
    return "bool";
  }
  if (schema.type === "array") {
    return "list";
  }
  if (schema.type === "object") {
    return "json";
  }
  return "text";
}

/** How the entity view should place a field of this kind: a compact metadata row, the flowing
 *  body under the title, or a headlined content section (a table, a list of examples, ...). */
export function fieldLayout(kind: string): "metadata" | "body" | "section" {
  switch (kind) {
    case "markdown":
      return "body";
    case "table":
    case "sections":
    case "code":
    case "json":
      return "section";
    default:
      return "metadata";
  }
}

/** Render an attribute value using the registered (or built-in) renderer for its kind. */
export function renderField(value: unknown, schema: PropSchema, ctx: FieldCtx): TemplateResult {
  const renderer = renderers.get(fieldKind(schema)) ?? renderers.get("text");
  return renderer ? renderer(value, schema, ctx) : html`<span>${stringify(value)}</span>`;
}

function stringify(value: unknown): string {
  if (value === null || value === undefined) {
    return "";
  }
  if (typeof value === "string") {
    return value;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  if (typeof value === "object") {
    return JSON.stringify(value);
  }
  return ""; // symbol / function / bigint are not valid catalog field data
}

// --- built-in field renderers ---

registerFieldRenderer("text", (v) => html`<span>${stringify(v)}</span>`);
// Rendered as real markdown in a sandboxed iframe when the `web-markdown` feature is built in,
// else preformatted text (see <mcp-markdown>).
registerFieldRenderer("markdown", (v) => html`<mcp-markdown .text=${stringify(v)}></mcp-markdown>`);
registerFieldRenderer("number", (v) => html`<span>${stringify(v)}</span>`);
registerFieldRenderer("date", (v) => html`<span>${stringify(v)}</span>`);
registerFieldRenderer("bool", (v) => html`<span>${v ? "yes" : "no"}</span>`);
registerFieldRenderer(
  "enum",
  (v) => html`<span class="badge text-bg-secondary">${stringify(v)}</span>`,
);
registerFieldRenderer(
  "code",
  (v) => html`<pre class="mcp-json p-2 m-0 small border rounded">${stringify(v)}</pre>`,
);
registerFieldRenderer(
  "json",
  (v) =>
    html`<pre class="mcp-json p-2 m-0 small border rounded">${JSON.stringify(v, null, 2)}</pre>`,
);
registerFieldRenderer("list", (v) =>
  Array.isArray(v)
    ? html`<ul class="mb-0 ps-3">
        ${v.map((item) => html`<li>${stringify(item)}</li>`)}
      </ul>`
    : html`<span>${stringify(v)}</span>`,
);
registerFieldRenderer("table", (v) => renderTable(v));
registerFieldRenderer("ref", (v, _schema, ctx) => renderRef(v, ctx));
// A list of titled text blocks (e.g. a paragraph's Beispiele / Hinweise).
registerFieldRenderer("sections", (v) =>
  Array.isArray(v)
    ? html`<div class="vstack gap-3">
        ${v.map((s) => {
          const sec = (s ?? {}) as { title?: unknown; body?: unknown };
          const title = stringify(sec.title);
          return html`<div>
            ${title ? html`<div class="fw-semibold mb-1">${title}</div>` : ""}
            <div style="white-space: pre-wrap">${stringify(sec.body)}</div>
          </div>`;
        })}
      </div>`
    : html`<span>${stringify(v)}</span>`,
);

/** Whether `x` looks like a single table object (`{ headers?, rows, caption? }`). */
function isTableObject(x: unknown): boolean {
  return !!x && typeof x === "object" && !Array.isArray(x) && ("rows" in x || "headers" in x);
}

/** Render a single table, an array of tables, or a 2-D array as Bootstrap table(s). */
function renderTable(value: unknown): TemplateResult {
  // An array of table objects (a document with several tables) -> render each.
  if (Array.isArray(value) && value.length > 0 && isTableObject(value[0])) {
    return html`<div class="vstack gap-3">
      ${(value as unknown[]).map((t) => renderOneTable(t))}
    </div>`;
  }
  return renderOneTable(value);
}

/** Render `{ caption?, headers?, rows }` or a 2-D array as a Bootstrap table. Cells are stringified. */
function renderOneTable(value: unknown): TemplateResult {
  const obj = (value ?? {}) as { caption?: unknown; headers?: unknown; rows?: unknown };
  const rows: unknown[][] = Array.isArray(value)
    ? (value as unknown[][])
    : Array.isArray(obj.rows)
      ? (obj.rows as unknown[][])
      : [];
  const headerRaw = Array.isArray(obj.headers) ? (obj.headers as unknown[]) : [];
  const headers: unknown[] = Array.isArray(headerRaw[0])
    ? (headerRaw as unknown[][]).flat()
    : headerRaw;
  if (rows.length === 0 && headers.length === 0) {
    return html`<span class="text-muted small">(empty table)</span>`;
  }
  const caption = typeof obj.caption === "string" ? obj.caption : "";
  return html`<div class="table-responsive">
    ${caption ? html`<div class="small fw-semibold mb-1">${caption}</div>` : ""}
    <table class="table table-sm table-bordered mb-0 small">
      ${headers.length
        ? html`<thead>
            <tr>
              ${headers.map((h) => html`<th>${stringify(h)}</th>`)}
            </tr>
          </thead>`
        : ""}
      <tbody>
        ${rows.map(
          (row) =>
            html`<tr>
              ${(Array.isArray(row) ? row : [row]).map((cell) => html`<td>${stringify(cell)}</td>`)}
            </tr>`,
        )}
      </tbody>
    </table>
  </div>`;
}

/** Render a value that is itself a resource reference as a navigable link. */
function renderRef(value: unknown, ctx: FieldCtx): TemplateResult {
  const ref = value as ResourceRef | null;
  if (!ref || typeof ref !== "object" || typeof ref.id !== "string") {
    return html`<span></span>`;
  }
  return html`<button
    type="button"
    class="btn btn-sm btn-link p-0"
    @click=${() => navigate(ctx.source, { type: "entity", entityType: ref.type, id: ref.id })}
  >
    ${ref.title ?? ref.id}
  </button>`;
}
