import { renderToStaticMarkup } from 'react-dom/server'
import type { TFunction } from 'i18next'
import { describe, expect, it, vi } from 'vitest'
import AddSkillModal from './AddSkillModal'

const translate = ((key: string) => key) as TFunction

const renderModal = ({
  addModalTab,
  localPath = '',
  gitUrl = '',
}: {
  addModalTab: 'local' | 'git'
  localPath?: string
  gitUrl?: string
}) =>
  renderToStaticMarkup(
    <AddSkillModal
      open
      loading={false}
      canClose
      addModalTab={addModalTab}
      localPath={localPath}
      gitUrl={gitUrl}
      onRequestClose={vi.fn()}
      onTabChange={vi.fn()}
      onLocalPathChange={vi.fn()}
      onPickLocalPath={vi.fn()}
      onGitUrlChange={vi.fn()}
      onSubmit={vi.fn()}
      t={translate}
    />,
  )

describe('AddSkillModal source validation', () => {
  it.each([
    ['git', { gitUrl: '   ' }],
    ['local', { localPath: '   ' }],
  ] as const)('disables the %s action while its source is blank', (addModalTab, source) => {
    const markup = renderModal({ addModalTab, ...source })

    expect(markup).toMatch(
      /<button type="submit" class="btn btn-primary" disabled="">installFlow.detect<\/button>/,
    )
  })

  it.each([
    ['git', { gitUrl: 'https://github.com/example/skill.git' }],
    ['local', { localPath: '/tmp/example-skill' }],
  ] as const)('enables the %s action when its source is present', (addModalTab, source) => {
    const markup = renderModal({ addModalTab, ...source })

    expect(markup).toMatch(
      /<button type="submit" class="btn btn-primary">installFlow.detect<\/button>/,
    )
  })
})
