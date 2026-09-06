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
      tags={[]}
      selectedTagIds={[]}
      syncTargets={{}}
      installedTools={[]}
      toolStatus={null}
      installScope="global"
      installProjects={[]}
      recentProjects={[]}
      onRequestClose={vi.fn()}
      onTabChange={vi.fn()}
      onLocalPathChange={vi.fn()}
      onPickLocalPath={vi.fn()}
      onGitUrlChange={vi.fn()}
      onToggleTag={vi.fn()}
      onSyncTargetChange={vi.fn()}
      onInstallScopeChange={vi.fn()}
      onInstallProjectsChange={vi.fn()}
      onPickProject={vi.fn()}
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
      /<button class="btn btn-primary" disabled="">(?:install|create)<\/button>/,
    )
  })

  it.each([
    ['git', { gitUrl: 'https://github.com/example/skill.git' }],
    ['local', { localPath: '/tmp/example-skill' }],
  ] as const)('enables the %s action when its source is present', (addModalTab, source) => {
    const markup = renderModal({ addModalTab, ...source })

    expect(markup).toMatch(
      /<button class="btn btn-primary">(?:install|create)<\/button>/,
    )
  })
})
