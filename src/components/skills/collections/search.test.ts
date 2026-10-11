import { test, assert } from 'vitest';
import { matchesCollection } from './search';
import type { CollectionIndexItem } from './types';

const record = {
  title: '工程合集', description: '编写可靠应用', category: 'development',
  tags: ['代码质量'], authorLabel: 'Example Author',
  searchText: 'UniqueInternalSkillSummary',
  localizations: { en: { title: 'Engineering collection', description: 'Reliable applications', tags: ['Quality'], searchText: 'LegacySkillText' } },
} as CollectionIndexItem;
test('matches only bilingual collection metadata even when a cached index contains skill text', () => {
  for (const query of ['工程', '可靠', 'development', '代码质量', 'example author', 'engineering', 'reliable', 'quality', '  QUALITY  ', '']) assert.equal(matchesCollection(record, query), true);
  for (const query of ['UniqueInternalSkillSummary', 'LegacySkillText', 'missing']) assert.equal(matchesCollection(record, query), false);
});
