// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

import { html, type TemplateResult, type PropertyValues } from "lit";
import { property, state } from "lit/decorators.js";
import { ListElement } from "./list-element.js";
import { McpClient, type ToolResult } from "./mcp-client.js";
import { entityType } from "./catalog-schema.js";
import { apiBase, mcpEndpoint } from "./endpoints.js";
import { emptyNote, errorAlert, loadingNote, toolResultText, workingNote } from "./ui.js";
import type {
  EntityAction,
  EntityType,
  FilterToggle,
  JsonApiList,
  PropSchema,
  ResourceRef,
} from "./catalog-types.js";

/** Context handed to a custom list-toolbar renderer (the consumer extension point). */
export interface ListToolbarContext {
  entityType: string;
  /** Whether the list is in edit mode (the gear is toggled on). */
  editing: boolean;
  /** Reload the current list. */
  reload: () => void;
  /** Call an MCP tool over `/mcp` (the same transport the actions use). */
  callTool: (tool: string, args?: Record<string, unknown>) => Promise<ToolResult>;
}

/** A custom toolbar renderer: extra controls shown next to the search/filter input. */
export type ListToolbarRenderer = (ctx: ListToolbarContext) => TemplateResult;

const listToolbars = new Map<string, ListToolbarRenderer>();

/** Hook extra controls into a type's list toolbar (a settings menu, a custom action, ...).
 *  Light DOM, so this registry replaces a native `<slot>`. */
export function registerListToolbar(type: string, renderer: ListToolbarRenderer): void {
  listToolbars.set(type, renderer);
}

/**
 * Faceted, typed list for one entity type (set `entity-type`): a free-text box plus facet
 * controls built from the schema drive `{api}/catalog/items/{type}`; selecting an item
 * navigates to its entity view. A gear toggles an edit mode that reveals per-row actions
 * (`item_actions`), list-level actions (`list_actions`) and filter toggles (`list_toggles`)
 * declared on the type -- each action invokes an MCP tool over `/mcp`. Default view stays
 * read-only and content-focused. Schema-driven -- no per-server frontend code.
 */
export class EntityList extends ListElement {
  @property({ type: String, attribute: "entity-type" }) entityType = "";
  @state() items: ResourceRef[] = [];
  @state() total: number | null = null;
  @state() type: EntityType | null = null;
  @state() q = "";
  @state() error = "";
  @state() loading = false;
  /** Whether the edit toolbar / per-row controls are revealed (gear on). */
  @state() editing = false;
  /** Id of the row whose action is running (disables buttons), else "". */
  @state() acting = "";
  /** Outcome of the last action, shown inline. */
  @state() feedback: { id: string; message: string; isError: boolean } | null = null;
  private filter: Record<string, string> = {};
  private limit = 50;
  private timer = 0;
  private readonly client = new McpClient(mcpEndpoint());

  connectedCallback(): void {
    super.connectedCallback();
    void this.load();
  }

  willUpdate(changed: PropertyValues): void {
    if (changed.has("entityType")) {
      this.filter = {};
      this.q = "";
      this.limit = 50;
      this.type = null;
      this.editing = false;
      this.feedback = null;
      void this.load();
    }
  }

  private async load(): Promise<void> {
    if (!this.entityType) {
      return;
    }
    this.loading = true;
    this.error = "";
    try {
      this.type ??= (await entityType(this.entityType)) ?? null;
      const params = new URLSearchParams();
      if (this.q.trim()) {
        params.set("q", this.q.trim());
      }
      for (const [key, value] of Object.entries(this.filter)) {
        if (value) {
          params.set(key, value);
        }
      }
      params.set("limit", String(this.limit));
      const res = await fetch(`${apiBase()}/catalog/items/${this.entityType}?${params.toString()}`);
      if (!res.ok) {
        throw new Error(`HTTP ${res.status}`);
      }
      const body = (await res.json()) as JsonApiList<ResourceRef>;
      this.items = body.data;
      this.total = body.meta.total ?? null;
    } catch (e) {
      this.error = String(e);
    } finally {
      this.loading = false;
    }
  }

  private debouncedLoad(): void {
    clearTimeout(this.timer);
    this.timer = window.setTimeout(() => void this.load(), 250);
  }

  private open(ref: ResourceRef): void {
    this.select(ref.id, { type: "entity", entityType: this.entityType, id: ref.id });
  }

  private toggleFilter(toggle: FilterToggle): void {
    const next = { ...this.filter };
    if (next[toggle.key] === toggle.value) {
      delete next[toggle.key];
    } else {
      next[toggle.key] = toggle.value;
    }
    this.filter = next;
    this.limit = 50;
    void this.load();
  }

  /** Run an action: call its MCP tool (a row passes its id), then refresh. */
  private async runAction(action: EntityAction, ref?: ResourceRef): Promise<void> {
    const subject = ref ? `${action.label}: ${ref.title ?? ref.id}?` : `${action.label}?`;
    if (action.danger && !window.confirm(subject)) {
      return;
    }
    const key = ref ? ref.id : `list:${action.id}`;
    this.acting = key;
    this.feedback = null;
    try {
      const args = ref ? { [action.arg ?? "id"]: ref.id } : {};
      const result = await this.client.callTool(action.tool, args);
      this.feedback = {
        id: key,
        message: toolResultText(result) ?? "Done.",
        isError: result.isError,
      };
      await this.load();
    } catch (e) {
      this.feedback = { id: key, message: String(e), isError: true };
    } finally {
      this.acting = "";
    }
  }

  private facetControl(name: string): TemplateResult {
    const schema: PropSchema | undefined = this.type?.attributes.properties?.[name];
    const label = schema?.title ?? name;
    const value = this.filter[name] ?? "";
    if (schema && Array.isArray(schema.enum)) {
      return html`<select
        class="form-select form-select-sm"
        aria-label=${label}
        @change=${(e: Event) => {
          this.filter = { ...this.filter, [name]: (e.target as HTMLSelectElement).value };
          void this.load();
        }}
      >
        <option value="">${label}: all</option>
        ${schema.enum.map((opt) => {
          const s = String(opt);
          return html`<option value=${s} ?selected=${value === s}>${s}</option>`;
        })}
      </select>`;
    }
    return html`<input
      type="search"
      class="form-control form-control-sm"
      placeholder=${label}
      .value=${value}
      @input=${(e: Event) => {
        this.filter = { ...this.filter, [name]: (e.target as HTMLInputElement).value };
        this.debouncedLoad();
      }}
    />`;
  }

  /** Whether the type declares anything to edit (so the gear is worth showing). */
  private get hasEditControls(): boolean {
    const t = this.type;
    return !!(t?.item_actions?.length || t?.list_actions?.length || t?.list_toggles?.length);
  }

  render(): TemplateResult {
    const facets = this.type?.facets ?? [];
    const custom = listToolbars.get(this.entityType);
    const ctx: ListToolbarContext = {
      entityType: this.entityType,
      editing: this.editing,
      reload: () => void this.load(),
      callTool: (tool, args) => this.client.callTool(tool, args ?? {}),
    };
    return html`
      <div class="p-2 border-bottom bg-body vstack gap-2">
        <div class="d-flex gap-2 align-items-center">
          <input
            type="search"
            class="form-control form-control-sm flex-grow-1"
            placeholder="Search..."
            .value=${this.q}
            @input=${(e: Event) => {
              this.q = (e.target as HTMLInputElement).value;
              this.debouncedLoad();
            }}
          />
          ${custom ? custom(ctx) : ""}
          ${this.hasEditControls
            ? html`<button
                type="button"
                class="btn btn-sm btn-outline-secondary ${this.editing ? "active" : ""}"
                title="Bearbeiten"
                aria-pressed=${this.editing ? "true" : "false"}
                @click=${() => {
                  this.editing = !this.editing;
                }}
              >
                ⚙
              </button>`
            : ""}
        </div>
        ${facets.map((facet) => this.facetControl(facet))} ${this.editing ? this.editToolbar() : ""}
      </div>
      ${this.body()}
    `;
  }

  private editToolbar(): TemplateResult | string {
    const toggles = this.type?.list_toggles ?? [];
    const actions = this.type?.list_actions ?? [];
    if (toggles.length === 0 && actions.length === 0) {
      return "";
    }
    return html`<div class="d-flex flex-wrap gap-1">
      ${toggles.map(
        (toggle) =>
          html`<button
            type="button"
            class="btn btn-sm ${this.filter[toggle.key] === toggle.value
              ? "btn-primary"
              : "btn-outline-primary"}"
            @click=${() => this.toggleFilter(toggle)}
          >
            ${toggle.label}
          </button>`,
      )}
      ${actions.map(
        (action) =>
          html`<button
            type="button"
            class="btn btn-sm btn-outline-${action.danger ? "danger" : "secondary"}"
            ?disabled=${this.acting !== ""}
            @click=${() => void this.runAction(action)}
          >
            ${action.icon ? html`${action.icon} ` : ""}${action.label}
          </button>`,
      )}
    </div>`;
  }

  private body(): TemplateResult {
    if (this.error) {
      return errorAlert(this.error);
    }
    if (this.loading && this.items.length === 0) {
      return loadingNote();
    }
    if (this.items.length === 0) {
      return emptyNote("No items.");
    }
    return html`
      ${this.total !== null
        ? html`<div class="px-3 py-1 small text-muted border-bottom">
            ${this.items.length} of ${this.total}
          </div>`
        : ""}
      <div class="list-group list-group-flush">${this.items.map((ref) => this.row(ref))}</div>
      ${this.total !== null && this.items.length < this.total
        ? html`<div class="p-2">
            <button
              type="button"
              class="btn btn-sm btn-outline-secondary w-100"
              @click=${() => {
                this.limit += 50;
                void this.load();
              }}
            >
              Load more
            </button>
          </div>`
        : ""}
    `;
  }

  private row(ref: ResourceRef): TemplateResult {
    const active = ref.id === this.selectedId;
    return html`<div
      class="list-group-item list-group-item-action d-flex flex-column gap-1 ${active
        ? "active"
        : ""}"
      role="button"
      style="cursor: pointer"
      @click=${() => this.open(ref)}
    >
      <div class="d-flex justify-content-between align-items-center gap-2">
        <span class="fw-medium">${ref.title ?? ref.id}</span>
        ${this.editing ? this.rowActions(ref) : ""}
      </div>
      ${this.acting === ref.id ? workingNote() : ""}
      ${this.feedback?.id === ref.id
        ? html`<div class="small ${this.feedback.isError ? "text-danger" : "text-success"}">
            ${this.feedback.message}
          </div>`
        : ""}
    </div>`;
  }

  private rowActions(ref: ResourceRef): TemplateResult | string {
    const all = this.type?.item_actions ?? [];
    const allowed = ref.actions ? all.filter((a) => ref.actions?.includes(a.id)) : all;
    if (allowed.length === 0) {
      return "";
    }
    return html`<span
      class="btn-group btn-group-sm flex-shrink-0"
      @click=${(e: Event) => e.stopPropagation()}
    >
      ${allowed.map(
        (action) =>
          html`<button
            type="button"
            class="btn btn-outline-${action.danger ? "danger" : "secondary"}"
            ?disabled=${this.acting !== ""}
            title=${action.label}
            @click=${() => void this.runAction(action, ref)}
          >
            ${action.icon ?? action.label}
          </button>`,
      )}
    </span>`;
  }
}

customElements.define("mcp-entity-list", EntityList);
