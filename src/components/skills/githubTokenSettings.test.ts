import { describe, expect, it } from 'vitest'
import {
  buildGithubTokenSaveRequest,
  githubTokenSettingsReducer,
  githubTokenStatusErrorMessage,
  initialGithubTokenSettingsState,
} from './githubTokenSettings'

describe('GitHub token settings state', () => {
  it('represents a saved token without hydrating the secret into React state', () => {
    expect(initialGithubTokenSettingsState(true)).toEqual({
      draft: '',
      hasToken: true,
    })
  })

  it('does not turn a blank draft into an implicit delete request', () => {
    const configured = initialGithubTokenSettingsState(true)
    const edited = githubTokenSettingsReducer(configured, {
      type: 'draft_changed',
      draft: '   ',
    })

    expect(buildGithubTokenSaveRequest(edited.draft)).toBeNull()
    expect(edited.hasToken).toBe(true)
  })

  it('clears the draft and marks the token configured after a successful save', () => {
    const edited = githubTokenSettingsReducer(initialGithubTokenSettingsState(false), {
      type: 'draft_changed',
      draft: 'replacement-token',
    })

    expect(
      githubTokenSettingsReducer(edited, { type: 'save_succeeded' }),
    ).toEqual({ draft: '', hasToken: true })
  })

  it('changes configured state only after an explicit remove succeeds', () => {
    const configured = initialGithubTokenSettingsState(true)

    expect(
      githubTokenSettingsReducer(configured, { type: 'remove_succeeded' }),
    ).toEqual({ draft: '', hasToken: false })
  })

  it('preserves a credential status load failure for visible error feedback', () => {
    expect(
      githubTokenStatusErrorMessage(new Error('system credential access denied')),
    ).toBe('system credential access denied')
  })
})
