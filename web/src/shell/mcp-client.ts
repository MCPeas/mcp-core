// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

// A tiny, dependency-free MCP client over the Streamable HTTP transport, for the browser.
//
// The shell talks MCP directly to the server it was served from (the same `/mcp` endpoint a
// `mcp_core::streamable_http_router` mounts), so the tool schemas (`tools/list`) and their
// invocation (`tools/call`) are the single source of truth -- no parallel REST API. This is
// the engine behind the Operations console (mcp-tool) and catalog item actions.
//
// Transport notes (verified against rmcp 1.7 Streamable HTTP): `initialize` returns the
// session id in the `MCP-Session-Id` response header, which every later request must echo;
// responses are content-negotiated and arrive as `text/event-stream` (one JSON-RPC message
// per `data:` frame) or plain JSON. Same-origin, so the custom response header is readable.

/** JSON Schema for a tool's input (the `properties`/`required` subset the form needs). */
export interface JsonSchema {
  type?: string;
  properties?: Record<string, JsonSchema>;
  required?: string[];
  description?: string;
  enum?: unknown[];
  default?: unknown;
  items?: JsonSchema;
  [key: string]: unknown;
}

/** Behavioral hints a server may attach to a tool (all optional, advisory). */
export interface ToolAnnotations {
  title?: string;
  readOnlyHint?: boolean;
  destructiveHint?: boolean;
  idempotentHint?: boolean;
  openWorldHint?: boolean;
}

/** A tool as advertised by `tools/list`. */
export interface ToolDef {
  name: string;
  description?: string;
  /** JSON Schema for the tool's arguments. */
  inputSchema?: JsonSchema;
  annotations?: ToolAnnotations;
}

/** One block of a tool result's `content` (text is the common case). */
export interface ContentBlock {
  type: string;
  text?: string;
  [key: string]: unknown;
}

/** The outcome of a `tools/call`. `isError` is the MCP tool-level error flag (not a transport error). */
export interface ToolResult {
  content: ContentBlock[];
  structuredContent?: unknown;
  isError: boolean;
}

export interface McpClientOptions {
  /** Bearer token for servers started with `--auth-token` (sent as `Authorization: Bearer ...`). */
  authToken?: string;
}

interface JsonRpcMessage {
  jsonrpc: "2.0";
  id?: number | string | null;
  result?: unknown;
  error?: { code: number; message: string; data?: unknown };
}

const PROTOCOL_VERSION = "2025-06-18";

/**
 * A connected MCP session over Streamable HTTP. Construct with the endpoint URL (e.g. `/mcp`),
 * then call {@link listTools} / {@link callTool} -- the first call connects lazily (a single
 * shared `initialize` handshake).
 */
export class McpClient {
  private readonly endpoint: string;
  private readonly authToken?: string;
  private sessionId: string | null = null;
  private negotiatedVersion = PROTOCOL_VERSION;
  private nextId = 1;
  /** Cached handshake so concurrent first calls share one `initialize`. */
  private connecting: Promise<void> | null = null;

  constructor(endpoint: string, options: McpClientOptions = {}) {
    this.endpoint = endpoint;
    this.authToken = options.authToken;
  }

  /** Initialize the session (idempotent): negotiate the protocol, capture the session id, and
   * send the `initialized` notification. Throws on a transport or JSON-RPC error. */
  connect(): Promise<void> {
    if (!this.connecting) {
      this.connecting = this.handshake().catch((err: unknown) => {
        // Let a later call retry a failed handshake rather than caching the rejection forever.
        this.connecting = null;
        throw err;
      });
    }
    return this.connecting;
  }

  /** The server's tools, from `tools/list`. */
  async listTools(): Promise<ToolDef[]> {
    await this.connect();
    const result = await this.request("tools/list", {});
    const tools = (result as { tools?: ToolDef[] } | undefined)?.tools;
    return tools ?? [];
  }

  /** Invoke a tool by name. Returns its `content`/`structuredContent` and the `isError` flag;
   * throws only on a transport/JSON-RPC failure (a tool that reports failure sets `isError`). */
  async callTool(name: string, args: Record<string, unknown>): Promise<ToolResult> {
    await this.connect();
    const result = (await this.request("tools/call", { name, arguments: args })) as {
      content?: ContentBlock[];
      structuredContent?: unknown;
      isError?: boolean;
    };
    return {
      content: result.content ?? [],
      structuredContent: result.structuredContent,
      isError: result.isError ?? false,
    };
  }

  private async handshake(): Promise<void> {
    const res = await this.post({
      jsonrpc: "2.0",
      id: this.nextId++,
      method: "initialize",
      params: {
        protocolVersion: PROTOCOL_VERSION,
        capabilities: {},
        clientInfo: { name: "mcp-ui", version: "1" },
      },
    });
    this.sessionId = res.headers.get("MCP-Session-Id");
    const message = await this.readMessage(res);
    const result = this.unwrap(message) as { protocolVersion?: string } | undefined;
    if (result?.protocolVersion) {
      this.negotiatedVersion = result.protocolVersion;
    }
    await this.post({ jsonrpc: "2.0", method: "notifications/initialized" });
  }

  /** Send an id'd request and return its `result`, throwing on a JSON-RPC `error`. */
  private async request(method: string, params: unknown): Promise<unknown> {
    const res = await this.post({ jsonrpc: "2.0", id: this.nextId++, method, params });
    const message = await this.readMessage(res);
    return this.unwrap(message);
  }

  private async post(body: unknown): Promise<Response> {
    const headers: Record<string, string> = {
      "content-type": "application/json",
      accept: "application/json, text/event-stream",
    };
    if (this.sessionId) {
      headers["MCP-Session-Id"] = this.sessionId;
      headers["MCP-Protocol-Version"] = this.negotiatedVersion;
    }
    if (this.authToken) {
      headers["Authorization"] = `Bearer ${this.authToken}`;
    }
    const res = await fetch(this.endpoint, {
      method: "POST",
      headers,
      body: JSON.stringify(body),
    });
    if (!res.ok) {
      throw new Error(`MCP ${res.status} from ${this.endpoint}`);
    }
    return res;
  }

  /** Read the single JSON-RPC message from a response (JSON or `text/event-stream`). Returns
   * null for an empty body (e.g. the 202 to a notification). */
  private async readMessage(res: Response): Promise<JsonRpcMessage | null> {
    const text = (await res.text()).trim();
    if (!text) {
      return null;
    }
    const type = res.headers.get("content-type") ?? "";
    const payloads = type.includes("text/event-stream") ? eventStreamData(text) : [text];
    // The response to a request carries exactly one result/error message; take the last
    // non-empty data frame that parses to a JSON-RPC message.
    let last: JsonRpcMessage | null = null;
    for (const payload of payloads) {
      const parsed = JSON.parse(payload) as JsonRpcMessage;
      if (parsed.result !== undefined || parsed.error !== undefined) {
        last = parsed;
      }
    }
    return last;
  }

  /** Extract a request's `result`, throwing on a JSON-RPC `error` or a missing response. */
  private unwrap(message: JsonRpcMessage | null): unknown {
    if (!message) {
      throw new Error("MCP: empty response");
    }
    if (message.error) {
      throw new Error(`MCP error ${message.error.code}: ${message.error.message}`);
    }
    return message.result;
  }
}

/** Pull the `data:` payloads out of an SSE response body, one per event. */
function eventStreamData(body: string): string[] {
  const out: string[] = [];
  for (const line of body.split("\n")) {
    if (line.startsWith("data:")) {
      const payload = line.slice(5).trimStart();
      if (payload) {
        out.push(payload);
      }
    }
  }
  return out;
}
