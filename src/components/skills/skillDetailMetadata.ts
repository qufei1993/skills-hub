export function getVisibleFrontmatterEntries(
  meta: Record<string, string> | null,
): [string, string][] {
  if (!meta) return []
  return Object.entries(meta).filter(
    ([key]) => !['name', 'description'].includes(key.toLowerCase()),
  )
}
