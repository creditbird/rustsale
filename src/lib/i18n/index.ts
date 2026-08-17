import { baseLocale, locales, type Locale } from '$lib/paraglide/runtime';
import * as m from '$lib/paraglide/messages';
import {
  localeState,
  getLocale,
  setLocale,
  currencyState,
  getBaseCurrency,
  setBaseCurrency,
  getExchangeRate,
  setExchangeRate,
  DEFAULT_USD_VND_RATE,
  type Currency
} from './state.svelte';
import {
  formatCurrency,
  formatCompactCurrency,
  convertCurrency,
  formatDate,
  formatDateTime,
  formatNumber,
  formatRelativeTime
} from './formatters';

export {
  m,
  locales,
  baseLocale,
  type Locale,
  type Currency,
  localeState,
  getLocale,
  setLocale,
  currencyState,
  getBaseCurrency,
  setBaseCurrency,
  getExchangeRate,
  setExchangeRate,
  DEFAULT_USD_VND_RATE,
  convertCurrency,
  formatCurrency,
  formatCompactCurrency,
  formatDate,
  formatDateTime,
  formatNumber,
  formatRelativeTime
};
