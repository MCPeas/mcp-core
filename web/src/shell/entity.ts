// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

import { LitElement, html, type TemplateResult, type PropertyValues } from "lit";
import { property, state } from "lit/decorators.js";
import { registerRenderer } from "./registry.js";
import { loadEntity } from "./catalog-schema.js";
import { renderField, fieldKind, fieldLayout } from "./catalog-fields.js";
import { navigate } from "./navigate.js";
import { errorAlert } from "./ui.js";
import type { ContentRef } from "./types.js";
import type {
  EntityType,
  PropSchema,
  Relationship,
  Resource,
  ResourceRef,
} from "./catalog-types.js";

/** A custom whole-entity renderer for a type: a bare render method (it may build markup or
 *  mount its own custom element). A registered type bypasses the generic renderer. */
export type EntityRenderer = (
  resource: Resource,
  type: EntityType | undefined,
  host: HTMLElement,
) => void;

const entityRenderers = new Map<string, EntityRenderer>();

/** Override the rendering of a whole entity type (else the generic attribute/relation view). */
export function registerEntityRenderer(type: string, renderer: EntityRenderer): void {
  entityRenderers.set(type, renderer);
}

/**
 * Generic, schema-driven entity view: the type's attributes rendered by field kind, plus a
 * Relations section whose refs navigate to the related entity (exploring the graph). Driven
 * entirely by the catalog schema -- no per-server code.
 */
export class Entity extends LitElement {
  @property({ type: String, attribute: "entity-type" }) entityType = "";
  @property({ type: String, attribute: "entity-id" }) entityId = "";
  @state() resource: Resource | null = null;
  @state() type: EntityType | null = null;
  @state() error = "";
  /** Per-relation filter text (relation name -> query). */
  @state() relFilter: Record<string, string> = {};
  /** Collapsed group keys (`${relation} ${group}`); large relations start mostly collapsed. */
  @state() collapsed = new Set<string>();

  createRenderRoot(): this {
    return this;
  }

  willUpdate(changed: PropertyValues): void {
    if (changed.has("entityType") || changed.has("entityId")) {
      void this.load();
    }
  }

  private async load(): Promise<void> {
    this.resource = null;
    this.type = null;
    this.error = "";
    this.relFilter = {};
    this.collapsed = new Set();
    if (!this.entityType || !this.entityId) {
      return;
    }
    try {
      const { type, resource } = await loadEntity(this.entityType, this.entityId);
      this.type = type ?? null;
      this.resource = resource;
      // A multi-section relation (a law's table of contents, say) opens with its first
      // section expanded and the rest collapsed -- scannable, not a wall. (Ungrouped refs
      // have no header and always show, so collapse by the first *labelled* section.)
      const collapsed = new Set<string>();
      for (const rel of type?.relationships ?? []) {
        const groups = orderedGroups(resource.relationships?.[rel.name]?.data ?? []);
        const labelled = groups.filter((g) => g.key !== undefined);
        for (const g of labelled.slice(1)) {
          collapsed.add(groupKey(rel.name, g.key));
        }
      }
      this.collapsed = collapsed;
    } catch (e) {
      this.error = String(e);
    }
  }

  private displayTitle(): string {
    const field = this.type?.title_field;
    const value = field !== undefined ? this.resource?.attributes[field] : undefined;
    if (typeof value === "string") {
      return value;
    }
    if (typeof value === "number") {
      return String(value);
    }
    return this.resource?.id ?? "";
  }

  render(): TemplateResult {
    if (this.error) {
      return errorAlert(this.error);
    }
    if (!this.resource) {
      return html`<div class="p-4 text-muted">Loading...</div>`;
    }
    const props = this.type?.attributes.properties ?? {};
    const attrs = this.resource.attributes;
    // Skip the title (it is the heading) and any attribute with no value, then order by the
    // optional `x-mcp-order` hint (markdown-body-before-block-sections is the default). Scalars
    // become a compact metadata list; markdown + block kinds (tables, example/hint lists) become
    // the ordered content stream, each with an anchor the section-nav can jump to.
    const where = (schema: PropSchema) => fieldLayout(fieldKind(schema));
    const shown = Object.entries(props)
      .filter(([name]) => name !== this.type?.title_field && hasValue(attrs[name]))
      .sort(([, a], [, b]) => fieldOrder(a) - fieldOrder(b));
    const metadata = shown.filter(([, schema]) => where(schema) === "metadata");
    const content = shown.filter(([, schema]) => where(schema) !== "metadata");
    const field = (name: string, schema: PropSchema) =>
      renderField(attrs[name], schema, { name, source: this });
    const anchor = (name: string) => `mcp-sec-${name}`;
    return html`
      <div class="mcp-entity p-3">
        <h2 class="h5 mb-2">${this.displayTitle()}</h2>
        ${metadata.length
          ? html`<dl class="row small text-body-secondary mb-3">
              ${metadata.map(
                ([name, schema]) => html`
                  <dt class="col-sm-3 fw-normal text-truncate">${schema.title ?? name}</dt>
                  <dd class="col-sm-9 mb-1">${field(name, schema)}</dd>
                `,
              )}
            </dl>`
          : ""}
        ${content.length > 1
          ? html`<nav
              class="mcp-section-nav d-flex flex-wrap gap-2 mb-3 py-2 border-bottom sticky-top bg-body"
            >
              ${content.map(
                ([name, schema]) =>
                  html`<button
                    type="button"
                    class="btn btn-sm btn-outline-secondary"
                    @click=${() => this.jumpTo(anchor(name))}
                  >
                    ${schema.title ?? name}
                  </button>`,
              )}
            </nav>`
          : ""}
        ${content.map(
          ([name, schema]) =>
            html`<div id=${anchor(name)} class="mb-3">
              ${where(schema) === "section"
                ? html`<h3 class="h6 text-body-secondary border-bottom pb-1 mb-2">
                    ${schema.title ?? name}
                  </h3>`
                : ""}
              ${field(name, schema)}
            </div>`,
        )}
        ${this.renderRelations()}
      </div>
    `;
  }

  /** Scroll a content section into view -- the section-nav jump links. */
  private jumpTo(id: string): void {
    this.querySelector(`#${CSS.escape(id)}`)?.scrollIntoView({
      behavior: "smooth",
      block: "start",
    });
  }

  private renderRelations(): TemplateResult | string {
    const rels = this.type?.relationships ?? [];
    const data = this.resource?.relationships ?? {};
    const shown = rels.filter((rel) => (data[rel.name]?.data.length ?? 0) > 0);
    if (shown.length === 0) {
      return "";
    }
    return html`<div class="mt-4 border-top pt-3">
      ${shown.map((rel) => this.renderRelation(rel, data[rel.name]?.data ?? []))}
    </div>`;
  }

  /** A relation renders inline (to-one / a few refs) or as a filterable, optionally grouped,
   *  collapsible list (a long table of contents). */
  private renderRelation(rel: Relationship, refs: ResourceRef[]): TemplateResult {
    const grouped = refs.some((r) => r.group !== undefined);
    const listMode = rel.cardinality === "to_many" && (refs.length > 6 || grouped);
    const go = (ref: ResourceRef) =>
      navigate(this, { type: "entity", entityType: rel.target, id: ref.id });

    if (!listMode) {
      return html`<div class="mb-3">
        <div class="small text-muted mb-1">${rel.label}</div>
        <div class="d-flex flex-wrap gap-1">
          ${refs.map(
            (ref) =>
              html`<button
                type="button"
                class="btn btn-sm btn-outline-secondary"
                @click=${() => go(ref)}
              >
                ${ref.title ?? ref.id}
              </button>`,
          )}
        </div>
      </div>`;
    }

    const query = (this.relFilter[rel.name] ?? "").trim().toLowerCase();
    const filtered = query
      ? refs.filter((r) => (r.title ?? r.id).toLowerCase().includes(query))
      : refs;
    const groups = orderedGroups(filtered);
    return html`<div class="mb-3">
      <div class="d-flex align-items-center justify-content-between mb-1">
        <span class="small text-muted">${rel.label}</span>
        <span class="small text-muted">${refs.length}</span>
      </div>
      ${refs.length > 12
        ? html`<input
            type="search"
            class="form-control form-control-sm mb-2"
            placeholder="Filtern..."
            .value=${this.relFilter[rel.name] ?? ""}
            @input=${(e: Event) => {
              this.relFilter = {
                ...this.relFilter,
                [rel.name]: (e.target as HTMLInputElement).value,
              };
            }}
          />`
        : ""}
      <div class="list-group list-group-flush border rounded">
        ${groups.map((g) => this.renderGroup(rel, g, go, query !== ""))}
      </div>
    </div>`;
  }

  private renderGroup(
    rel: Relationship,
    group: RefGroup,
    go: (ref: ResourceRef) => void,
    filtering: boolean,
  ): TemplateResult {
    // While filtering, show every matching group expanded so hits are never hidden.
    const collapsed = !filtering && this.collapsed.has(groupKey(rel.name, group.key));
    const header =
      group.label !== undefined
        ? html`<button
            type="button"
            class="list-group-item list-group-item-action bg-body-tertiary d-flex justify-content-between align-items-center"
            @click=${() => this.toggleGroup(rel.name, group.key)}
          >
            <span class="fw-semibold small">${collapsed ? "▸" : "▾"} ${group.label}</span>
            <span class="badge text-bg-light">${group.items.length}</span>
          </button>`
        : "";
    const rows = collapsed
      ? ""
      : group.items.map(
          (ref) =>
            html`<button
              type="button"
              class="list-group-item list-group-item-action py-1 small"
              @click=${() => go(ref)}
            >
              ${ref.title ?? ref.id}
            </button>`,
        );
    return html`${header}${rows}`;
  }

  private toggleGroup(relName: string, group: string | undefined): void {
    const key = groupKey(relName, group);
    const next = new Set(this.collapsed);
    if (next.has(key)) {
      next.delete(key);
    } else {
      next.add(key);
    }
    this.collapsed = next;
  }
}

/** A relation's refs bucketed by their `group` (first-seen order preserved). Refs with no
 *  group collapse into a single unlabelled bucket. */
interface RefGroup {
  key: string | undefined;
  label: string | undefined;
  items: ResourceRef[];
}

function orderedGroups(refs: ResourceRef[]): RefGroup[] {
  const order: (string | undefined)[] = [];
  const buckets = new Map<string | undefined, ResourceRef[]>();
  for (const ref of refs) {
    if (!buckets.has(ref.group)) {
      buckets.set(ref.group, []);
      order.push(ref.group);
    }
    buckets.get(ref.group)?.push(ref);
  }
  return order.map((key) => ({ key, label: key, items: buckets.get(key) ?? [] }));
}

/** A field's detail-view sort key: the explicit `x-mcp-order` hint if present, else a default that
 *  keeps the markdown body above block sections (preserving the unhinted layout). */
function fieldOrder(schema: PropSchema): number {
  const order = schema["x-mcp-order"];
  if (typeof order === "number") {
    return order;
  }
  return fieldLayout(fieldKind(schema)) === "body" ? 1_000_000 : 2_000_000;
}

/** Whether an attribute value is worth a row (drops absent optionals and empty arrays). */
function hasValue(value: unknown): boolean {
  if (value === null || value === undefined) {
    return false;
  }
  if (Array.isArray(value)) {
    return value.length > 0;
  }
  return true;
}

function groupKey(relName: string, group: string | undefined): string {
  return `${relName} ${group ?? ""}`;
}

customElements.define("mcp-entity", Entity);

// The "entity" ContentRef: delegate to a custom renderer for the type, else the generic element.
registerRenderer("entity", {
  async render(ref: ContentRef, host: HTMLElement): Promise<void> {
    const entityType = (ref as { entityType?: string }).entityType ?? "";
    const id = (ref as { id?: string }).id ?? "";
    const custom = entityRenderers.get(entityType);
    if (custom) {
      try {
        const { type, resource } = await loadEntity(entityType, id);
        custom(resource, type, host);
      } catch (e) {
        host.replaceChildren(note(`Could not load ${entityType}/${id}: ${String(e)}`));
      }
      return;
    }
    const el = document.createElement("mcp-entity") as Entity;
    el.entityType = entityType;
    el.entityId = id;
    host.replaceChildren(el);
  },
});

function note(message: string): HTMLElement {
  const div = document.createElement("div");
  div.className = "alert alert-warning m-3";
  div.textContent = message;
  return div;
}
