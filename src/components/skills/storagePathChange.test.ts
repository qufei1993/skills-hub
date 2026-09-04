import { describe, expect, it } from 'vitest'
import { requiresStorageMigrationConfirmation } from './storagePathChange'

describe('requiresStorageMigrationConfirmation', () => {
  it('requires confirmation when managed Skills will move', () => {
    expect(
      requiresStorageMigrationConfirmation({
        current_path: '/Users/test/.skillshub',
        new_path: '/Users/test/Skills',
        skill_count: 3,
      }),
    ).toBe(true)
  })

  it('does not require confirmation when no managed Skills will move', () => {
    expect(
      requiresStorageMigrationConfirmation({
        current_path: '/Users/test/.skillshub',
        new_path: '/Users/test/.skillshub',
        skill_count: 0,
      }),
    ).toBe(false)
  })
})
