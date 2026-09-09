import { describe, expect, it } from 'vitest'
import { skillBoardColumns } from './skillBoardLayout'

describe('skillBoardColumns', () => {
  it('maps Magnet-like board widths to 1–4 columns', () => {
    expect(skillBoardColumns(280)).toBe(1)
    expect(skillBoardColumns(559)).toBe(1)
    expect(skillBoardColumns(560)).toBe(2)
    expect(skillBoardColumns(664)).toBe(2)
    expect(skillBoardColumns(899)).toBe(2)
    expect(skillBoardColumns(900)).toBe(3)
    expect(skillBoardColumns(1236)).toBe(3)
    expect(skillBoardColumns(1320)).toBe(4)
    expect(skillBoardColumns(1600)).toBe(4)
  })
})
