// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest'
import { useState } from 'react'
import { toast } from 'sonner'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { I18nextProvider } from 'react-i18next'
import i18n from '../../../i18n'
import CollectionInstallModal from './CollectionInstallModal'
import type { CollectionManifest } from '../types'

async function nextStep() {
  const next = screen.queryByRole('button', { name: '下一步' }) as HTMLButtonElement | null
  if (next) { await waitFor(() => expect(next.disabled).toBe(false)); fireEvent.click(next) }
}

async function clickInstall() {
  await nextStep()
  const button = screen.getByRole('button', { name: /安装 \d+ 个 Skills/ }) as HTMLButtonElement
  await waitFor(() => expect(button.disabled).toBe(false))
  fireEvent.click(button)
}

const manifest: CollectionManifest = { v: 1, title: '真实合集', sources: [{ repo: 'author/repo', ref: 'a'.repeat(40) }], skills: [{ name: 'design', path: 'skills/design', source: 0 }] }
afterEach(() => { cleanup(); vi.restoreAllMocks() })
describe('collection confirmation', () => {
  it('displays sources and requires a user click before any download or installation', async () => {
    await i18n.changeLanguage('zh')
    const calls: string[] = []
    render(<I18nextProvider i18n={i18n}><CollectionInstallModal manifest={manifest} tools={[]} invoke={async <T,>(command: string) => {
      if (command === 'preview_git_skills_local') return [] as T
      calls.push(command)
      return (command === 'list_git_skills_cmd' ? [{ name: 'design', subpath: 'skills/design', status: 'install' }] : { skill_id: 'id', name: 'design', central_path: '/tmp/design', action: 'installed' }) as T
    }} onComplete={async () => {}} onClose={() => {}} /></I18nextProvider>)
    expect(screen.getByText('真实合集')).toBeTruthy()
    expect(screen.getByText('skills/design')).toBeTruthy()
    expect(calls).toEqual([])
    await clickInstall()
    await waitFor(() => expect(screen.getByText('已安装')).toBeTruthy())
    expect(calls).toEqual(['list_git_skills_cmd', 'install_git_selection'])
    expect(screen.getByRole('button', { name: '完成' }).className).toContain('btn-primary')
  })
  it('keeps successfully installed items when retrying a failed distribution', async () => {
    const calls: string[] = []
    let attempts = 0
    render(<I18nextProvider i18n={i18n}><CollectionInstallModal manifest={manifest} tools={[{ id: 'cursor', label: 'Cursor' }]} invoke={async <T,>(command: string) => {
      if (command === 'preview_git_skills_local') return [] as T
      calls.push(command)
      if (command === 'sync_skill_to_tool' && attempts++ === 0) throw new Error('Failed to distribute')
      return (command === 'list_git_skills_cmd' ? [{ name: 'design', subpath: 'skills/design', status: 'install' }] : { skill_id: 'id', name: 'design', central_path: '/tmp/design', action: 'installed' }) as T
    }} onComplete={async () => {}} onClose={() => {}} /></I18nextProvider>)
    await nextStep()
  fireEvent.click(screen.getByLabelText('Cursor'))
    await clickInstall()
    await waitFor(() => expect(screen.getByText('Failed to distribute')).toBeTruthy())
    fireEvent.click(screen.getByRole('button', { name: '重试失败项' }))
    await waitFor(() => expect(screen.getByRole('button', { name: '完成' })).toBeTruthy())
    expect(calls).toEqual(['list_git_skills_cmd', 'install_git_selection', 'sync_skill_to_tool', 'sync_skill_to_tool'])
  })
})

it('preserves installation state while another dialog temporarily takes priority', async () => {
  let release: () => void = () => {}
  const wait = new Promise<void>(resolve => { release = resolve })
  const invoke = async <T,>(command: string) => {
    if (command === 'preview_git_skills_local') return [] as T
    if (command === 'list_git_skills_cmd') {
      await wait
      return [{ name: 'design', subpath: 'skills/design', status: 'install' }] as T
    }
    return { skill_id: 'id', name: 'design', central_path: '/tmp/design', action: 'installed' } as T
  }
  const props = { manifest, tools: [], invoke, onComplete: async () => {}, onClose: () => {} }
  const view = render(<I18nextProvider i18n={i18n}><CollectionInstallModal {...props} open /></I18nextProvider>)
  await clickInstall()
  view.rerender(<I18nextProvider i18n={i18n}><CollectionInstallModal {...props} open={false} /></I18nextProvider>)
  expect(screen.queryByRole('dialog')).toBeNull()
  release()
  view.rerender(<I18nextProvider i18n={i18n}><CollectionInstallModal {...props} open /></I18nextProvider>)
  await waitFor(() => expect(screen.getByRole('button', { name: '完成' })).toBeTruthy())
})

it('filters the visible list without changing the installation set', async () => {
  await i18n.changeLanguage('zh')
  const names: string[] = []
  const batch = { ...manifest, skills: [...manifest.skills, { name: 'review', path: 'skills/review', source: 0 }] }
  render(<I18nextProvider i18n={i18n}><CollectionInstallModal manifest={batch} tools={[]} invoke={async <T,>(command: string, args?: Record<string, unknown>) => {
    if (command === 'preview_git_skills_local') return [] as T
    if (command === 'list_git_skills_cmd') return batch.skills.map(skill => ({ ...skill, subpath: skill.path, status: 'install' })) as T
    if (command === 'install_git_selection') { names.push(String(args?.name)); return { skill_id: String(args?.name), name: args?.name, central_path: '/tmp/item', action: 'installed' } as T }
    return undefined as T
  }} onComplete={async () => {}} onClose={() => {}} /></I18nextProvider>)
  fireEvent.change(screen.getByRole('searchbox'), { target: { value: 'review' } })
  expect(screen.queryByText('design', { selector: 'strong' })).toBeNull()
  expect(screen.getByText('review', { selector: 'strong' })).toBeTruthy()
  await clickInstall()
  await waitFor(() => expect(screen.getByRole('button', { name: '完成' })).toBeTruthy())
  expect(names).toEqual(['design', 'review'])
})

it('creates one tag for new installs and reuses it after distribution failure', async () => {
  const calls: string[] = []
  let syncs = 0
  render(<I18nextProvider i18n={i18n}><CollectionInstallModal manifest={manifest} tools={[{ id: 'cursor', label: 'Cursor' }]} invoke={async <T,>(command: string, args?: Record<string, unknown>) => {
    if (command === 'preview_git_skills_local') return [] as T
      calls.push(command)
    if (command === 'create_tag') { expect(args).toEqual({ name: '工程测试' }); return { id: 7, name: '工程测试' } as T }
    if (command === 'preview_git_skills_local') return [] as T
    if (command === 'list_git_skills_cmd') return [{ name: 'design', subpath: 'skills/design', status: 'install' }] as T
    if (command === 'get_managed_skills') return [{ id: 'new', tags: [] }] as T
    if (command === 'sync_skill_to_tool' && syncs++ === 0) throw new Error('sync failed')
    return { skill_id: 'new', name: 'design', central_path: '/tmp/design', action: 'installed' } as T
  }} onComplete={async () => {}} onClose={() => {}} /></I18nextProvider>)
  await nextStep()
  fireEvent.change(screen.getByLabelText('新建标签'), { target: { value: '工程测试' } })
  await nextStep()
  fireEvent.click(screen.getByLabelText('Cursor'))
  await clickInstall()
  await waitFor(() => expect(screen.getByText('sync failed')).toBeTruthy())
  fireEvent.click(screen.getByRole('button', { name: '重试失败项' }))
  await waitFor(() => expect(screen.getByRole('button', { name: '完成' })).toBeTruthy())
  expect(calls.filter(command => command === 'create_tag')).toHaveLength(1)
  expect(calls.filter(command => command === 'install_git_selection')).toHaveLength(1)
  expect(calls.indexOf('set_skill_tags')).toBeLessThan(calls.indexOf('sync_skill_to_tool'))
})

it('requires a project before installation and keeps shared targets selected together', async () => {
  await i18n.changeLanguage('zh')
  const calls: string[] = []
  render(<I18nextProvider i18n={i18n}><CollectionInstallModal manifest={manifest}
    tools={[{ id: 'cursor', label: 'Cursor' }, { id: 'codex', label: 'Codex' }, { id: 'legacy', label: 'Legacy', supports_project_scope: false }]}
    sharedTools={{ cursor: ['cursor', 'codex'], codex: ['cursor', 'codex'] }}
    recentProjects={['/work/project']} invoke={async <T,>(command: string) => { if (command === 'preview_git_skills_local') return [] as T
      calls.push(command); return undefined as T }} onComplete={async () => {}} onClose={() => {}} /></I18nextProvider>)
  await nextStep()
  fireEvent.click(screen.getByLabelText('Cursor'))
  expect((screen.getByLabelText('Codex') as HTMLInputElement).checked).toBe(true)

  fireEvent.click(screen.getByRole('button', { name: i18n.t('scope.project') }))
  expect((screen.getByLabelText('Legacy') as HTMLInputElement).disabled).toBe(true)
  expect((screen.getByRole('button', { name: /安装 \d+ 个 Skills/ }) as HTMLButtonElement).disabled).toBe(true)
  fireEvent.click(screen.getByRole('button', { name: /\/work\/project/ }))
  await waitFor(() => expect((screen.getByRole('button', { name: /安装 \d+ 个 Skills/ }) as HTMLButtonElement).disabled).toBe(false))
  expect(calls).toEqual([])
})

it('allows returning to global after project scope clears the only selected tool', async () => {
  await i18n.changeLanguage('zh')
  render(<I18nextProvider i18n={i18n}><CollectionInstallModal manifest={manifest} tools={[{ id: 'hermes_agent', label: 'Hermes Agent', supports_project_scope: false }]} invoke={async <T,>() => [] as T} onComplete={async () => {}} onClose={() => {}} /></I18nextProvider>)
  await nextStep()
  fireEvent.click(screen.getByLabelText('Hermes Agent'))

  fireEvent.click(screen.getByRole('button', { name: i18n.t('scope.project') }))
  fireEvent.click(screen.getByRole('button', { name: i18n.t('scope.global') }))
  expect((screen.getByLabelText('Hermes Agent') as HTMLInputElement).disabled).toBe(false)
})

it('updates shared tool selections when switching installation scope', async () => {
  render(<I18nextProvider i18n={i18n}><CollectionInstallModal manifest={manifest} tools={[{ id: 'codex', label: 'Codex' }, { id: 'antigravity', label: 'Antigravity' }]}
    sharedProjectTools={{ codex: ['codex', 'antigravity'], antigravity: ['codex', 'antigravity'] }} invoke={async <T,>() => [] as T} onComplete={async () => {}} onClose={() => {}} /></I18nextProvider>)
  await nextStep()
  fireEvent.click(screen.getByLabelText('Codex'))
  expect((screen.getByLabelText('Antigravity') as HTMLInputElement).checked).toBe(false)

  fireEvent.click(screen.getByRole('button', { name: i18n.t('scope.project') }))
  expect((screen.getByLabelText('Antigravity') as HTMLInputElement).checked).toBe(true)
})

it('keeps explicit selection through search and retries only selected failures', async () => {
  await i18n.changeLanguage('zh')
  const batch = { ...manifest, skills: [...manifest.skills, { name: 'review', path: 'skills/review', source: 0 }] }
  const names: string[] = []
  render(<I18nextProvider i18n={i18n}><CollectionInstallModal manifest={batch} tools={[]} invoke={async <T,>(command: string, args?: Record<string, unknown>) => {
    if (command === 'preview_git_skills_local') return [] as T
    if (command === 'list_git_skills_cmd') return batch.skills.map(skill => ({ ...skill, subpath: skill.path, status: 'install' })) as T
    if (command === 'install_git_selection') {
      names.push(String(args?.name))
      if (names.length === 1) throw new Error('temporary failure')
      return { skill_id: 'review', name: 'review', central_path: '/tmp/review', action: 'installed' } as T
    }
    return undefined as T
  }} onComplete={async () => {}} onClose={() => {}} /></I18nextProvider>)
  expect((screen.getByRole('checkbox', { name: '全选' }) as HTMLInputElement).checked).toBe(true)
  fireEvent.click(screen.getByRole('checkbox', { name: '选择 design' }))
  expect((screen.getByRole('checkbox', { name: '全选' }) as HTMLInputElement).indeterminate).toBe(true)
  fireEvent.change(screen.getByRole('searchbox'), { target: { value: 'design' } })
  expect(screen.getAllByText('已选 1 / 2')[0]).toBeTruthy()
  await clickInstall()
  await waitFor(() => expect(screen.getByRole('button', { name: '重试失败项' })).toBeTruthy())
  expect((screen.getByRole('checkbox', { name: '选择 design' }) as HTMLInputElement).disabled).toBe(true)
  fireEvent.click(screen.getByRole('button', { name: '重试失败项' }))
  await waitFor(() => expect(screen.getByRole('button', { name: '完成' })).toBeTruthy())
  expect(names).toEqual(['review', 'review'])
  expect(screen.getByText('已完成 1 / 1')).toBeTruthy()
})

it('disables installation when all skills are deselected and restores the entire collection', async () => {
  await i18n.changeLanguage('zh')
  const calls: string[] = []
  render(<I18nextProvider i18n={i18n}><CollectionInstallModal manifest={manifest} tools={[]} invoke={async <T,>(command: string) => { if (command === 'preview_git_skills_local') return [] as T
      calls.push(command); return undefined as T }} onComplete={async () => {}} onClose={() => {}} /></I18nextProvider>)
  fireEvent.click(screen.getByRole('checkbox', { name: '全选' }))
  const install = screen.getByRole('button', { name: '下一步' }) as HTMLButtonElement
  expect(install.disabled).toBe(true)
  fireEvent.click(install)
  expect(calls).toEqual([])
  fireEvent.click(screen.getByRole('checkbox', { name: '全选' }))
  expect((screen.getByRole('checkbox', { name: '选择 design' }) as HTMLInputElement).checked).toBe(true)
})


it('keeps failures open, then closes and announces success once after retry and refresh', async () => {
  await i18n.changeLanguage('zh')
  const success = vi.spyOn(toast, 'success').mockImplementation(() => 'test')
  const events: string[] = []
  let attempts = 0
  function Harness() {
    const [open, setOpen] = useState(true)
    return <I18nextProvider i18n={i18n}><CollectionInstallModal open={open} manifest={manifest} tools={[]}
      invoke={async <T,>(command: string) => {
        if (command === 'preview_git_skills_local') return [] as T
    if (command === 'list_git_skills_cmd') return [{ name: 'design', subpath: 'skills/design', status: 'install' }] as T
        if (attempts++ === 0) throw new Error('retry me')
        return { skill_id: 'id', name: 'design', central_path: '/tmp/design', action: 'installed' } as T
      }} onComplete={async () => { events.push('refresh') }} onClose={() => { events.push('close'); setOpen(false) }} /></I18nextProvider>
  }
  render(<Harness />)
  await clickInstall()
  await waitFor(() => expect(screen.getByText('retry me')).toBeTruthy())
  expect(screen.getByRole('dialog')).toBeTruthy()
  expect(success).not.toHaveBeenCalled()
  fireEvent.click(screen.getByRole('button', { name: '重试失败项' }))
  await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull())
  expect(events).toEqual(['refresh', 'refresh', 'close'])
  expect(success).toHaveBeenCalledExactlyOnceWith('合集安装成功，共 1 个 Skills')
})


it('previews installed and conflicting sources before installation without downloading', async () => {
  await i18n.changeLanguage('zh')
  const calls: string[] = []
  render(<I18nextProvider i18n={i18n}><CollectionInstallModal manifest={manifest} tools={[]}
    invoke={async <T,>(command: string) => {
      calls.push(command)
      return [{ name: 'design', subpath: 'skills/design', status: 'update', existing_skill_id: 'renamed-id' }] as T
    }} onComplete={async () => {}} onClose={() => {}} /></I18nextProvider>)
  await waitFor(() => expect(screen.getByText('已在技能库中')).toBeTruthy())
  expect(screen.queryByText('待安装')).toBeNull()
  expect(calls).toEqual(['preview_git_skills_local'])
  expect((screen.getByRole('button', { name: '下一步' }) as HTMLButtonElement).disabled).toBe(false)
})

it('blocks installation when the local preview fails and allows retrying the check', async () => {
  await i18n.changeLanguage('zh')
  let attempts = 0
  render(<I18nextProvider i18n={i18n}><CollectionInstallModal manifest={manifest} tools={[]}
    invoke={async <T,>(command: string) => {
      expect(command).toBe('preview_git_skills_local')
      if (attempts++ === 0) throw new Error('database unavailable')
      return [{ name: 'design', subpath: 'skills/design', status: 'conflict' }] as T
    }} onComplete={async () => {}} onClose={() => {}} /></I18nextProvider>)
  await waitFor(() => expect(screen.getByRole('alert')).toBeTruthy())
  expect((screen.getByRole('button', { name: '下一步' }) as HTMLButtonElement).disabled).toBe(true)
  fireEvent.click(screen.getByRole('button', { name: '重新检查' }))
  await waitFor(() => expect(screen.getByText('冲突 · 已跳过')).toBeTruthy())
  expect(screen.queryByRole('alert')).toBeNull()
})


it('keeps selection and settings across both steps without installing on Next', async () => {
  await i18n.changeLanguage('zh')
  const calls: string[] = []
  const invoke = async <T,>(command: string) => { calls.push(command); return [] as T }
  const batch = { ...manifest, skills: [...manifest.skills, { name: 'review', path: 'skills/review', source: 0 }] }
  render(<I18nextProvider i18n={i18n}><CollectionInstallModal manifest={batch} tools={[{ id: 'cursor', label: 'Cursor' }]} invoke={invoke} onComplete={async () => {}} onClose={() => {}} /></I18nextProvider>)
  expect(screen.queryByRole('checkbox', { name: 'Cursor' })).toBeNull()
  fireEvent.click(screen.getByRole('checkbox', { name: '选择 design' }))
  await nextStep()
  expect(screen.queryByRole('searchbox')).toBeNull()
  fireEvent.keyDown(document.activeElement!, { key: 'Tab', shiftKey: true })
  expect(document.activeElement).toBe(screen.getByRole('button', { name: '安装 1 个 Skills' }))
  fireEvent.click(screen.getByRole('checkbox', { name: 'Cursor' }))
  fireEvent.click(screen.getByRole('button', { name: '上一步' }))
  expect((screen.getByRole('checkbox', { name: '选择 design' }) as HTMLInputElement).checked).toBe(false)
  await nextStep()
  expect((screen.getByRole('checkbox', { name: 'Cursor' }) as HTMLInputElement).checked).toBe(true)
  expect(calls).toEqual(['preview_git_skills_local'])
})

it('excludes preview conflicts from selection, select-all and the installation request', async () => {
  await i18n.changeLanguage('zh')
  const batch = { ...manifest, skills: [...manifest.skills, { name: 'review', path: 'skills/review', source: 0 }] }
  const installed: string[] = []
  const invoke = async <T,>(command: string, args?: Record<string, unknown>) => {
    if (command === 'preview_git_skills_local' || command === 'list_git_skills_cmd') return batch.skills.map((skill, index) => ({ name: skill.name, subpath: skill.path, status: index === 0 ? 'conflict' : 'install' })) as T
    if (command === 'install_git_selection') {
      installed.push(String(args?.name))
      return { skill_id: 'review', name: 'review', central_path: '/tmp/review', action: 'installed' } as T
    }
    return undefined as T
  }
  render(<I18nextProvider i18n={i18n}><CollectionInstallModal manifest={batch} tools={[]} invoke={invoke} onComplete={async () => {}} onClose={() => {}} /></I18nextProvider>)
  const conflict = screen.getByRole('checkbox', { name: '选择 design' }) as HTMLInputElement
  await waitFor(() => expect(conflict.disabled).toBe(true))
  expect(conflict.checked).toBe(false)
  fireEvent.click(screen.getByRole('checkbox', { name: '全选' }))
  fireEvent.click(screen.getByRole('checkbox', { name: '全选' }))
  expect(conflict.checked).toBe(false)
  await clickInstall()
  await waitFor(() => expect(installed).toEqual(['review']))
})
