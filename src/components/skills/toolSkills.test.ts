import { describe, expect, it } from 'vitest'
import type { ManagedSkill, ToolOption } from './types'
import { buildToolSkillGroups, getSkillsForTool } from './toolSkills'

describe('toolSkills helpers', () => {
  const tools: ToolOption[] = [
    { id: 'codex', label: 'Codex' },
    { id: 'claude_code', label: 'Claude Code' },
    { id: 'cursor', label: 'Cursor' },
  ]

  const managedSkills: ManagedSkill[] = [
    {
      id: 's1',
      name: 'Spec Writer',
      source_type: 'git',
      source_ref: null,
      central_path: '/hub/spec-writer',
      created_at: 1,
      updated_at: 10,
      status: 'active',
      targets: [
        {
          tool: 'codex',
          mode: 'link',
          status: 'ok',
          target_path: '/tools/codex/spec-writer',
          synced_at: 10,
        },
        {
          tool: 'claude_code',
          mode: 'link',
          status: 'ok',
          target_path: '/tools/claude/spec-writer',
          synced_at: 10,
        },
      ],
    },
    {
      id: 's2',
      name: 'Code Reviewer',
      source_type: 'local',
      source_ref: null,
      central_path: '/hub/reviewer',
      created_at: 2,
      updated_at: 20,
      status: 'active',
      targets: [
        {
          tool: 'codex',
          mode: 'link',
          status: 'ok',
          target_path: '/tools/codex/reviewer',
          synced_at: 20,
        },
      ],
    },
  ]

  it('builds tool groups with skill counts and installation status', () => {
    const groups = buildToolSkillGroups(managedSkills, tools, ['codex'])

    expect(groups).toEqual([
      { toolId: 'codex', label: 'Codex', skillCount: 2, installed: true },
      {
        toolId: 'claude_code',
        label: 'Claude Code',
        skillCount: 1,
        installed: false,
      },
      { toolId: 'cursor', label: 'Cursor', skillCount: 0, installed: false },
    ])
  })

  it('returns only skills synced to selected tool', () => {
    const codexSkills = getSkillsForTool(managedSkills, 'codex')
    const claudeSkills = getSkillsForTool(managedSkills, 'claude_code')

    expect(codexSkills.map((s) => s.name)).toEqual(['Code Reviewer', 'Spec Writer'])
    expect(claudeSkills.map((s) => s.name)).toEqual(['Spec Writer'])
  })
})
