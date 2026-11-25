import type { Page } from 'playwright-core';

export type GoogleMapsPageState =
  | 'ready'
  | 'empty_results'
  | 'place_unavailable'
  | 'captcha'
  | 'consent_required'
  | 'rate_limited'
  | 'blank_page'
  | 'selector_changed';

export interface GoogleMapsPageSnapshot {
  url: string;
  title?: string;
  bodyText?: string;
  hasFeed?: boolean;
  hasPlaceHeading?: boolean;
  hasEmptyPlaceHeading?: boolean;
  hasConsentDialog?: boolean;
  hasCaptchaChallenge?: boolean;
}

export function classifyGoogleMapsPageState(
  snapshot: GoogleMapsPageSnapshot,
): GoogleMapsPageState {
  const url = snapshot.url.toLowerCase();
  const body = (snapshot.bodyText ?? '').toLowerCase();
  const isPlaceUrl = url.includes('/maps/place/');

  // Structural signals are checked before body text, and body text is only
  // consulted once the structural signals are absent. Listing and review text
  // routinely contains phrases such as "try again later" or "no results found",
  // and a text-first order let ordinary page content pause an entire job.
  // A challenge can be served as an overlay that leaves the underlying feed DOM
  // intact, so the structural captcha signal has to outrank the feed.
  // 1. Structural signals that definitively indicate a hard block / captcha / consent overlay
  // even if the underlying feed DOM is still mounted in the background.
  if (
    url.includes('/sorry/') ||
    snapshot.hasCaptchaChallenge ||
    body.includes('unusual traffic') ||
    body.includes('not a robot') ||
    body.includes('recaptcha') ||
    body.includes('lưu lượng bất thường') ||
    body.includes('lưu lượng truy cập bất thường') ||
    body.includes('không phải là người máy') ||
    body.includes('xác minh bạn là con người')
  ) {
    return 'captcha';
  }

  if (url.includes('consent.google.') || snapshot.hasConsentDialog) {
    return 'consent_required';
  }

  // 2. An error page can render an h1 (for example "Error") without being a
  // place or a result list, so it must not count as ready through the heading
  // signal alone.
  if (!snapshot.hasFeed && looksLikeErrorPage(snapshot.title ?? '', body)) {
    return 'selector_changed';
  }

  // 3. Structural presence of search results feed or place details heading
  if (snapshot.hasFeed || snapshot.hasPlaceHeading) {
    return 'ready';
  }

  // 4. Fallback text inspection when structural feed/heading is absent
  if (
    body.includes('before you continue to google') ||
    body.includes('trước khi tiếp tục đến google') ||
    body.includes('trước khi bạn tiếp tục đến google') ||
    body.includes('trước khi tiếp tục') ||
    body.includes('chấp nhận tất cả') ||
    body.includes('đồng ý tất cả')
  ) {
    return 'consent_required';
  }

  if (
    body.includes('too many requests') ||
    body.includes('rate limit') ||
    body.includes('try again later') ||
    body.includes('quá nhiều yêu cầu') ||
    body.includes('thử lại sau') ||
    body.includes('vui lòng thử lại sau')
  ) {
    return 'rate_limited';
  }

  if (
    body.includes('no results found') ||
    body.includes('không tìm thấy kết quả') ||
    body.includes('không tìm thấy bất kỳ kết quả') ||
    body.includes('không thể tìm thấy') ||
    body.includes('google maps không thể tìm thấy') ||
    body.includes('google maps không tìm thấy') ||
    body.includes('chúng tôi không tìm thấy') ||
    body.includes('không có kết quả') ||
    body.includes('couldn\'t find') ||
    body.includes('cannot find') ||
    body.includes('google maps can\'t find') ||
    body.includes('google maps cannot find') ||
    body.includes('make sure your search is spelled correctly') ||
    body.includes('đảm bảo rằng tất cả các từ đều đúng chính tả')
  ) {
    return isPlaceUrl ? 'place_unavailable' : 'empty_results';
  }

  // 5. A withdrawn or non-existent /maps/place/ URL renders an empty (hidden)
  // [role="main"] h1 alongside ordinary map chrome buttons instead of a place
  // heading. Report it explicitly so the Rust scraper can apply a short settle
  // window and classify it as PlaceUnavailable without 20 s SelectorChanged polling.
  if (isPlaceUrl && snapshot.hasEmptyPlaceHeading) {
    return 'place_unavailable';
  }

  if (body.trim().length === 0) {
    return 'blank_page';
  }
  return 'selector_changed';
}

function looksLikeErrorPage(title: string, body: string): boolean {
  const text = `${title}\n${body}`.toLowerCase();
  return (
    text.includes('server error') ||
    text.includes('the server encountered an error') ||
    text.includes("500. that's an error") ||
    text.includes('500. that’s an error') ||
    text.includes('this page isn’t working') ||
    text.includes("this page isn't working") ||
    text.includes('service unavailable')
  );
}

export async function inspectGoogleMapsPage(page: Page): Promise<{
  state: GoogleMapsPageState;
  url: string;
  title: string;
}> {
  const snapshot = await page.evaluate(() => {
    const mainHeadings = Array.from(document.querySelectorAll('[role="main"] h1'));
    const hasMapChrome = Boolean(document.querySelector('[data-item-id], [role="main"] button'));
    const hasNonEmptyHeading = mainHeadings.some(
      (heading) => (heading.textContent ?? '').trim().length > 0,
    );
    return {
      url: window.location.href,
      title: document.title,
      bodyText: document.body?.innerText.slice(0, 20_000) ?? '',
      hasFeed: Boolean(document.querySelector('div[role="feed"]')),
      // A place that no longer exists renders an empty (hidden) h1 next to the
      // ordinary map buttons, so the heading only counts when it carries text.
      hasPlaceHeading: hasNonEmptyHeading && hasMapChrome,
      hasEmptyPlaceHeading: mainHeadings.length > 0 && !hasNonEmptyHeading && hasMapChrome,
      hasConsentDialog: Boolean(
        document.querySelector(
          'form[action*="consent"], [role="dialog"] [aria-label*="consent" i], [role="dialog"] [aria-label*="Đồng ý" i], [role="dialog"] [aria-label*="Chấp nhận" i]',
        ),
      ),
      hasCaptchaChallenge: Boolean(
        document.querySelector(
          'iframe[src*="recaptcha"], form#captcha-form, .g-recaptcha, [id*="recaptcha"]',
        ),
      ),
    };
  });
  return {
    state: classifyGoogleMapsPageState(snapshot),
    url: snapshot.url,
    title: snapshot.title,
  };
}
