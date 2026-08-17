import { localeState } from './state.svelte';

/**
 * Maps app locale code to standard BCP 47 locale tag
 */
export function getIntlLocale(): string {
  const current = localeState.current;
  switch (current) {
    case 'vi':
      return 'vi-VN';
    case 'en':
      return 'en-US';
    default:
      return 'vi-VN';
  }
}

/**
 * Format a number to currency with standard Intl formatting
 */
export function formatCurrency(
  value: number | undefined | null,
  currency?: 'VND' | 'USD'
): string {
  const defaultCurrency: 'VND' | 'USD' = localeState.current === 'en' ? 'USD' : 'VND';
  const targetCurrency = currency ?? defaultCurrency;

  if (value === undefined || value === null || isNaN(value)) {
    return '0 ' + (targetCurrency === 'VND' ? '₫' : '$');
  }

  const intlLocale = getIntlLocale();
  return new Intl.NumberFormat(intlLocale, {
    style: 'currency',
    currency: targetCurrency,
    maximumFractionDigits: targetCurrency === 'VND' ? 0 : 2
  }).format(value);
}

/**
 * Format a plain number according to the active locale
 */
export function formatNumber(
  value: number | undefined | null,
  options?: Intl.NumberFormatOptions
): string {
  if (value === undefined || value === null || isNaN(value)) {
    return '0';
  }
  const intlLocale = getIntlLocale();
  return new Intl.NumberFormat(intlLocale, options).format(value);
}

/**
 * Format a Date or date string according to active locale
 */
export function formatDate(
  dateInput: string | Date | number | undefined | null,
  options?: Intl.DateTimeFormatOptions
): string {
  if (dateInput === null || dateInput === undefined || dateInput === '') return '';
  const date = typeof dateInput === 'string' || typeof dateInput === 'number' ? new Date(dateInput) : dateInput;
  if (isNaN(date.getTime())) return '';

  const intlLocale = getIntlLocale();
  const defaultOptions: Intl.DateTimeFormatOptions = {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit'
  };

  return new Intl.DateTimeFormat(intlLocale, options ?? defaultOptions).format(date);
}

/**
 * Format Date with time
 */
export function formatDateTime(
  dateInput: string | Date | number | undefined | null,
  options?: Intl.DateTimeFormatOptions
): string {
  if (dateInput === null || dateInput === undefined || dateInput === '') return '';
  const date = typeof dateInput === 'string' || typeof dateInput === 'number' ? new Date(dateInput) : dateInput;
  if (isNaN(date.getTime())) return '';

  const intlLocale = getIntlLocale();
  const defaultOptions: Intl.DateTimeFormatOptions = {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit'
  };

  return new Intl.DateTimeFormat(intlLocale, options ?? defaultOptions).format(date);
}

/**
 * Format relative time (e.g. "5 phút trước" / "5 minutes ago")
 */
export function formatRelativeTime(
  dateInput: string | Date | number | undefined | null
): string {
  if (dateInput === null || dateInput === undefined || dateInput === '') return '';
  const date = typeof dateInput === 'string' || typeof dateInput === 'number' ? new Date(dateInput) : dateInput;
  if (isNaN(date.getTime())) return '';

  const now = Date.now();
  const diffInSeconds = Math.round((date.getTime() - now) / 1000);
  const diffInMinutes = Math.round(diffInSeconds / 60);
  const diffInHours = Math.round(diffInMinutes / 60);
  const diffInDays = Math.round(diffInHours / 24);

  const intlLocale = getIntlLocale();
  const rtf = new Intl.RelativeTimeFormat(intlLocale, { numeric: 'auto' });

  if (Math.abs(diffInSeconds) < 60) {
    return rtf.format(diffInSeconds, 'second');
  }
  if (Math.abs(diffInMinutes) < 60) {
    return rtf.format(diffInMinutes, 'minute');
  }
  if (Math.abs(diffInHours) < 24) {
    return rtf.format(diffInHours, 'hour');
  }
  if (Math.abs(diffInDays) < 30) {
    return rtf.format(diffInDays, 'day');
  }

  return formatDate(date);
}
