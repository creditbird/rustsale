import {
  baseLocale,
  locales,
  overwriteGetLocale,
  isLocale
} from '$lib/paraglide/runtime';

/** @typedef {import('$lib/paraglide/runtime').Locale} Locale */

const STORAGE_KEY = 'rustsale_locale';

/**
 * @returns {Locale}
 */
function getInitialLocale() {
  if (typeof window !== 'undefined') {
    try {
      const saved = localStorage.getItem(STORAGE_KEY);
      if (saved && isLocale(saved)) {
        return /** @type {Locale} */ (saved);
      }
      const browserLang = navigator.language?.split('-')[0];
      if (browserLang && isLocale(browserLang)) {
        return /** @type {Locale} */ (browserLang);
      }
    } catch {
      // Ignore storage errors in restricted contexts
    }
  }
  return 'vi';
}

export class LocaleStore {
  /** @type {Locale} */
  current = $state(getInitialLocale());

  constructor() {
    overwriteGetLocale(() => this.current);
    if (typeof document !== 'undefined') {
      document.documentElement.lang = this.current;
    }
  }

  /**
   * @param {Locale} newLocale
   */
  setLocale(newLocale) {
    if (!isLocale(newLocale)) {
      console.warn(`[i18n] Invalid locale provided: ${newLocale}`);
      return;
    }
    this.current = newLocale;
    if (typeof window !== 'undefined') {
      try {
        localStorage.setItem(STORAGE_KEY, newLocale);
      } catch (err) {
        console.warn('[i18n] Could not save locale preference to localStorage', err);
      }
      if (typeof document !== 'undefined') {
        document.documentElement.lang = newLocale;
      }
    }
  }

  /**
   * @returns {Locale}
   */
  getLocale() {
    return this.current;
  }
}

export const localeState = new LocaleStore();
export const getLocale = () => localeState.getLocale();
/**
 * @param {Locale} locale
 */
export const setLocale = (locale) => localeState.setLocale(locale);
