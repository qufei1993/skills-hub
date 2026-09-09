import { describe, expect, it } from 'vitest'
import { buildTemporarySnapshot, hasAnyTag, hasTag, temporaryActivationKey } from './temporaryActivation'

describe('temporary activation', () => {
  it('builds a target key per tool, scope, and project', () => {
    expect(temporaryActivationKey('codex', 'global')).toBe('codex|global|')
    expect(temporaryActivationKey('codex', 'project', '/tmp/app')).toBe('codex|project|/tmp/app')
  })

  it('matches a selected tag or untagged skills', () => {
    expect(hasTag([{ id: 3 }], 3)).toBe(true)
    expect(hasTag([{ id: 3 }], 4)).toBe(false)
    expect(hasTag([], null)).toBe(true)
    expect(hasTag([{ id: 3 }], null)).toBe(false)
  })

  it('matches any of the selected tags including untagged', () => {
    expect(hasAnyTag([{ id: 3 }], [3, 8])).toBe(true)
    expect(hasAnyTag([{ id: 3 }], [8, 9])).toBe(false)
    expect(hasAnyTag([], [3, null])).toBe(true)
    expect(hasAnyTag([{ id: 3 }], [])).toBe(true)
  })

  it('snapshots only the selected tool and scope', () => {
    const skills = [
      {
        id: 'a',
        targets: [{ tool: 'codex', scope: 'global', status: 'ok' }],
      },
      {
        id: 'b',
        targets: [{ tool: 'codex', scope: 'project', project_path: '/tmp/app', status: 'ok' }],
      },
    ]
    expect(buildTemporarySnapshot(skills, 'codex', 'global')).toEqual({ a: true, b: false })
    expect(buildTemporarySnapshot(skills, 'codex', 'project', '/tmp/app')).toEqual({ a: false, b: true })
  })
})
