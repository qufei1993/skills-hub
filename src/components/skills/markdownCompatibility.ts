type RegExpFactory = (pattern: string) => RegExp

export function supportsRegExpLookbehind(
  createRegExp: RegExpFactory = (pattern) => new RegExp(pattern),
): boolean {
  try {
    return createRegExp('(?<=a)b').test('ab')
  } catch {
    return false
  }
}
