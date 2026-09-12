/** Magnet-style board columns from the skills list content width (not the window).
 * ¼ / ⅓ → 1; ⅖ / ½ → 2; ⅔ → 3; fullscreen / left-right → 3–4.
 */
export function skillBoardColumns(boardWidth: number): 1 | 2 | 3 | 4 {
  if (boardWidth < 560) return 1
  if (boardWidth < 900) return 2
  if (boardWidth < 1320) return 3
  return 4
}
