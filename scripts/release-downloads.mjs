export function renderDownloads({ tag, assets, language }) {
  if (!/^v\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(tag) || !['en', 'zh'].includes(language)) {
    throw new Error('Invalid download table version or language')
  }
  const zh = language === 'zh'
  const installers = [
    ['macOS-aarch64', 'macOS', zh ? 'Apple 芯片（M1 / M2 / M3 等）' : 'Apple silicon (M1 / M2 / M3, etc.)', ['.dmg']],
    ['macOS-x86_64', 'macOS', zh ? 'Intel 芯片' : 'Intel processor', ['.dmg']],
    ['Windows-x64', 'Windows', zh ? 'Intel / AMD 64 位电脑' : 'Intel / AMD 64-bit PCs', ['.exe']],
    ['Linux-x64', 'Linux', zh ? 'Intel / AMD 64 位电脑' : 'Intel / AMD 64-bit PCs', ['.deb', '.AppImage']],
    ['Linux-arm64', 'Linux', zh ? 'ARM64 电脑' : 'ARM64 computers', ['.deb', '.AppImage']],
  ].map(([platform, system, computer, extensions]) => ({
    system, computer,
    links: extensions.filter(extension => assets.includes(`Skills-Hub-${tag}-${platform}${extension}`)).map(extension =>
      `[${zh ? '下载' : 'Download'} ${extension}](https://github.com/qufei1993/skills-hub/releases/download/${tag}/Skills-Hub-${tag}-${platform}${extension})`),
  })).filter(row => row.links.length)
  if (!installers.length) throw new Error('No desktop installers available')
  const rows = installers.map(({ system, computer, links }) => {
    const minimum = system === 'Linux'
      ? (zh ? 'Ubuntu 24.04 或兼容发行版（glibc ≥ 2.39）' : 'Ubuntu 24.04 or compatible distribution (glibc ≥ 2.39)')
      : '—'
    return `| ${system} | ${computer} | ${minimum} | ${links.join(' · ')} |`
  })
  return [
    zh ? '### 下载安装' : '### Downloads', '',
    zh ? '| 系统 | 适用电脑 | 最低环境 | 安装包 |' : '| System | Computer | Minimum environment | Installer |',
    '| --- | --- | --- | --- |', ...rows, '',
    zh ? '桌面用户只需下载安装包，CLI 会在启用 AI 管理时自动下载。' : 'Desktop users only need the installer. The CLI downloads automatically when you enable AI management.',
  ].join('\n')
}

export function prependDownloads(section, options) {
  const lines = section.split('\n')
  const start = lines.findIndex(line => /^### (Downloads|下载安装)\s*$/.test(line))
  if (start >= 0) {
    const next = lines.findIndex((line, index) => index > start && /^### /.test(line))
    lines.splice(start, (next < 0 ? lines.length : next) - start)
  }
  return renderDownloads(options) + '\n\n' + lines.join('\n').trim()
}
