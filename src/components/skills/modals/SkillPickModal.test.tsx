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

afterEach(cleanup)

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
