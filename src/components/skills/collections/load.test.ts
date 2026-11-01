import { expect, it } from 'vitest'
import { loadWebsiteCollection } from './load'
import { validateIndex } from './validation'
import index from './fixtures/index.json'
it('does not replace a usable cache with invalid HTTP 200 content', async () => {
  let cached: unknown = index
  const result = await loadWebsiteCollection(async <T,>(command: string, args?: Record<string, unknown>) => {
    if (command === 'cache_website_collections') { cached = args?.value; return undefined as T }
    return (args?.cached ? cached : { schemaVersion: 1, collections: [{}] }) as T
  }, validateIndex)
  expect(result.length).toBe(index.collections.length)
  expect(cached).toBe(index)
})

it('pins details and their offline fallback to the validated index snapshot', async () => {
  const session = { version: undefined as string | undefined }
  const version = 'a'.repeat(64)
  const invoke = async <T,>(command: string, args?: Record<string, unknown>) => {
    if (command === 'cache_website_collections') return undefined as T
    if (!args?.id) return { ...index, snapshotVersion: version } as T
    expect(args.version).toBe(version)
    if (!args.cached) throw new Error('offline')
    return { snapshotVersion: version, name: 'cached detail' } as T
  }
  await loadWebsiteCollection(invoke, validateIndex, undefined, session)
  expect(session.version).toBe(version)
  const detail = await loadWebsiteCollection(invoke, value => value, 'detail-id', session)
  expect(detail).toMatchObject({ name: 'cached detail' })
})
