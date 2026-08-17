import { baseLocale, locales, type Locale } from '$lib/paraglide/runtime';
import * as m from '$lib/paraglide/messages';
import { localeState, getLocale, setLocale } from './state.svelte';
import {
  formatCurrency,
  formatCompactCurrency,
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
  localeState,
  getLocale,
  setLocale,
  formatCurrency,
  formatCompactCurrency,
  formatDate,
  formatDateTime,
  formatNumber,
  formatRelativeTime
};
