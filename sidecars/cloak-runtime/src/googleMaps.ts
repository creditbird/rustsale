import type { Page } from 'playwright-core';

export type GoogleMapsPageState =
  | 'ready'
  | 'empty_results'
  | 'captcha'
  | 'consent_required'
  | 'rate_limited'
  | 'selector_changed';

export interface GoogleMapsPageSnapshot {
  url: string;
  title?: string;
  bodyText?: string;
  hasFeed?: boolean;
  hasPlaceHeading?: boolean;
  hasConsentDialog?: boolean;
}

export function classifyGoogleMapsPageState(
  snapshot: GoogleMapsPageSnapshot,
): GoogleMapsPageState {
  const url = snapshot.url.toLowerCase();
  const body = (snapshot.bodyText ?? '').toLowerCase();
  if (
    url.includes('/sorry/') ||
    body.includes('unusual traffic') ||
    body.includes('not a robot') ||
    body.includes('recaptcha')
  ) {
    return 'captcha';
  }
  if (
    url.includes('consent.google.') ||
    snapshot.hasConsentDialog ||
    body.includes('before you continue to google')
  ) {
    return 'consent_required';
  }
  if (
    body.includes('too many requests') ||
    body.includes('rate limit') ||
    body.includes('try again later')
  ) {
    return 'rate_limited';
  }
  if (
    body.includes('no results found') ||
    body.includes('không tìm thấy kết quả') ||
    body.includes('couldn\'t find') ||
    body.includes('google maps can\'t find')
  ) {
    return 'empty_results';
  }
  if (snapshot.hasFeed || snapshot.hasPlaceHeading) {
    return 'ready';
  }
  return 'selector_changed';
}

export async function inspectGoogleMapsPage(page: Page): Promise<{
  state: GoogleMapsPageState;
  url: string;
  title: string;
}> {
  const snapshot = await page.evaluate(() => ({
    url: window.location.href,
    title: document.title,
    bodyText: document.body?.innerText.slice(0, 20_000) ?? '',
    hasFeed: Boolean(document.querySelector('div[role="feed"]')),
    hasPlaceHeading: Boolean(document.querySelector('h1, [role="main"] h1')),
    hasConsentDialog: Boolean(
      document.querySelector('form[action*="consent"], [aria-label*="consent" i]'),
    ),
  }));
  return {
    state: classifyGoogleMapsPageState(snapshot),
    url: snapshot.url,
    title: snapshot.title,
  };
}
