// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { useState, type ComponentProps } from 'react'
import type { TFunction } from 'i18next'
import { afterEach, describe, expect, it, vi } from 'vitest'
import SettingsPage from './SettingsPage'
import ToolsPage from './ToolsPage'
import type { ToolConfigDto, ToolStatusDto } from './types'

const messages: Record<string, string> = {
  cleanNow: 'Clean now',
  cancel: 'Cancel',
  'gitCacheConfirm.title': 'Clear Git cache?',
  'gitCacheConfirm.body':
    'This removes downloaded Git cache from this computer. Installed Skills and local source folders are not affected. Repositories will be downloaded again when needed.',
  'gitCacheConfirm.confirm': 'Clear cache',
  'toolManagement.removeCustom': 'Remove custom tool',
  'toolManagement.removeCustomTitle': 'Remove custom tool?',
  'toolManagement.removeCustomBody':
    'This removes the tool configuration for "Demo tool" from Skills Hub. Skill files in /Users/demo/.demo/skills are not deleted. Add the tool again to resume management and sync.',
  'toolManagement.removeCustomConfirm': 'Remove tool',
}

const t = ((key: string, options?: Record<string, unknown>) => {
  let message = messages[key] ?? key
  for (const [name, value] of Object.entries(options ?? {})) {
    message = message.replaceAll(`{{${name}}}`, String(value))
  }
  return message
}) as unknown as TFunction

const customToolConfig: ToolConfigDto = {
  disabled_builtin_tools: [],
  custom_tools: [
    {
      key: 'custom_demo',
      label: 'Demo tool',
      avatar: null,
      skills_dir: '/Users/demo/.demo/skills',
      project_skills_dir: null,
      sync_mode: 'copy',
      enabled: true,
    },
  ],
}

const toolStatus: ToolStatusDto = {
  installed: ['custom_demo'],
  newly_installed: [],
  tools: [
    {
      key: 'custom_demo',
      label: 'Demo tool',
      avatar: null,
      installed: true,
      enabled: true,
      is_custom: true,
      skills_dir: '/Users/demo/.demo/skills',
      project_skills_dir: '',
      supports_project_scope: false,
      sync_mode: 'copy',
    },
  ],
}

afterEach(cleanup)

const renderToolsPage = (
  save: (config: ToolConfigDto) => Promise<boolean>,
) => {
  const Harness = () => {
    const [config, setConfig] = useState(customToolConfig)
    const handleChange = async (nextConfig: ToolConfigDto) => {
      setConfig(nextConfig)
      const saved = await save(nextConfig)
      if (!saved) setConfig(customToolConfig)
      return saved
    }
    return (
      <ToolsPage
        toolStatus={toolStatus}
        toolConfig={config}
        onToolConfigChange={handleChange}
        t={t}
      />
    )
  }
  return render(<Harness />)
}

const settingsProps = (
  onClearGitCacheNow: () => Promise<boolean>,
): ComponentProps<typeof SettingsPage> => ({
  isTauri: false,
  language: 'en',
  storagePath: '/Users/demo/.skillshub',
  gitCacheCleanupDays: 7,
  gitCacheTtlSecs: 300,
  themePreference: 'system',
  githubTokenDraft: '',
  githubTokenConfigured: false,
  githubProxyConfig: {
    enabled: false,
    port: 7890,
    url: 'http://127.0.0.1:7890',
    auto_detected: false,
  },
  discoveryScanEnabledCount: 1,
  discoveryScanSourceCount: 1,
  onPickStoragePath: vi.fn(),
  onToggleLanguage: vi.fn(),
  onThemeChange: vi.fn(),
  onGitCacheCleanupDaysChange: vi.fn(),
  onGitCacheTtlSecsChange: vi.fn(),
  onClearGitCacheNow,
  onGithubTokenDraftChange: vi.fn(),
  onGithubTokenSave: vi.fn(),
  onGithubTokenRemove: vi.fn(),
  onGithubProxyConfigChange: vi.fn(),
  onOpenDiscoveryScanSettings: vi.fn(),
  onBack: vi.fn(),
  t,
})

describe('destructive action confirmations', () => {
  it('waits for confirmation before removing a custom tool configuration', async () => {
    const onToolConfigChange = vi.fn(async () => true)
    renderToolsPage(onToolConfigChange)

    fireEvent.click(screen.getByRole('button', { name: 'Remove custom tool' }))

    expect(onToolConfigChange).not.toHaveBeenCalled()
    expect(screen.getByRole('dialog', { name: 'Remove custom tool?' })).toBeTruthy()
    expect(screen.getByText(messages['toolManagement.removeCustomBody'])).toBeTruthy()

    fireEvent.click(screen.getByRole('button', { name: 'Remove tool' }))

    await waitFor(() => {
      expect(onToolConfigChange).toHaveBeenCalledWith({
        disabled_builtin_tools: [],
        custom_tools: [],
      })
    })
  })

  it('keeps the custom tool confirmation stable during an optimistic failed save', async () => {
    let finishSave: ((saved: boolean) => void) | undefined
    const saveResult = new Promise<boolean>((resolve) => {
      finishSave = resolve
    })
    const onToolConfigChange = vi.fn(() => saveResult)
    renderToolsPage(onToolConfigChange)

    fireEvent.click(screen.getByRole('button', { name: 'Remove custom tool' }))
    fireEvent.click(screen.getByRole('button', { name: 'Remove tool' }))

    await waitFor(() => expect(onToolConfigChange).toHaveBeenCalledOnce())
    expect(screen.getByRole('dialog', { name: 'Remove custom tool?' })).toBeTruthy()
    expect(screen.getByRole('button', { name: 'Remove tool' }).hasAttribute('disabled')).toBe(true)

    finishSave?.(false)

    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Remove tool' }).hasAttribute('disabled')).toBe(false)
    })
    expect(screen.getByRole('dialog', { name: 'Remove custom tool?' })).toBeTruthy()
  })

  it('waits for confirmation before clearing the Git cache', async () => {
    const onClearGitCacheNow = vi.fn(async () => true)
    render(<SettingsPage {...settingsProps(onClearGitCacheNow)} />)

    fireEvent.click(screen.getByRole('button', { name: 'Clean now' }))

    expect(onClearGitCacheNow).not.toHaveBeenCalled()
    expect(screen.getByRole('dialog', { name: 'Clear Git cache?' })).toBeTruthy()
    expect(screen.getByText(messages['gitCacheConfirm.body'])).toBeTruthy()

    fireEvent.click(screen.getByRole('button', { name: 'Clear cache' }))

    await waitFor(() => expect(onClearGitCacheNow).toHaveBeenCalledOnce())
  })

  it('keeps the Git cache confirmation open when cleanup fails', async () => {
    const onClearGitCacheNow = vi.fn(async () => false)
    render(<SettingsPage {...settingsProps(onClearGitCacheNow)} />)

    fireEvent.click(screen.getByRole('button', { name: 'Clean now' }))
    fireEvent.click(screen.getByRole('button', { name: 'Clear cache' }))

    await waitFor(() => expect(onClearGitCacheNow).toHaveBeenCalledOnce())
    expect(screen.getByRole('dialog', { name: 'Clear Git cache?' })).toBeTruthy()
  })

  it('traps focus, closes on Escape, and restores focus to the trigger', () => {
    render(<SettingsPage {...settingsProps(vi.fn(async () => true))} />)
    const trigger = screen.getByRole('button', { name: 'Clean now' })

    trigger.focus()
    fireEvent.click(trigger)

    const cancel = screen.getByRole('button', { name: 'Cancel' })
    const confirm = screen.getByRole('button', { name: 'Clear cache' })
    expect(document.activeElement).toBe(cancel)

    fireEvent.keyDown(document, { key: 'Tab', shiftKey: true })
    expect(document.activeElement).toBe(confirm)
    fireEvent.keyDown(document, { key: 'Tab' })
    expect(document.activeElement).toBe(cancel)

    fireEvent.keyDown(document, { key: 'Escape' })
    expect(screen.queryByRole('dialog')).toBeNull()
    expect(document.activeElement).toBe(trigger)
  })
})
