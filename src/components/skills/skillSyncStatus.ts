import type { ManagedSkill } from './types'

export type SkillSyncState =
  | 'disabled'
  | 'healthy'
  | 'partial'
  | 'failed'
  | 'idle'

export type ToolSyncState = 'synced' | 'partial' | 'failed' | 'not-synced'

type SkillTarget = ManagedSkill['targets'][number]

export const isActiveSkillTarget = (target: SkillTarget) =>
  target.status !== 'disabled'

export const isSuccessfulSkillTarget = (target: SkillTarget) =>
  target.status === 'ok'

export function getSkillSyncState(
  skill: Pick<ManagedSkill, 'enabled' | 'targets'>,
): SkillSyncState {
  if (!skill.enabled) return 'disabled'
  const activeTargets = skill.targets.filter(isActiveSkillTarget)
  if (activeTargets.length === 0) return 'idle'
  const successfulCount = activeTargets.filter(isSuccessfulSkillTarget).length
  if (successfulCount === activeTargets.length) return 'healthy'
  if (successfulCount === 0) return 'failed'
  return 'partial'
}

export function getToolSyncState(
  skill: Pick<ManagedSkill, 'targets'>,
  toolId: string,
  scope: 'global' | 'project',
): ToolSyncState {
  const targets = skill.targets.filter(
    (target) =>
      target.tool === toolId &&
      (target.scope ?? 'global') === scope &&
      isActiveSkillTarget(target),
  )
  if (targets.length === 0) return 'not-synced'
  const successfulCount = targets.filter(isSuccessfulSkillTarget).length
  if (successfulCount === targets.length) return 'synced'
  if (successfulCount === 0) return 'failed'
  return 'partial'
}
