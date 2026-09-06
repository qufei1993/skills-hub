// @vitest-environment jsdom
/// <reference types="node" />
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { afterEach, expect, it } from 'vitest'
import type { TFunction } from 'i18next'
import type { ManagedSkill } from './types'
import SkillIssueNotice from './SkillIssueNotice'

afterEach(cleanup)
const t = ((key: string) => key) as TFunction
const skill: ManagedSkill = { id: 'one', name: 'one', enabled: true, status: 'ok', source_type: 'local', central_path: '/central', created_at: 0, updated_at: 0, tags: [], targets: [] }

it('opens compact issue details without expanding the card and closes on Escape or outside click', () => {
  const { container } = render(<SkillIssueNotice compact skill={{ ...skill, status: 'error', source_error: 'sourceMissing' }} tools={[]} t={t} />)
  const trigger = screen.getByRole('button', { name: 'deviceSync.viewIssueReason' })
  expect(screen.queryByRole('dialog')).toBeNull()
  fireEvent.click(trigger)
  const popup = screen.getByRole('dialog')
  expect(container.contains(popup)).toBe(false)
  expect(popup.textContent).toContain('deviceSync.issueReason.sourceMissing')
  fireEvent.keyDown(document, { key: 'Escape' })
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(document.activeElement).toBe(trigger)
  fireEvent.click(trigger)
  fireEvent.pointerDown(document.body)
  expect(screen.queryByRole('dialog')).toBeNull()
})

it('removes an open compact warning when the issue recovers', () => {
  const { rerender } = render(<SkillIssueNotice compact skill={{ ...skill, status: 'error' }} tools={[]} t={t} />)
  fireEvent.click(screen.getByRole('button'))
  rerender(<SkillIssueNotice compact skill={skill} tools={[]} t={t} />)
  expect(screen.queryByRole('dialog')).toBeNull()
  expect(screen.queryByRole('button')).toBeNull()
})

it('shows a reason entry for a failed tool even if it is not installed', () => {
  render(<SkillIssueNotice skill={{ ...skill, targets: [{ tool: 'custom-test', scope: 'global', status: 'error', mode: 'copy', target_path: '/tool', last_error: 'private diagnostic' }] }} tools={[]} t={t} />)
  expect(screen.getByText('deviceSync.viewIssueReason')).toBeTruthy()
  expect(screen.getByText('custom-test')).toBeTruthy()
  expect(screen.queryByText('private diagnostic')).toBeNull()
})

it('does not mark disabled Skills or healthy Skills as current issues', () => {
  const { container, rerender } = render(<SkillIssueNotice skill={skill} tools={[]} t={t} />)
  expect(container.childElementCount).toBe(0)
  rerender(<SkillIssueNotice skill={{ ...skill, enabled: false, status: 'error' }} tools={[]} t={t} />)
  expect(container.childElementCount).toBe(0)
})

it('does not consume the detail workspace height when the source is missing', () => {
  const appCss = readFileSync(resolve(process.cwd(), 'src/App.css'), 'utf8')
  render(
    <>
      <style>{appCss}</style>
      <div className="detail-view">
        <SkillIssueNotice
          skill={{ ...skill, status: 'error', source_error: 'sourceMissing' }}
          tools={[]}
          t={t}
        />
        <div data-testid="detail-content">detail content</div>
      </div>
    </>,
  )

  const notice = screen.getByText('deviceSync.viewIssueReason').closest('details')
  expect(notice).not.toBeNull()
  expect(getComputedStyle(notice!).marginBottom).toBe('10px')
  expect(getComputedStyle(notice!).flexBasis).toBe('auto')
  expect(screen.getByTestId('detail-content')).toBeTruthy()
})
