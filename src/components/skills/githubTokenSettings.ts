export type GithubTokenSettingsState = {
  draft: string
  hasToken: boolean
}

export type GithubTokenSettingsAction =
  | { type: 'status_loaded'; hasToken: boolean }
  | { type: 'draft_changed'; draft: string }
  | { type: 'save_succeeded' }
  | { type: 'remove_succeeded' }

export const initialGithubTokenSettingsState = (
  hasToken: boolean,
): GithubTokenSettingsState => ({ draft: '', hasToken })

export const buildGithubTokenSaveRequest = (
  draft: string,
): { token: string } | null => {
  const token = draft.trim()
  return token ? { token } : null
}

export const githubTokenStatusErrorMessage = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

export const githubTokenSettingsReducer = (
  state: GithubTokenSettingsState,
  action: GithubTokenSettingsAction,
): GithubTokenSettingsState => {
  switch (action.type) {
    case 'status_loaded':
      return { ...state, hasToken: action.hasToken }
    case 'draft_changed':
      return { ...state, draft: action.draft }
    case 'save_succeeded':
      return { draft: '', hasToken: true }
    case 'remove_succeeded':
      return { draft: '', hasToken: false }
  }
}
