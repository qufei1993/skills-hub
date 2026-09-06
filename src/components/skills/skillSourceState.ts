import type { ManagedSkill } from './types'

export function hasUnboundLocalSource(skill: Pick<ManagedSkill, 'source_type' | 'source_ref'>) {
  return skill.source_type === 'local' && !skill.source_ref?.trim()
}
