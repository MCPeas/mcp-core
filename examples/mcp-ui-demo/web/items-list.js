import { html } from "lit";
import { ListElement } from "mcp-ui";

// Each item carries its own ContentRef. The `record` items have no registered renderer,
// so the shell falls back to <pre>JSON.stringify(...)</pre> ("see the data"); the last
// item uses a custom `esm` override component instead.
const ITEMS = [
  {
    id: "alpha",
    label: "Record: Alpha (JSON fallback)",
    content: { type: "record", data: { id: "alpha", title: "Alpha", value: 42, tags: ["x", "y"] } },
  },
  {
    id: "beta",
    label: "Record: Beta (JSON fallback)",
    content: {
      type: "record",
      data: { id: "beta", title: "Beta", nested: { ok: true, items: [1, 2, 3] } },
    },
  },
  {
    id: "echo",
    label: "Echo (custom override)",
    content: {
      type: "esm",
      module: "/app/echo-detail.js",
      element: "demo-echo-detail",
      selection: { message: "hello from the override component" },
    },
  },
];

class DemoItems extends ListElement {
  render() {
    return html`
      <div class="list-group list-group-flush">
        ${ITEMS.map(
          (item) => html`
            <button
              type="button"
              class="list-group-item list-group-item-action ${item.id === this.selectedId
                ? "active"
                : ""}"
              @click=${() => this.select(item.id, item.content)}
            >
              ${item.label}
            </button>
          `,
        )}
      </div>
    `;
  }
}

customElements.define("demo-items", DemoItems);
