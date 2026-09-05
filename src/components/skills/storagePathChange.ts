import type { StoragePathChangePreview } from './types'

export const requiresStorageMigrationConfirmation = (
  preview: StoragePathChangePreview,
) => preview.skill_count > 0 && preview.current_path !== preview.new_path
