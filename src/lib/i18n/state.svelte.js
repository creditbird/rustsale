import {
  overwriteGetLocale,
  isLocale
} from '$lib/paraglide/runtime';

/** @typedef {import('$lib/paraglide/runtime').Locale} Locale */

const STORAGE_KEY = 'rustsale_locale';
/** Default fallback UI locale determined by product target market (Vietnamese CRM) */
const DEFAULT_PRODUCT_LOCALE = 'vi';

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
  return DEFAULT_PRODUCT_LOCALE;
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

/** @typedef {'VND' | 'USD'} Currency */

const CURRENCY_STORAGE_KEY = 'rustsale_base_currency';
const EXCHANGE_RATE_STORAGE_KEY = 'rustsale_usd_vnd_rate';
const DEFAULT_BASE_CURRENCY = 'VND';
export const DEFAULT_USD_VND_RATE = 25400;

function getInitialBaseCurrency() {
  if (typeof window !== 'undefined') {
    try {
      const saved = localStorage.getItem(CURRENCY_STORAGE_KEY);
      if (saved === 'VND' || saved === 'USD') {
        return /** @type {Currency} */ (saved);
      }
    } catch {
      // Ignore
    }
  }
  return DEFAULT_BASE_CURRENCY;
}

function getInitialExchangeRate() {
  if (typeof window !== 'undefined') {
    try {
      const saved = localStorage.getItem(EXCHANGE_RATE_STORAGE_KEY);
      if (saved) {
        const num = parseFloat(saved);
        if (!isNaN(num) && num > 0) {
          return num;
        }
      }
    } catch {
      // Ignore
    }
  }
  return DEFAULT_USD_VND_RATE;
}

export class CurrencyStore {
  /** @type {Currency} */
  baseCurrency = $state(getInitialBaseCurrency());
  /** @type {number} */
  exchangeRate = $state(getInitialExchangeRate());
  /** @type {boolean} */
  isHydrated = $state(false);
  /** @type {Promise<void>} */
  ready;
  /** @type {Promise<void>} */
  _mutationQueue = Promise.resolve();
  /** @type {number} */
  _mutationSeq = 0;

  constructor() {
    this.ready = this.syncFromBackend();
  }

  async syncFromBackend() {
    if (typeof window !== 'undefined') {
      const currentSeq = this._mutationSeq;
      try {
        const { invoke } = await import('@tauri-apps/api/core');
        const settings = await invoke('get_currency_settings');
        // Do not overwrite if user mutated settings while backend read was in-flight
        if (this._mutationSeq !== currentSeq) {
          return;
        }
        if (settings && (settings.base_currency === 'VND' || settings.base_currency === 'USD')) {
          this.baseCurrency = settings.base_currency;
          try {
            localStorage.setItem(CURRENCY_STORAGE_KEY, settings.base_currency);
          } catch {}
        }
        if (settings && typeof settings.exchange_rate === 'number' && settings.exchange_rate > 0) {
          this.exchangeRate = settings.exchange_rate;
          try {
            localStorage.setItem(EXCHANGE_RATE_STORAGE_KEY, String(settings.exchange_rate));
          } catch {}
        }
      } catch {
        // Fallback to localStorage in test/web environments
      } finally {
        this.isHydrated = true;
      }
    } else {
      this.isHydrated = true;
    }
  }

  async ensureReady() {
    if (this.ready) {
      try {
        await this.ready;
      } catch {}
    }
  }

  /**
   * @param {() => Promise<void>} fn
   * @returns {Promise<void>}
   */
  async _enqueueMutation(fn) {
    await this.ensureReady();
    const next = this._mutationQueue.then(async () => {
      await fn();
    });
    this._mutationQueue = next.catch(() => {});
    return next;
  }

  /**
   * @param {Currency} baseCurrency
   * @param {number} exchangeRate
   */
  async syncToBackend(baseCurrency, exchangeRate) {
    if (typeof window !== 'undefined') {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('update_currency_settings', {
        baseCurrency,
        exchangeRate
      });
    }
  }

  /**
   * @param {Currency} newCurrency
   * @returns {Promise<void>}
   */
  async setBaseCurrency(newCurrency) {
    if (newCurrency !== 'VND' && newCurrency !== 'USD') {
      console.warn(`[i18n] Invalid currency provided: ${newCurrency}`);
      throw new Error(`Invalid currency: ${newCurrency}`);
    }
    return this._enqueueMutation(async () => {
      const previousCurrency = this.baseCurrency;
      this._mutationSeq++;
      this.baseCurrency = newCurrency;
      if (typeof window !== 'undefined') {
        try {
          localStorage.setItem(CURRENCY_STORAGE_KEY, newCurrency);
        } catch (err) {
          console.warn('[i18n] Could not save base currency to localStorage', err);
        }
      }
      try {
        await this.syncToBackend(newCurrency, this.exchangeRate);
      } catch (err) {
        // Rollback on failure
        this.baseCurrency = previousCurrency;
        if (typeof window !== 'undefined') {
          try {
            localStorage.setItem(CURRENCY_STORAGE_KEY, previousCurrency);
          } catch {}
        }
        throw err;
      }
    });
  }

  /**
   * @param {number} rate
   * @returns {Promise<void>}
   */
  async setExchangeRate(rate) {
    if (typeof rate !== 'number' || isNaN(rate) || rate <= 0) {
      console.warn(`[i18n] Invalid exchange rate provided: ${rate}`);
      throw new Error(`Invalid exchange rate: ${rate}`);
    }
    return this._enqueueMutation(async () => {
      const previousRate = this.exchangeRate;
      this._mutationSeq++;
      this.exchangeRate = rate;
      if (typeof window !== 'undefined') {
        try {
          localStorage.setItem(EXCHANGE_RATE_STORAGE_KEY, String(rate));
        } catch (err) {
          console.warn('[i18n] Could not save exchange rate to localStorage', err);
        }
      }
      try {
        await this.syncToBackend(this.baseCurrency, rate);
      } catch (err) {
        // Rollback on failure
        this.exchangeRate = previousRate;
        if (typeof window !== 'undefined') {
          try {
            localStorage.setItem(EXCHANGE_RATE_STORAGE_KEY, String(previousRate));
          } catch {}
        }
        throw err;
      }
    });
  }

  async resetExchangeRate() {
    return this.setExchangeRate(DEFAULT_USD_VND_RATE);
  }

  getBaseCurrency() {
    return this.baseCurrency;
  }

  getExchangeRate() {
    return this.exchangeRate;
  }
}

export const currencyState = new CurrencyStore();
export const getBaseCurrency = () => currencyState.getBaseCurrency();
/**
 * @param {Currency} currency
 */
export const setBaseCurrency = (currency) => currencyState.setBaseCurrency(currency);
export const getExchangeRate = () => currencyState.getExchangeRate();
/**
 * @param {number} rate
 */
export const setExchangeRate = (rate) => currencyState.setExchangeRate(rate);
