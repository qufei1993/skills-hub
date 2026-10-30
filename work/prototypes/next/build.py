#!/usr/bin/env python3
"""Assemble the next design without modifying the fixed development reference."""
from pathlib import Path
import argparse
import hashlib
import json
import re

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
BASE = ROOT / 'docs/releases/v1.1.0/prototypes/skills-hub.html'
MANIFEST = BASE.parent.parent / 'baseline-manifest.json'


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--serve', action='store_true', help='Copy only the next draft to the existing local preview directory')
    args = parser.parse_args()
    expected = json.loads(MANIFEST.read_text())['files']['prototypes/skills-hub.html']
    if hashlib.sha256(BASE.read_bytes()).hexdigest() != expected:
        raise SystemExit('Fixed design reference has changed; review the new baseline before rebuilding.')
    html = BASE.read_text()
    html = re.sub(r'<title>.*?</title>', '<title>Skills Hub · MCP 未来功能讨论</title>', html, count=1)
    # The product retains the existing shell. The outer bar identifies the simulation.
    start = html.index('<div class="review-title">')
    end = html.index('</div>', start)
    html = html[:start] + '<div class="review-title"><strong>Skills Hub</strong><span>MCP 未来功能讨论</span><small>当前不实现 · 模拟交互 · 不修改本机配置</small>' + html[end:]
    html = html.replace('data-action="changeIndex">本次改动', 'data-action="mcp-design">本次改动')
    html = html.replace('data-action="startTour">体验主流程', 'data-action="mcp-tour">体验主流程')
    html = html.replace('<div class="app-window" data-ui>', '<div class="app-window">', 1)
    css = (HERE / 'mcp.css').read_text()
    js = (HERE / 'mcp.js').read_text()
    html = html.replace('</style></head>', '\n/* Next design: MCP workspace */\n' + css + '\n</style></head>', 1)
    marker = '\nbootPrototype();'
    if html.count(marker) != 1:
        raise SystemExit('Expected exactly one prototype boot call.')
    html = html.replace(marker, '\n' + js + '\nif (!location.hash) U.page = "mcp";\nbootPrototype();', 1)
    (HERE / 'index.html').write_text(html)
    if args.serve:
        preview = Path('/tmp/skills-hub-local-preview/next')
        if not preview.is_dir():
            raise SystemExit('Existing next preview directory is missing; no server or root preview was changed.')
        (preview / 'index.html').write_text(html)
    print('Built next/index.html' + (' and updated the existing /next/ preview.' if args.serve else '.'))


if __name__ == '__main__':
    main()
