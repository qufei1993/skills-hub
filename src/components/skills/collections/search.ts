import type { CollectionIndexItem } from "./types.ts";

export function matchesCollection(item: CollectionIndexItem, query: string): boolean {
  const normalized = query.trim().toLowerCase();
  if (!normalized) return true;
  const english = item.localizations?.en;
  return [item.title, item.description, item.category, item.authorLabel,
    ...item.tags, english?.title, english?.description, ...(english?.tags ?? [])]
    .filter(Boolean).join(" ").toLowerCase().includes(normalized);
}
