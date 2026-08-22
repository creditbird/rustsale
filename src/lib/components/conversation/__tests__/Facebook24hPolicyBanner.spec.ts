/// <reference types="bun-types" />
import { afterEach, describe, expect, test } from 'bun:test';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import {
  FACEBOOK_MESSAGE_TAGS,
  evaluateFacebookWindow,
  formatRemainingHm,
  validateOutboundPolicy,
  WINDOW_MS
} from '../facebookPolicyGate';
import Facebook24hPolicyBanner from '../Facebook24hPolicyBanner.svelte';

const HOUR = 60 * 60 * 1000;
const NOW = 1_800_000_000_000; // fixed "now"

afterEach(() => cleanup());

describe('evaluateFacebookWindow', () => {
  test('missing last inbound time fails closed as outside window', () => {
    const s = evaluateFacebookWindow(null, NOW);
    expect(s.outsideWindow).toBe(true);
    expect(s.remainingMs).toBe(0);
  });

  test('unparseable last inbound time fails closed as outside window', () => {
    const s = evaluateFacebookWindow('not-a-date', NOW);
    expect(s.outsideWindow).toBe(true);
    expect(s.remainingMs).toBe(0);
  });

  test('recent interaction is inside the window with positive remaining', () => {
    const s = evaluateFacebookWindow(NOW - HOUR, NOW);
    expect(s.outsideWindow).toBe(false);
    expect(s.remainingMs).toBe(23 * HOUR);
  });

  test('exactly 24h boundary closes the window', () => {
    const s = evaluateFacebookWindow(NOW - WINDOW_MS, NOW);
    expect(s.outsideWindow).toBe(true);
    expect(s.remainingMs).toBe(0);
  });

  test('beyond 24h is outside the window', () => {
    const s = evaluateFacebookWindow(NOW - WINDOW_MS - 1, NOW);
    expect(s.outsideWindow).toBe(true);
    expect(s.remainingMs).toBe(0);
  });
});

describe('validateOutboundPolicy', () => {
  test('inside window allows sending without a tag', () => {
    const v = validateOutboundPolicy(false, null);
    expect(v.allowed).toBe(true);
    expect(v.reason).toBeNull();
  });

  test('outside window without a tag blocks sending with a reason', () => {
    const v = validateOutboundPolicy(true, null);
    expect(v.allowed).toBe(false);
    expect(v.reason).toBeTruthy();
  });

  test('outside window with a selected HUMAN_AGENT tag allows sending', () => {
    const v = validateOutboundPolicy(true, 'HUMAN_AGENT');
    expect(v.allowed).toBe(true);
    expect(v.reason).toBeNull();
  });
});

describe('formatRemainingHm', () => {
  test('formats hours and padded minutes', () => {
    expect(formatRemainingHm(23 * HOUR)).toBe('23h 00m');
    expect(formatRemainingHm(2 * HOUR + 5 * 60 * 1000)).toBe('2h 05m');
    expect(formatRemainingHm(0)).toBe('0h 00m');
  });
});

describe('FACEBOOK_MESSAGE_TAGS', () => {
  test('contains the four Meta-approved CSKH tags', () => {
    const values = FACEBOOK_MESSAGE_TAGS.map((t) => t.value);
    expect(values).toContain('CONFIRMED_EVENT_UPDATE');
    expect(values).toContain('POST_PURCHASE_UPDATE');
    expect(values).toContain('ACCOUNT_UPDATE');
    expect(values).toContain('HUMAN_AGENT');
  });
});

describe('Facebook24hPolicyBanner component', () => {
  test('inside window shows remaining-time indicator', () => {
    render(Facebook24hPolicyBanner, {
      props: { lastInboundAt: new Date(NOW - HOUR).toISOString(), nowMs: NOW }
    });
    expect(screen.getByText(/cửa sổ 24h của Meta/)).toBeTruthy();
    expect(screen.getByText(/23h 00m/)).toBeTruthy();
  });

  test('outside window shows warning and tag selector with all four options', async () => {
    render(Facebook24hPolicyBanner, {
      props: { lastInboundAt: new Date(NOW - 25 * HOUR).toISOString(), nowMs: NOW }
    });
    expect(screen.getByText(/quá 24h/i)).toBeTruthy();

    const select = screen.getByRole('combobox') as HTMLSelectElement;
    expect(select.value).toBe('');
    for (const t of FACEBOOK_MESSAGE_TAGS) {
      const option = Array.from(select.options).find((o) => o.value === t.value);
      expect(option).toBeTruthy();
    }
  });

  test('selecting a tag reports it through onTagChange and clears the block reason', async () => {
    const holder: { reported: string | null } = { reported: null };
    render(Facebook24hPolicyBanner, {
      props: {
        lastInboundAt: new Date(NOW - 25 * HOUR).toISOString(),
        nowMs: NOW,
        onTagChange: (tag: string | null) => {
          holder.reported = tag;
        }
      }
    });
    const select = screen.getByRole('combobox') as HTMLSelectElement;
    select.value = 'POST_PURCHASE_UPDATE';
    await fireEvent.change(select);
    expect(holder.reported).toBe('POST_PURCHASE_UPDATE');
  });
});
