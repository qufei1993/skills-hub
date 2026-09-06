// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import type { TFunction } from 'i18next'
import { afterEach, expect, it, vi } from 'vitest'
import ToolSyncNotice from './ToolSyncNotice'

afterEach(cleanup)
const t = ((key: string) => key) as TFunction

it('renders nothing when no tool needs attention', () => {
  const { container } = render(<ToolSyncNotice issues={[]} onOpen={vi.fn()} t={t} />)
  expect(container.childElementCount).toBe(0)
})

it('keeps long lists collapsed and uses tool display names with full-name tooltips', () => {
  const issues = Array.from({ length: 30 }, (_, index) => ({ skill_name: `very-long-skill-name-${index}`, tool: 'custom_hermes_casey' }))
  render(<ToolSyncNotice issues={issues} toolLabels={{ custom_hermes_casey: 'Hermes · Casey' }} onOpen={vi.fn()} t={t} />)
  expect(screen.queryByRole('table')).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: 'deviceSync.showToolIssues' }))
  expect(screen.getAllByRole('row')).toHaveLength(31)
  expect(screen.queryByText('custom_hermes_casey')).toBeNull()
  expect(screen.getAllByTitle('Hermes · Casey')).toHaveLength(30)
  expect(screen.getByTitle('very-long-skill-name-29')).toBeTruthy()
})
