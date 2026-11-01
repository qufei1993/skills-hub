export type ContentCheckSummary = {
  skillId: string
  name: string
  signature: string
  checkedAt: number
  status: 'changed' | 'unchanged' | 'failed'
  sourceRevision?: string | null
  targetCount: number
  error?: string | null
}

export type ContentCheckProgress = {
  checkId: string
  phase: 'preparing' | 'checking' | 'completed'
  total: number
  completed: number
  currentSkillId?: string | null
  summary?: ContentCheckSummary | null
}

export type ContentCheckRunResult = {
  checkId: string
  summaries: ContentCheckSummary[]
  cancelled: boolean
  total: number
  completed: number
  checkedAt: number
}

export type UpdateCandidate = ContentCheckSummary & { id: string; checkId: string }

export type CheckSources = (
  checkId: string,
  skillIds: string[],
  onProgress: (progress: ContentCheckProgress) => void,
) => Promise<ContentCheckRunResult>
