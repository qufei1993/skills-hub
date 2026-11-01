import { readFileSync } from 'node:fs'

const knownStatuses = new Set(['Accepted', 'Invalid', 'Rejected', 'In Progress'])
let result = { status: 'InvalidResponse', id: null }
try {
  const response = JSON.parse(readFileSync(0, 'utf8'))
  if (knownStatuses.has(response?.status)
    && typeof response.id === 'string'
    && /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(response.id)) {
    result = { status: response.status, id: response.id }
  }
} catch { /* Never echo the untrusted response or parser error. */ }
process.stdout.write(`${JSON.stringify(result)}\n`)
process.exitCode = result.status === 'Accepted' ? 0 : 1
