export type SkillSourceSite = {
  id: string
  url: string
  nameKey: string
  descriptionKey: string
}

export const SKILL_SOURCE_SITES: SkillSourceSite[] = [
  {
    id: 'skillhub',
    url: 'https://skillhub.cn/',
    nameKey: 'moreSkills.sites.skillhub.name',
    descriptionKey: 'moreSkills.sites.skillhub.desc',
  },
  {
    id: 'clawhub',
    url: 'https://clawhub.ai/',
    nameKey: 'moreSkills.sites.clawhub.name',
    descriptionKey: 'moreSkills.sites.clawhub.desc',
  },
]
