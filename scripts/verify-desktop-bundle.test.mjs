import assert from 'node:assert/strict'
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
const { it } = process.env.VITEST ? await import('vitest') : await import('node:test')
const api=await import('./verify-desktop-bundle.mjs').catch(()=>({}))
it('checks real package files and Windows packaging instructions for a carried CLI', () => {
  assert.equal(typeof api.verifyDesktopBundle,'function')
  const root=mkdtempSync(path.join(tmpdir(),'desktop-bundle-'))
  try {
    mkdirSync(path.join(root,'Skills Hub.app/Contents/MacOS'),{recursive:true})
    writeFileSync(path.join(root,'Skills Hub.app/Contents/MacOS/app'),'app')
    api.verifyDesktopBundle(root)
    const cli=path.join(root,'Skills Hub.app/Contents/MacOS/skillshub-cli')
    writeFileSync(cli,'cli'); assert.throws(()=>api.verifyDesktopBundle(root)); rmSync(cli)
    writeFileSync(path.join(root,'installer.nsi'),'File "binaries/skillshub-cli.exe"'); assert.throws(()=>api.verifyDesktopBundle(root))
  } finally {rmSync(root,{recursive:true,force:true})}
})
