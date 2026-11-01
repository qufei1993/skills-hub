// @vitest-environment jsdom
import { afterEach, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import type { TFunction } from 'i18next'
import InstallSettingsSections from './InstallSettingsSections'
import AddSkillModal from './AddSkillModal'
const t = ((key: string) => key) as TFunction
afterEach(cleanup)

it('shows tags, tools and scope immediately without a reveal button', () => {
  const props = { tags: <input aria-label="new tag" />, tools: <input type="checkbox" aria-label="Tool" />, scope: <input aria-label="project" />, toolSummary: '0', disabled: false, t }
  const view = render(<InstallSettingsSections {...props} />)
  expect(screen.getByRole('checkbox', { name: 'Tool' })).toBeTruthy()
  expect(screen.getByRole('textbox', { name: 'project' })).toBeTruthy()
  const tag = screen.getByRole('textbox', { name: 'new tag' }) as HTMLInputElement
  fireEvent.change(tag, { target: { value: 'test-tag' } })
  expect(screen.queryByRole('button')).toBeNull()
  view.rerender(<InstallSettingsSections {...props} disabled />)
  expect(tag.value).toBe('test-tag')
  expect(tag.matches(':disabled')).toBe(true)
})

it('focuses the source input and restores the entry button after closing', () => {
  const entry = document.createElement('button')
  document.body.append(entry)
  entry.focus()
  const props = { open: true, loading: false, canClose: true, addModalTab: 'git' as const, localPath: '', gitUrl: '', onRequestClose: vi.fn(), onTabChange: vi.fn(), onLocalPathChange: vi.fn(), onPickLocalPath: vi.fn(), onGitUrlChange: vi.fn(), onSubmit: vi.fn(), t }
  const view = render(<AddSkillModal {...props} />)
  expect(document.activeElement).toBe(screen.getByRole('textbox'))
  fireEvent.keyDown(document.activeElement!, { key: 'Escape' })
  expect(props.onRequestClose).toHaveBeenCalledOnce()
  view.rerender(<AddSkillModal {...props} open={false} />)
  expect(document.activeElement).toBe(entry)
  entry.remove()
})
