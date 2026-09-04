import type { CSSProperties } from 'react'
import ampLogo from '@lobehub/icons-static-svg/icons/amp-color.svg'
import antigravityLogo from '@lobehub/icons-static-svg/icons/antigravity-color.svg'
import claudeCodeLogo from '@lobehub/icons-static-svg/icons/claudecode-color.svg'
import clineLogo from '@lobehub/icons-static-svg/icons/cline.svg'
import codeBuddyLogo from '@lobehub/icons-static-svg/icons/codebuddy-color.svg'
import codexLogo from '@lobehub/icons-static-svg/icons/codex-color.svg'
import copilotLogo from '@lobehub/icons-static-svg/icons/copilot-color.svg'
import cursorLogo from '@lobehub/icons-static-svg/icons/cursor.svg'
import deepSeekLogo from '@lobehub/icons-static-svg/icons/deepseek-color.svg'
import geminiCliLogo from '@lobehub/icons-static-svg/icons/geminicli-color.svg'
import gooseLogo from '@lobehub/icons-static-svg/icons/goose.svg'
import hermesAgentLogo from '@lobehub/icons-static-svg/icons/hermesagent.svg'
import junieLogo from '@lobehub/icons-static-svg/icons/junie-color.svg'
import kiloCodeLogo from '@lobehub/icons-static-svg/icons/kilocode.svg'
import kiroLogo from '@lobehub/icons-static-svg/icons/kiro-color.svg'
import mcpLogo from '@lobehub/icons-static-svg/icons/mcp.svg'
import mistralLogo from '@lobehub/icons-static-svg/icons/mistral-color.svg'
import openClawLogo from '@lobehub/icons-static-svg/icons/openclaw-color.svg'
import openCodeLogo from '@lobehub/icons-static-svg/icons/opencode.svg'
import openHandsLogo from '@lobehub/icons-static-svg/icons/openhands-color.svg'
import qoderLogo from '@lobehub/icons-static-svg/icons/qoder-color.svg'
import qwenLogo from '@lobehub/icons-static-svg/icons/qwen-color.svg'
import rooCodeLogo from '@lobehub/icons-static-svg/icons/roocode.svg'
import traeLogo from '@lobehub/icons-static-svg/icons/trae-color.svg'
import windsurfLogo from '@lobehub/icons-static-svg/icons/windsurf.svg'
import zencoderLogo from '@lobehub/icons-static-svg/icons/zencoder-color.svg'
import kimiCodeLogo from '../../assets/kimi-code-icon.png'

type ToolIconProps = {
  toolKey: string
  label: string
  avatar?: string | null
  className?: string
}

const logoByToolKey: Record<string, string> = {
  amp: ampLogo,
  antigravity: antigravityLogo,
  claude_code: claudeCodeLogo,
  clawdbot: openClawLogo,
  cline: clineLogo,
  codebuddy: codeBuddyLogo,
  codex: codexLogo,
  gemini_cli: geminiCliLogo,
  github_copilot: copilotLogo,
  cursor: cursorLogo,
  deepseek_harness: deepSeekLogo,
  goose: gooseLogo,
  hermes_agent: hermesAgentLogo,
  junie: junieLogo,
  kilo_code: kiloCodeLogo,
  kimi_cli: kimiCodeLogo,
  kiro_cli: kiroLogo,
  mcpjam: mcpLogo,
  mistral_vibe: mistralLogo,
  moltbot: openClawLogo,
  openclaw: openClawLogo,
  opencode: openCodeLogo,
  openhands: openHandsLogo,
  qoder: qoderLogo,
  qoderwork: qoderLogo,
  qwen_code: qwenLogo,
  roo_code: rooCodeLogo,
  trae: traeLogo,
  trae_cn: traeLogo,
  windsurf: windsurfLogo,
  zencoder: zencoderLogo,
}

const fallbackColorByToolKey: Record<string, string> = {
  augment: '#6d5dfc',
  copaw: '#f26b3a',
  codewhale: '#1686d9',
  workbuddy: '#5b65f5',
  command_code: '#171717',
  continue: '#f59e0b',
  crush: '#e83e8c',
  iflow_cli: '#5f62e9',
  kode: '#6f50e8',
  mux: '#1677ff',
  openclaude: '#d97757',
  pi: '#292524',
  neovate: '#15a37d',
  pochi: '#ff6b6b',
  adal: '#0f766e',
  droid: '#6366f1',
}

const getInitials = (label: string) =>
  label
    .split(/[\s_-]+/)
    .filter(Boolean)
    .map((part) => part[0])
    .join('')
    .slice(0, 2)
    .toUpperCase()

const ToolIcon = ({ toolKey, label, avatar, className = '' }: ToolIconProps) => {
  const logo = avatar || logoByToolKey[toolKey]
  const style = {
    '--tool-brand': fallbackColorByToolKey[toolKey] ?? '#64748b',
  } as CSSProperties

  return (
    <span
      className={`tool-brand-icon${logo ? ' has-logo' : ' fallback'}${toolKey === 'kimi_cli' && !avatar ? ' kimi-code-icon' : ''}${avatar ? ' custom-avatar' : ''}${className ? ` ${className}` : ''}`}
      style={style}
      aria-hidden="true"
    >
      {logo ? <img src={logo} alt="" draggable={false} /> : getInitials(label)}
    </span>
  )
}

export default ToolIcon
