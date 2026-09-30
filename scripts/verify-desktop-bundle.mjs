import { readdirSync, readFileSync } from 'node:fs'
import path from 'node:path'
import { pathToFileURL } from 'node:url'
export function verifyDesktopBundle(directory) {
  const entries=readdirSync(directory,{withFileTypes:true})
  for(const entry of entries) {
    const filename=path.join(directory,entry.name)
    if (/^skillshub-cli(?:[-.]|$)/i.test(entry.name)) throw new Error('DESKTOP_CONTAINS_CLI')
    if(entry.isDirectory()) verifyDesktopBundle(filename)
    else if(entry.name.endsWith('.nsi') && /skillshub-cli/i.test(readFileSync(filename,'utf8'))) throw new Error('DESKTOP_INSTALLER_CONTAINS_CLI')
  }
}
if(process.argv[1] && import.meta.url===pathToFileURL(path.resolve(process.argv[1])).href) {
 try {verifyDesktopBundle(process.argv[2]);console.log('Desktop package contains no CLI executable.')}
 catch {console.error('DESKTOP_BUNDLE_VERIFICATION_FAILED');process.exitCode=1}
}
