// @vitest-environment jsdom
import { useState } from 'react'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { createInstance } from 'i18next'
import GitPickModal from './GitPickModal'
import LocalPickModal from './LocalPickModal'

const i18n = createInstance()
await i18n.init({
  lng: 'en',
  resources: {
    en: { translation: {
      installSelected: 'Install selected',
      selectAll: 'Select all',
      selectedCount: 'Selected {{selected}}/{{total}}',
    } },
  },
})

const candidates = [
  { name: 'academy-guide', subpath: 'skills/academy-guide', description: 'Learning resources', valid: true },
  { name: 'review', subpath: 'skills/review', description: 'Code review', valid: true },
  { name: 'testing', subpath: 'skills/testing', description: null, valid: true },
]

afterEach(() => { cleanup(); vi.restoreAllMocks() })

describe.each(['git', 'local'] as const)('%s skill picker', (kind) => {
  function setup({ invalid = false, loading = false } = {}) {
    const onInstall = vi.fn()
    const items = invalid
      ? [...candidates, { name: 'broken', subpath: 'skills/broken', valid: false, reason: 'missing_skill_md' }]
      : candidates
    function Picker() {
      const [selected, setSelected] = useState<Record<string, boolean>>(
        Object.fromEntries(items.map((c) => [c.subpath, true])),
      )
      const props = {
        open: true,
        loading,
        onRequestClose: vi.fn(),
        onCancel: vi.fn(),
        onInstall,
        onToggleCandidate: (subpath: string, checked: boolean) =>
          setSelected((prev) => ({ ...prev, [subpath]: checked })),
        t: i18n.t,
      }
      return kind === 'git'
        ? <GitPickModal {...props} gitCandidates={items} gitCandidateSelected={selected} />
        : <LocalPickModal {...props} localCandidates={items} localCandidateSelected={selected} />
    }
    render(<Picker />)
    return {
      onInstall,
      search: (query: string) => fireEvent.change(screen.getByRole('textbox'), { target: { value: query } }),
      install: () => fireEvent.click(screen.getByRole('button', { name: 'Install selected' })),
    }
  }

  it.each([' acad ', 'LEARNING', 'skills/academy'])('submits only the visible selection when searching %s', (query) => {
    const { search, install, onInstall } = setup()
    search(query)
    expect(screen.getByText('Selected 1/1')).toBeTruthy()
    install()
    expect(onInstall).toHaveBeenCalledExactlyOnceWith(['skills/academy-guide'])
  })

  it('does not install hidden selections when no results match', () => {
    const { search, install, onInstall } = setup()
    search('no-matching-skill')
    expect((screen.getByRole('button', { name: 'Install selected' }) as HTMLButtonElement).disabled).toBe(true)
    install()
    expect(onInstall).not.toHaveBeenCalled()
  })

  it('disables installation after deselecting the only visible skill', () => {
    const { search, install, onInstall } = setup()
    search('acad')
    fireEvent.click(screen.getAllByRole('checkbox')[1])
    expect(screen.getByText('Selected 0/1')).toBeTruthy()
    install()
    expect(onInstall).not.toHaveBeenCalled()
  })

  it('preserves individual choices when clearing search', () => {
    const { search, install, onInstall } = setup()
    search('acad')
    fireEvent.click(screen.getByRole('checkbox', { name: 'Select all' }))
    search('')
    expect(screen.getByText('Selected 2/3')).toBeTruthy()
    install()
    expect(onInstall).toHaveBeenCalledExactlyOnceWith(['skills/review', 'skills/testing'])
  })

  it('submits all selected skills without a search', () => {
    const { install, onInstall } = setup()
    install()
    expect(onInstall).toHaveBeenCalledExactlyOnceWith([
      'skills/academy-guide', 'skills/review', 'skills/testing',
    ])
  })

  it('prevents repeated installation while loading', () => {
    const { install, onInstall } = setup({ loading: true })
    install()
    expect(onInstall).not.toHaveBeenCalled()
  })

  if (kind === 'local') {
    it('excludes invalid local candidates even if their selection state is true', () => {
      const { install, onInstall } = setup({ invalid: true })
      expect(screen.getByText('Selected 3/3')).toBeTruthy()
      install()
      expect(onInstall).toHaveBeenCalledExactlyOnceWith([
        'skills/academy-guide', 'skills/review', 'skills/testing',
      ])
    })
  }
})


it('keeps same-source updates selectable and excludes conflicts from select all', () => {
  const onInstall = vi.fn()
  render(<GitPickModal
    open loading={false}
    gitCandidates={[
      { name: 'existing', subpath: 'a', status: 'update' },
      { name: 'new', subpath: 'b', status: 'install' },
      { name: 'conflict', subpath: 'c', status: 'conflict' },
    ]}
    gitCandidateSelected={{ a: true, b: true, c: true }}
    onRequestClose={vi.fn()} onCancel={vi.fn()} onToggleCandidate={vi.fn()}
    onInstall={onInstall} t={i18n.t}
  />)
  expect(screen.getByText('gitInstall.update')).toBeTruthy()
  expect(screen.getByText('gitInstall.conflict')).toBeTruthy()
  const checkboxes = screen.getAllByRole('checkbox') as HTMLInputElement[]
  expect(checkboxes[3].disabled).toBe(true)
  fireEvent.click(screen.getByRole('button', { name: 'gitInstall.submit' }))
  expect(onInstall).toHaveBeenCalledExactlyOnceWith(['a', 'b'])
})


it('expands and collapses overflowing descriptions without enabling a conflicting skill', () => {
  vi.spyOn(HTMLElement.prototype, 'clientHeight', 'get').mockReturnValue(48)
  vi.spyOn(HTMLElement.prototype, 'scrollHeight', 'get').mockReturnValue(160)
  const onToggleCandidate = vi.fn()
  render(<GitPickModal open loading={false}
    gitCandidates={[{ name: 'conflict', subpath: 'a', status: 'conflict', description: 'Long skill description' }]}
    gitCandidateSelected={{ a: false }} onToggleCandidate={onToggleCandidate}
    onRequestClose={vi.fn()} onCancel={vi.fn()} onInstall={vi.fn()} t={i18n.t}
  />)
  const expand = screen.getByRole('button', { name: 'gitInstall.expandDescription' })
  expect(expand.getAttribute('aria-expanded')).toBe('false')
  const contentId = expand.getAttribute('aria-controls')!
  expect(document.getElementById(contentId)?.textContent).toBe('Long skill description')
  fireEvent.click(expand)
  const collapse = screen.getByRole('button', { name: 'gitInstall.collapseDescription' })
  expect(collapse.getAttribute('aria-expanded')).toBe('true')
  expect((screen.getByRole('checkbox', { name: 'conflict' }) as HTMLInputElement).disabled).toBe(true)
  expect(onToggleCandidate).not.toHaveBeenCalled()
  fireEvent.click(collapse)
  expect(screen.getByRole('button', { name: 'gitInstall.expandDescription' }).getAttribute('aria-expanded')).toBe('false')
})

it('only offers description expansion when the text overflows the available width', () => {
  let height = 32
  vi.spyOn(HTMLElement.prototype, 'clientHeight', 'get').mockReturnValue(48)
  vi.spyOn(HTMLElement.prototype, 'scrollHeight', 'get').mockImplementation(() => height)
  render(<GitPickModal open loading={false}
    gitCandidates={[{ name: 'short', subpath: 'a', description: 'Skill description' }]}
    gitCandidateSelected={{ a: true }} onToggleCandidate={vi.fn()}
    onRequestClose={vi.fn()} onCancel={vi.fn()} onInstall={vi.fn()} t={i18n.t}
  />)
  expect(screen.queryByRole('button', { name: 'gitInstall.expandDescription' })).toBeNull()
  height = 120
  fireEvent(window, new Event('resize'))
  expect(screen.getByRole('button', { name: 'gitInstall.expandDescription' })).toBeTruthy()
  height = 32
  fireEvent(window, new Event('resize'))
  expect(screen.queryByRole('button', { name: 'gitInstall.expandDescription' })).toBeNull()
})
