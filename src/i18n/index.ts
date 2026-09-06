import i18n from 'i18next'
import { initReactI18next } from 'react-i18next'
import { resources } from './resources'

const languageStorageKey = 'skills-language'

const getStoredLanguage = () => {
  if (typeof window === 'undefined') return null
  try {
    const stored = window.localStorage.getItem(languageStorageKey)
    if (stored === 'en' || stored === 'zh' || stored === 'ko') return stored
  } catch {
    // ignore storage failures
  }
  return null
}

void i18n.use(initReactI18next).init({
  resources,
  lng: getStoredLanguage() ?? 'en',
  fallbackLng: 'en',
  interpolation: {
    escapeValue: false,
  },
})

export default i18n
