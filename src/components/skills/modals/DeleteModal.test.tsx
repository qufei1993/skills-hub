// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import i18n from '../../../i18n'
import DeleteModal from './DeleteModal'
import BulkDeleteModal from './BulkDeleteModal'

afterEach(cleanup)

describe('Skill deletion confirmations', () => {
  it.each(['single', 'bulk'])('%s describes app recovery and requires explicit confirmation', (kind) => {
    const onConfirm = vi.fn()
    const onRequestClose = vi.fn()
    const props = { open: true, loading: false, onConfirm, onRequestClose, t: i18n.getFixedT('zh') }
    if (kind === 'single') render(<DeleteModal {...props} skillName="demo" />)
    else render(<BulkDeleteModal {...props} skillNames={['demo', 'second']} />)
    expect(screen.getByRole('dialog')).toBeTruthy()
    expect(screen.queryByText(/系统回收站/)).toBeNull()
    expect(screen.getByText(/删除后会移入 Skills Hub 本机回收站/)).toBeTruthy()
    expect(onConfirm).not.toHaveBeenCalled()
    const cancel = screen.getByRole('button', { name: '取消' })
    expect(document.activeElement).toBe(cancel)
    fireEvent.keyDown(document, { key: 'Escape' })
    expect(onRequestClose).toHaveBeenCalledOnce()
    fireEvent.click(screen.getByRole('button', { name: '删除' }))
    expect(onConfirm).toHaveBeenCalledOnce()
  })

  it('explains that deleting missing-content records preserves tool paths without a recycle backup', () => {
    const { container } = render(<DeleteModal open loading={false} centralMissing skillName="missing" onRequestClose={vi.fn()} onConfirm={vi.fn()} t={i18n.getFixedT('zh')} />)
    expect(screen.getByText(/只删除管理记录/).textContent).toContain('无法生成回收站备份')
    expect(container.querySelector('.modal-recoverable-delete')).toBeNull()
    expect(screen.queryByText(/文件和本机配置保留 30 天/)).toBeNull()
  })

  it('prevents dismissal while deleting', () => {
    const close = vi.fn()
    const { container } = render(<DeleteModal open loading skillName="demo" onRequestClose={close} onConfirm={vi.fn()} t={i18n.getFixedT('zh')} />)
    fireEvent.keyDown(document, { key: 'Escape' })
    fireEvent.click(container.querySelector('.modal-backdrop')!)
    expect(close).not.toHaveBeenCalled()
  })
})

it('bulk deletion of only missing-content records does not promise a recycle backup', () => {
  const { container } = render(<BulkDeleteModal open loading={false} skillNames={["missing"]} missingCentralCount={1} onRequestClose={vi.fn()} onConfirm={vi.fn()} t={i18n.getFixedT('zh')} />)
  expect(screen.getByText(/只删除管理记录/)).toBeTruthy()
  expect(container.querySelector('.modal-recoverable-delete')).toBeNull()
  expect(container.querySelector('.delete-warning')).toBeNull()
  expect(screen.queryByText(/30 天/)).toBeNull()
})
