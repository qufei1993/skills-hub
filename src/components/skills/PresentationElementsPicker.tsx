import { memo } from 'react'
import type { PresentationElements } from './skillProfile'

type PresentationElementsPickerProps = {
  value: PresentationElements
  onChange: (next: PresentationElements) => void
  disabled?: boolean
  title?: string
}

const OPTIONS: Array<{ key: keyof PresentationElements; label: string }> = [
  { key: 'category', label: '分类' },
  { key: 'zhName', label: '中文名' },
  { key: 'englishName', label: '英文名' },
  { key: 'summary', label: '简介' },
  { key: 'color', label: '颜色(写入元数据)' },
]

const PresentationElementsPicker = ({
  value,
  onChange,
  disabled = false,
  title = '同步/导出时要露出的元素',
}: PresentationElementsPickerProps) => {
  return (
    <div className="bulk-presentation-options">
      <div className="bulk-modal-subtitle">{title}</div>
      {OPTIONS.map(({ key, label }) => (
        <label key={key} className="bulk-presentation-item">
          <input
            type="checkbox"
            checked={value[key]}
            onChange={(event) => onChange({ ...value, [key]: event.target.checked })}
            disabled={disabled}
          />
          <span>{label}</span>
        </label>
      ))}
    </div>
  )
}

export default memo(PresentationElementsPicker)
