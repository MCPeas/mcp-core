// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

import { LitElement, html, type TemplateResult } from "lit";
import { property, state } from "lit/decorators.js";
import type { JsonSchema } from "./mcp-client.js";

/**
 * Generic input form driven by a JSON Schema (an object schema's `properties` + `required`) --
 * the single source of truth for a tool's arguments. Renders one control per property
 * (boolean -> checkbox, number/integer -> number, enum -> select, string -> text), tracks the
 * collected values, and dispatches a bubbling `mcp-form-change` CustomEvent with `{ values }`
 * on every edit. A reusable building block: `mcp-tool` pairs it with a Run button, but any
 * consumer can drop `<mcp-schema-form .schema=${...}>` into its own UI and read `.values`.
 */
export class SchemaForm extends LitElement {
  @property({ attribute: false }) schema: JsonSchema | null = null;
  /** The collected arguments, kept in sync with the inputs. */
  @state() values: Record<string, unknown> = {};

  createRenderRoot(): this {
    return this;
  }

  willUpdate(changed: Map<string, unknown>): void {
    // Reset to the schema's defaults whenever the schema changes.
    if (changed.has("schema")) {
      this.values = defaults(this.schema);
    }
  }

  private get props(): [string, JsonSchema][] {
    return Object.entries(this.schema?.properties ?? {});
  }

  private set(name: string, value: unknown): void {
    // Drop empty optional strings so the tool receives its own default instead of "".
    const next = { ...this.values };
    if (value === "" && !this.required.has(name)) {
      delete next[name];
    } else {
      next[name] = value;
    }
    this.values = next;
    this.dispatchEvent(
      new CustomEvent("mcp-form-change", {
        detail: { values: this.values },
        bubbles: true,
        composed: true,
      }),
    );
  }

  private get required(): Set<string> {
    return new Set(this.schema?.required ?? []);
  }

  render(): TemplateResult {
    const props = this.props;
    if (props.length === 0) {
      return html`<div class="small text-muted">No input parameters.</div>`;
    }
    return html`<div class="mcp-schema-form vstack gap-2">
      ${props.map(([name, def]) => this.field(name, def))}
    </div>`;
  }

  private field(name: string, def: JsonSchema): TemplateResult {
    const required = this.required.has(name);
    const label = html`<label class="form-label mb-1 small fw-medium" for="f-${name}">
      <code>${name}</code>${required ? html`<span class="text-danger"> *</span>` : ""}
      ${def.description
        ? html`<span class="text-muted fw-normal"> - ${def.description}</span>`
        : ""}
    </label>`;
    return html`<div>${label}${this.control(name, def, required)}</div>`;
  }

  private control(name: string, def: JsonSchema, required: boolean): TemplateResult {
    const current = this.values[name];
    if (Array.isArray(def.enum)) {
      return html`<select
        id="f-${name}"
        class="form-select form-select-sm"
        ?required=${required}
        @change=${(e: Event) => this.set(name, (e.target as HTMLSelectElement).value)}
      >
        ${required ? "" : html`<option value=""></option>`}
        ${def.enum.map((v) => {
          const value = String(v);
          return html`<option value=${value} ?selected=${current === v}>${value}</option>`;
        })}
      </select>`;
    }
    if (def.type === "boolean") {
      return html`<div class="form-check">
        <input
          id="f-${name}"
          type="checkbox"
          class="form-check-input"
          .checked=${current === true}
          @change=${(e: Event) => this.set(name, (e.target as HTMLInputElement).checked)}
        />
      </div>`;
    }
    if (def.type === "number" || def.type === "integer") {
      return html`<input
        id="f-${name}"
        type="number"
        class="form-control form-control-sm"
        step=${def.type === "integer" ? "1" : "any"}
        ?required=${required}
        .value=${toInputValue(current)}
        @input=${(e: Event) => {
          const raw = (e.target as HTMLInputElement).value;
          this.set(name, raw === "" ? "" : Number(raw));
        }}
      />`;
    }
    return html`<input
      id="f-${name}"
      type="text"
      class="form-control form-control-sm"
      ?required=${required}
      .value=${toInputValue(current)}
      @input=${(e: Event) => this.set(name, (e.target as HTMLInputElement).value)}
    />`;
  }
}

/** Stringify a stored value for an input's `.value` (primitives only; never "[object Object]"). */
function toInputValue(value: unknown): string {
  if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return "";
}

/** The default value object for a schema (properties whose schema declares a `default`). */
function defaults(schema: JsonSchema | null): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const [name, def] of Object.entries(schema?.properties ?? {})) {
    if (def.default !== undefined) {
      out[name] = def.default;
    }
  }
  return out;
}

customElements.define("mcp-schema-form", SchemaForm);
