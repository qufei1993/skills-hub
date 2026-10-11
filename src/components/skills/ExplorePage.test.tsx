// @vitest-environment jsdom
import { useState } from 'react'
import { afterEach, expect, it, vi } from 'vitest'
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import i18n from '../../i18n'
import ExplorePage from './ExplorePage'
import index from './collections/fixtures/index.json'
import detail from './collections/fixtures/engineering.json'
import { validateDetail, validateIndex } from './collections/validation'
import { createInstallPlan } from './collections/install-plan'
afterEach(() => { cleanup(); vi.unstubAllGlobals() })
it('shows website collections by default, opens details and reuses the collection installation manifest', async () => {
  await i18n.changeLanguage('zh')
  const install = vi.fn()
  const calls: string[] = []
  let indexRequests = 0
  render(<ExplorePage invoke={async <T,>(command: string, args?: Record<string, unknown>) => {
    calls.push(command)
    if (command === 'get_website_collections') {
      if (!args?.id) indexRequests++
      return (args?.id ? detail : index) as T
    }
    if (command === 'cache_website_collections') return undefined as T
    expect(command).toBe('validate_collection_manifest')
    expect(args?.link).toBeUndefined()
    return args?.manifest as T
  }} onInstallCollection={install} exploreFilter="" searchResults={[]} searchLoading={false} managedSkills={[]} loading={false} onExploreFilterChange={() => {}} onInstallSkill={() => {}} onOpenManualAdd={() => {}} t={i18n.t} />)
  await screen.findByText('Skills for Real Engineers')
  const card = screen.getByText('Skills for Real Engineers').closest('article')!
  fireEvent.click(card.querySelector('button')!)
  await screen.findByText('包含的 Skills')
  expect(document.activeElement).toBe(screen.getByRole('button', { name: '返回探索' }))
  fireEvent.click(screen.getByRole('button', { name: '选择安装 · 31 个 Skills' }))
  await waitFor(() => expect(install).toHaveBeenCalledOnce())
  expect(install.mock.calls[0][0].skills).toHaveLength(31)
  expect(calls).not.toContain('get_featured_skills')
  expect(screen.queryByRole('button', { name: 'Skills 搜索' })).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: '返回探索' }))
  await waitFor(() => expect(document.activeElement).toBe(screen.getByRole('button', { name: '查看 Skills for Real Engineers' })))
  expect(indexRequests).toBe(1)
})
it('validates website data and rejects unpublished collections before generating an install plan', () => {
  expect(validateIndex(index).length).toBeGreaterThan(0)
  expect(() => validateDetail({ ...detail, review: { ...detail.review, status: 'pending' } })).toThrow()
  const collection = validateDetail(detail)
  const plan = createInstallPlan(collection, collection.skills.map(skill => skill.id))
  expect(plan.conflicts).toEqual([])
  expect(plan.manifest.sources.every(source => /^[a-f0-9]{40}$/.test(source.ref))).toBe(true)
})

it('keeps discovery useful before searching, searches in place, and restores collections when cleared', async () => {
  await i18n.changeLanguage('zh')
  const manual = vi.fn()
  const install = vi.fn()
  function Page() {
    const [query, setQuery] = useState('')
    return <ExplorePage invoke={async <T,>(command: string) => (command === 'get_website_collections' ? index : undefined) as T}
      onInstallCollection={() => {}} exploreFilter={query}
      searchResults={[{ name: 'frontend-design', source: 'anthropics/skills', source_url: 'https://github.com/anthropics/skills', installs: 123 }]}
      searchLoading={false} managedSkills={[]} loading={false}
      onExploreFilterChange={setQuery} onInstallSkill={install} onOpenManualAdd={manual} t={i18n.t} />
  }
  render(<Page />)
  await screen.findByText('Skills for Real Engineers')
  expect(screen.queryByRole('tablist')).toBeNull()
  fireEvent.click(screen.getByRole('button', { name: '从 Git 安装' }))
  expect(manual).toHaveBeenCalledWith('git')
  fireEvent.click(screen.getByRole('button', { name: '从本地导入' }))
  expect(manual).toHaveBeenCalledWith('local')
  fireEvent.click(screen.getByRole('button', { name: '前端设计' }))
  expect((screen.getByRole('searchbox') as HTMLInputElement).value).toBe('frontend')
  await screen.findByText('frontend-design')
  fireEvent.click(screen.getByRole('button', { name: '安装' }))
  expect(install).toHaveBeenCalledWith('https://github.com/anthropics/skills', 'frontend-design')
  fireEvent.click(screen.getByRole('button', { name: '清空搜索' }))
  expect((screen.getByRole('searchbox') as HTMLInputElement).value).toBe('')
  expect(await screen.findByText('Skills for Real Engineers')).toBeTruthy()
  expect(screen.queryByText('frontend-design')).toBeNull()
})

it('renders collections in batches, resets on search and preserves the batch on returning from details', async () => {
  let intersect: (visible: boolean) => void = () => {}
  vi.stubGlobal('IntersectionObserver', class {
    constructor(callback: IntersectionObserverCallback) {
      intersect = visible => callback([{ isIntersecting: visible } as IntersectionObserverEntry], this as unknown as IntersectionObserver)
    }
    observe() {}
    disconnect() {}
  })
  await i18n.changeLanguage('zh')
  const many = { ...index, collections: Array.from({ length: 301 }, (_, n) => ({
    ...index.collections.find(item => item.id === detail.id)!,
    id: n === 0 ? detail.id : `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`,
    slug: `collection-${n}`,
  })) }
  const invoke = async <T,>(command: string, args?: Record<string, unknown>) =>
    (command === 'get_website_collections' ? (args?.id ? detail : many) : undefined) as T
  function Page() {
    const [query, setQuery] = useState('')
    return <ExplorePage invoke={invoke} onInstallCollection={() => {}} exploreFilter={query}
      searchResults={[]} searchLoading={false} managedSkills={[]} loading={false}
      onExploreFilterChange={setQuery} onInstallSkill={() => {}} onOpenManualAdd={() => {}} t={i18n.t} />
  }
  render(<Page />)
  const cards = () => screen.getAllByRole('button', { name: /^查看 / })
  await waitFor(() => expect(cards()).toHaveLength(12))
  act(() => { intersect(true); intersect(true) })
  expect(cards()).toHaveLength(24)
  act(() => intersect(false))
  expect(cards()).toHaveLength(24)
  fireEvent.click(cards()[0])
  await screen.findByText('包含的 Skills')
  fireEvent.click(screen.getByRole('button', { name: '返回探索' }))
  await waitFor(() => expect(cards()).toHaveLength(24))
  act(() => intersect(true))
  for (let batch = 0; batch < 30; batch++) act(() => intersect(true))
  expect(cards()).toHaveLength(301)
  expect(screen.queryByRole('button', { name: '加载更多合集' })).toBeNull()
  fireEvent.change(screen.getByRole('searchbox'), { target: { value: 'engineers' } })
  expect(cards()).toHaveLength(12)
  fireEvent.click(screen.getByRole('button', { name: '清空搜索' }))
  expect(cards()).toHaveLength(12)
})
