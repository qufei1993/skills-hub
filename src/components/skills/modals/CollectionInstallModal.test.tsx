// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { I18nextProvider } from 'react-i18next'
import i18n from '../../../i18n'
import CollectionInstallModal from './CollectionInstallModal'
import type { CollectionManifest } from '../types'

const manifest: CollectionManifest = { v: 1, title: '真实合集', sources: [{ repo: 'author/repo', ref: 'a'.repeat(40) }], skills: [{ name: 'design', path: 'skills/design', source: 0 }] }
afterEach(cleanup)
describe('collection confirmation', () => {
  it('displays sources and requires a user click before any download or installation', async () => {
    await i18n.changeLanguage('zh')
    const calls: string[] = []
    render(<I18nextProvider i18n={i18n}><CollectionInstallModal manifest={manifest} tools={[]} invoke={async <T,>(command: string) => {
      calls.push(command)
      return (command === 'list_git_skills_cmd' ? [{ name: 'design', subpath: 'skills/design', status: 'install' }] : { skill_id: 'id', name: 'design', central_path: '/tmp/design', action: 'installed' }) as T
    }} onComplete={async () => {}} onClose={() => {}} /></I18nextProvider>)
    expect(screen.getByText('真实合集')).toBeTruthy()
    expect(screen.getByText('author/repo · skills/design')).toBeTruthy()
    expect(calls).toEqual([])
    fireEvent.click(screen.getByRole('button', { name: '安装 Skills' }))
    await waitFor(() => expect(screen.getByText('已安装')).toBeTruthy())
    expect(calls).toEqual(['list_git_skills_cmd', 'install_git_selection'])
    expect(screen.getByRole('button', { name: '完成' }).className).toContain('btn-primary')
  })
  it('keeps successfully installed items when retrying a failed distribution', async () => {
    const calls: string[] = []
    let attempts = 0
    render(<I18nextProvider i18n={i18n}><CollectionInstallModal manifest={manifest} tools={[{ id: 'cursor', label: 'Cursor' }]} invoke={async <T,>(command: string) => {
      calls.push(command)
      if (command === 'sync_skill_to_tool' && attempts++ === 0) throw new Error('Failed to distribute')
      return (command === 'list_git_skills_cmd' ? [{ name: 'design', subpath: 'skills/design', status: 'install' }] : { skill_id: 'id', name: 'design', central_path: '/tmp/design', action: 'installed' }) as T
    }} onComplete={async () => {}} onClose={() => {}} /></I18nextProvider>)
    fireEvent.click(screen.getByLabelText('Cursor'))
    fireEvent.click(screen.getByRole('button', { name: '安装 Skills' }))
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
    if (command === 'list_git_skills_cmd') {
      await wait
      return [{ name: 'design', subpath: 'skills/design', status: 'install' }] as T
    }
    return { skill_id: 'id', name: 'design', central_path: '/tmp/design', action: 'installed' } as T
  }
  const props = { manifest, tools: [], invoke, onComplete: async () => {}, onClose: () => {} }
  const view = render(<I18nextProvider i18n={i18n}><CollectionInstallModal {...props} open /></I18nextProvider>)
  fireEvent.click(screen.getByRole('button', { name: '安装 Skills' }))
  view.rerender(<I18nextProvider i18n={i18n}><CollectionInstallModal {...props} open={false} /></I18nextProvider>)
  expect(screen.queryByRole('dialog')).toBeNull()
  release()
  view.rerender(<I18nextProvider i18n={i18n}><CollectionInstallModal {...props} open /></I18nextProvider>)
  await waitFor(() => expect(screen.getByRole('button', { name: '完成' })).toBeTruthy())
})
