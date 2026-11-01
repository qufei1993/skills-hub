// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import i18n from '../../i18n'
import ProjectPicker from './ProjectPicker'

beforeEach(async () => { await i18n.changeLanguage('en') })
afterEach(cleanup)
const paths = ['/projects/customer-one/shared', '/projects/customer-two/shared', 'C:\\projects\\windows-app']
function setup(value = '') {
  const onChange = vi.fn()
  const onBrowse = vi.fn()
  render(<><ProjectPicker projects={paths} value={value} onChange={onChange} onBrowse={onBrowse} t={i18n.t} /><button>Outside</button></>)
  const trigger = screen.getByRole('button', { name: i18n.t('workspace110.project') })
  fireEvent.click(trigger)
  return { onChange, onBrowse, trigger, search: screen.getByRole('combobox') }
}
it('distinguishes duplicate names with full paths and searches the parent path', () => {
  const { search, onChange } = setup()
  expect(screen.getAllByRole('option', { name: /shared/ })).toHaveLength(2)
  fireEvent.change(search, { target: { value: 'customer-two' } })
  expect(screen.getAllByRole('option')).toHaveLength(1)
  fireEvent.click(screen.getByRole('option', { name: /customer-two/ }))
  expect(onChange).toHaveBeenCalledWith(paths[1])
  expect(screen.queryByRole('dialog')).toBeNull()
})
it('supports keyboard selection and returns focus to the trigger', () => {
  const { search, onChange, trigger } = setup()
  expect(document.activeElement).toBe(search)
  fireEvent.keyDown(search, { key: 'ArrowDown' })
  expect(document.getElementById(search.getAttribute('aria-activedescendant')!)?.textContent).toContain(paths[0])
  fireEvent.keyDown(search, { key: 'Enter' })
  expect(onChange).toHaveBeenCalledWith(paths[0])
  expect(document.activeElement).toBe(trigger)
})
it('closes on Escape without changing selection and resets search when reopened', () => {
  const { search, onChange, trigger } = setup(paths[1])
  fireEvent.change(search, { target: { value: 'missing' } })
  fireEvent.keyDown(search, { key: 'Escape' })
  expect(onChange).not.toHaveBeenCalled()
  expect(document.activeElement).toBe(trigger)
  fireEvent.click(trigger)
  expect((screen.getByRole('combobox') as HTMLInputElement).value).toBe('')
  expect(screen.getAllByRole('option')).toHaveLength(4)
  expect(document.getElementById(screen.getByRole('combobox').getAttribute('aria-activedescendant')!)?.textContent).toContain(paths[1])
})
it('keeps folder browsing available when search has no results', () => {
  const { search, onBrowse, onChange } = setup()
  fireEvent.change(search, { target: { value: 'missing' } })
  expect(screen.getByRole('status').textContent).toBe(i18n.t('workspace110.noProjectResults'))
  fireEvent.click(screen.getByRole('button', { name: i18n.t('workspace110.chooseProject') }))
  expect(onBrowse).toHaveBeenCalledOnce()
  expect(onChange).not.toHaveBeenCalled()
  expect(screen.queryByRole('dialog')).toBeNull()
})
it('closes on outside interaction or focus leaving and can clear to all projects', () => {
  const { trigger, onChange } = setup(paths[0])
  fireEvent.pointerDown(screen.getByText('Outside'))
  expect(screen.queryByRole('dialog')).toBeNull()
  fireEvent.click(trigger)
  act(() => { screen.getByText('Outside').focus() })
  expect(screen.queryByRole('dialog')).toBeNull()
  fireEvent.click(trigger)
  fireEvent.click(screen.getByRole('option', { name: i18n.t('workspace110.allProjects') }))
  expect(onChange).toHaveBeenCalledWith('')
})

it('does not submit or dismiss while an input method is composing', () => {
  const { search, onChange } = setup()
  fireEvent.keyDown(search, { key: 'Enter', isComposing: true })
  fireEvent.keyDown(search, { key: 'Escape', isComposing: true })
  fireEvent.keyDown(search, { key: 'Enter', keyCode: 229 })
  expect(onChange).not.toHaveBeenCalled()
  expect(screen.getByRole('dialog')).toBeTruthy()
  expect(document.activeElement).toBe(search)
})
