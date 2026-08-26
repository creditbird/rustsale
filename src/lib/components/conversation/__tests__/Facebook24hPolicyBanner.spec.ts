/// <reference types="bun-types" />
import { afterEach, describe, expect, test } from 'bun:test';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import {
  FACEBOOK_MESSAGE_TAGS,
  evaluateFacebookWindow,
  formatRemainingHm,
  validateOutboundPolicy,
  HUMAN_AGENT_WINDOW_MS,
  WINDOW_MS
} from '../facebookPolicyGate';
import Facebook24hPolicyBanner from '../Facebook24hPolicyBanner.svelte';

const HOUR = 60 * 60 * 1000;
const DAY = 24 * HOUR;
const NOW = 1_800_000_000_000; // fixed "now"

afterEach(() => cleanup());

describe('evaluateFacebookWindow', () => {
  test('missing last inbound time fails closed on both windows', () => {
    const s = evaluateFacebookWindow(null, NOW);
    expect(s.freeWindow).toBe(false);
    expect(s.humanAgentEligible).toBe(false);
    expect(s.remainingMs).toBe(0);
  });

  test('unparseable last inbound time fails closed on both windows', () => {
    const s = evaluateFacebookWindow('not-a-date', NOW);
    expect(s.freeWindow).toBe(false);
    expect(s.humanAgentEligible).toBe(false);
  });

  test('recent interaction: free window open with positive remaining', () => {
    const s = evaluateFacebookWindow(NOW - HOUR, NOW);
    expect(s.freeWindow).toBe(true);
    expect(s.humanAgentEligible).toBe(true);
    expect(s.remainingMs).toBe(23 * HOUR);
  });

  test('exactly 24h boundary closes the free window but keeps HUMAN_AGENT eligible', () => {
    const s = evaluateFacebookWindow(NOW - WINDOW_MS, NOW);
    expect(s.freeWindow).toBe(false);
    expect(s.humanAgentEligible).toBe(true);
    expect(s.remainingMs).toBe(0);
  });

  test('between 24h and 7d only HUMAN_AGENT remains', () => {
    const s = evaluateFacebookWindow(NOW - 3 * DAY, NOW);
    expect(s.freeWindow).toBe(false);
    expect(s.humanAgentEligible).toBe(true);
  });

  test('exactly 7d boundary ends HUMAN_AGENT eligibility', () => {
    const s = evaluateFacebookWindow(NOW - HUMAN_AGENT_WINDOW_MS, NOW);
    expect(s.humanAgentEligible).toBe(false);
  });
});

describe('validateOutboundPolicy', () => {
  test('free window allows sending without a tag', () => {
    const status = evaluateFacebookWindow(NOW - HOUR, NOW);
    const v = validateOutboundPolicy(status, null);
    expect(v.allowed).toBe(true);
    expect(v.reason).toBeNull();
  });

  test('24h-7d window requires exactly the HUMAN_AGENT tag', () => {
    const status = evaluateFacebookWindow(NOW - 2 * DAY, NOW);
    expect(validateOutboundPolicy(status, null).allowed).toBe(false);
    expect(
      validateOutboundPolicy(status, 'CONFIRMED_EVENT_UPDATE' as string).allowed
    ).toBe(false);
    expect(validateOutboundPolicy(status, 'HUMAN_AGENT').allowed).toBe(true);
  });

  test('beyond 7d nothing unlocks sending', () => {
    const status = evaluateFacebookWindow(NOW - 8 * DAY, NOW);
    const v = validateOutboundPolicy(status, 'HUMAN_AGENT');
    expect(v.allowed).toBe(false);
    expect(v.reason).toMatch(/7 (days|ngày)/i);
  });
});

describe('FACEBOOK_MESSAGE_TAGS', () => {
  test('only HUMAN_AGENT is offered (legacy tags retired by Meta)', () => {
    expect(FACEBOOK_MESSAGE_TAGS).toHaveLength(1);
    expect(FACEBOOK_MESSAGE_TAGS[0].value).toBe('HUMAN_AGENT');
  });
});

describe('formatRemainingHm', () => {
  test('formats hours and padded minutes', () => {
    expect(formatRemainingHm(23 * HOUR)).toBe('23h 00m');
    expect(formatRemainingHm(2 * HOUR + 5 * 60 * 1000)).toBe('2h 05m');
    expect(formatRemainingHm(0)).toBe('0h 00m');
  });
});

describe('Facebook24hPolicyBanner component', () => {
  test('free window shows remaining-time indicator', () => {
    render(Facebook24hPolicyBanner, {
      props: { lastInboundAt: new Date(NOW - HOUR).toISOString(), nowMs: NOW }
    });
    expect(screen.getByText(/cửa sổ 24h của Meta|24-hour messaging window/i)).toBeTruthy();
    expect(screen.getByText(/23h 00m/)).toBeTruthy();
  });

  test('24h-7d window shows amber banner with single HUMAN_AGENT option', async () => {
    render(Facebook24hPolicyBanner, {
      props: { lastInboundAt: new Date(NOW - 25 * HOUR).toISOString(), nowMs: NOW }
    });
    expect(screen.getByTestId('facebook-policy-outside')).toBeTruthy();

    const select = screen.getByRole('combobox') as HTMLSelectElement;
    expect(select.value).toBe('');
    expect(select.options.length).toBe(2); // placeholder + HUMAN_AGENT
  });

  test('selecting HUMAN_AGENT reports it through onTagChange', async () => {
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
    select.value = 'HUMAN_AGENT';
    await fireEvent.change(select);
    expect(holder.reported).toBe('HUMAN_AGENT');
  });

  test('beyond 7d renders hard block with no selector', () => {
    render(Facebook24hPolicyBanner, {
      props: { lastInboundAt: new Date(NOW - 8 * DAY).toISOString(), nowMs: NOW }
    });
    expect(screen.getByTestId('facebook-policy-expired')).toBeTruthy();
    expect(screen.queryByRole('combobox')).toBeNull();
  });
});
