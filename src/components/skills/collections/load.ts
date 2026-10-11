import type { CollectionInvoke } from '../collectionInstall'

export type CollectionSnapshot = { version?: string }

export async function loadWebsiteCollection<T>(invoke: CollectionInvoke, validate: (value: unknown) => T, id?: string, snapshot?: CollectionSnapshot): Promise<T> {
  const version = id ? snapshot?.version : undefined
  const checked = (raw: unknown) => {
    const result = validate(raw)
    const received = (raw as { snapshotVersion?: string })?.snapshotVersion
    if (received !== undefined && !/^[a-f0-9]{64}$/.test(received)) throw new Error('Invalid collection snapshot')
    if (version && received !== version) throw new Error('Collection snapshot changed')
    if (!id && snapshot) snapshot.version = received
    return result
  }
  try {
    const raw = await invoke('get_website_collections', { id, version })
    const result = checked(raw)
    // A cache write failure must not hide valid online content.
    try { await invoke('cache_website_collections', { id, value: raw }) } catch { /* Read-only storage can still browse online. */ }
    return result
  } catch {
    return checked(await invoke('get_website_collections', { id, version, cached: true }))
  }
}
