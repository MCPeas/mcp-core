// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

// Fetches and caches the catalog schema, and loads a single resource. Shared by the entity
// element and the entity-list element.

import { apiBase } from "./endpoints.js";
import type { EntityType, JsonApiResource, Resource } from "./catalog-types.js";

let cache: Promise<Map<string, EntityType>> | null = null;

/** The catalog schema (entity types by name), fetched once from `{api}/catalog/schema`. */
export function entitySchema(): Promise<Map<string, EntityType>> {
  if (!cache) {
    cache = fetch(`${apiBase()}/catalog/schema`)
      .then((res) => {
        if (!res.ok) {
          throw new Error(`HTTP ${res.status}`);
        }
        return res.json() as Promise<EntityType[]>;
      })
      .then((types) => new Map(types.map((t) => [t.name, t])))
      .catch((err: unknown) => {
        cache = null; // let a later call retry a failed fetch
        throw err instanceof Error ? err : new Error(String(err));
      });
  }
  return cache;
}

/** One entity type by name, or undefined if the server has no such type. */
export async function entityType(name: string): Promise<EntityType | undefined> {
  return (await entitySchema()).get(name);
}

/** Load a resource (its JSON:API `data`) plus its type, in parallel. */
export async function loadEntity(
  type: string,
  id: string,
): Promise<{ type?: EntityType; resource: Resource }> {
  const [typeDef, body] = await Promise.all([
    entityType(type),
    fetch(`${apiBase()}/catalog/items/${type}/${id}`).then((res) => {
      if (!res.ok) {
        throw new Error(`HTTP ${res.status}`);
      }
      return res.json() as Promise<JsonApiResource>;
    }),
  ]);
  return { type: typeDef, resource: body.data };
}
