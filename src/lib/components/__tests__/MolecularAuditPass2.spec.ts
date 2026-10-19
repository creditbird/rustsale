/// <reference types="bun-types" />
import { afterEach, beforeEach, describe, expect, mock, test } from 'bun:test';

if (typeof Element !== 'undefined' && !Element.prototype.animate) {
  Element.prototype.animate = function () {
    return {
      cancel: () => {},
      finish: () => {},
      pause: () => {},
      play: () => {},
      reverse: () => {},
      addEventListener: () => {},
      removeEventListener: () => {},
      dispatchEvent: () => {},
      onfinish: null,
      oncancel: null,
      onremove: null,
      finished: Promise.resolve(),
      ready: Promise.resolve(),
      currentTime: 0,
      effect: null,
      id: '',
      timeline: null,
      playbackRate: 1,
      playState: 'finished',
      pending: false
    } as any;
  };
}

import { spyOn } from 'bun:test';
import { cleanup, fireEvent, render, screen } from '@testing-library/svelte';
import * as tauriEvent from '@tauri-apps/api/event';
import * as crmService from '$lib/services/crmService';
import VoiceRecorderBar from '../conversation/VoiceRecorderBar.svelte';
import LeadDetails from '../LeadDetails.svelte';
import ZaloAccountsSettings from '../settings/ZaloAccountsSettings.svelte';
import { setLocale } from '$lib/i18n';

describe('Molecular Audit Pass 2 - Red/Green Regression Suite', () => {
  beforeEach(() => {
    setLocale('en');
  });

  afterEach(() => {
    cleanup();
  });

  test('VoiceRecorderBar stops microphone tracks and does not start MediaRecorder if unmounted while getUserMedia is pending', async () => {
    let resolveMedia!: (stream: MediaStream) => void;
    const stopTrackMock = mock(() => {});
    const recorderStartMock = mock(() => {});

    const fakeStream = {
      getTracks: () => [{ stop: stopTrackMock }]
    } as unknown as MediaStream;

    const originalMediaDevices = navigator.mediaDevices;
    const originalMediaRecorder = globalThis.MediaRecorder;

    Object.defineProperty(navigator, 'mediaDevices', {
      configurable: true,
      value: {
        getUserMedia: () =>
          new Promise<MediaStream>((resolve) => {
            resolveMedia = resolve;
          })
      }
    });

    class FakeMediaRecorder {
      state = 'inactive';
      stream: MediaStream;
      ondataavailable: ((e: { data: Blob }) => void) | null = null;
      onstop: (() => void) | null = null;
      constructor(stream: MediaStream) {
        this.stream = stream;
      }
      start() {
        this.state = 'recording';
        recorderStartMock();
      }
      stop() {
        this.state = 'inactive';
        this.onstop?.();
      }
    }
    globalThis.MediaRecorder = FakeMediaRecorder as unknown as typeof MediaRecorder;

    try {
      const { unmount } = render(VoiceRecorderBar, {
        props: {
          onRecordingComplete: () => {},
          onCancel: () => {}
        }
      });

      // Unmount BEFORE getUserMedia resolves
      unmount();

      // Now resolve getUserMedia
      resolveMedia(fakeStream);
      await new Promise((r) => setTimeout(r, 10));

      expect(stopTrackMock).toHaveBeenCalledTimes(1);
      expect(recorderStartMock).toHaveBeenCalledTimes(0);
    } finally {
      Object.defineProperty(navigator, 'mediaDevices', {
        configurable: true,
        value: originalMediaDevices
      });
      globalThis.MediaRecorder = originalMediaRecorder;
    }
  });

  test('LeadDetails renders clean multi-space initials and does not render broken mailto link for No Email leads', async () => {
    const scrapedLead = {
      id: 999,
      name: '  Alpha   Beta ',
      company: 'Beta Co',
      status: 'New',
      value: 0,
      last_activity: '2026-10-19',
      owner: 'Google Maps',
      email: 'No Email',
      phone: '0901234567'
    };

    const getLeadSpy = spyOn(crmService, 'getLeadDetails').mockResolvedValue(scrapedLead);
    const getActivitiesSpy = spyOn(crmService, 'getActivities').mockResolvedValue([]);
    const getDealsSpy = spyOn(crmService, 'getDeals').mockResolvedValue([]);

    try {
      const { container } = render(LeadDetails, {
        props: {
          leadId: 999,
          initialLead: scrapedLead,
          onClose: () => {},
          onRefresh: () => {}
        }
      });

      // Should compute "AB" from "  Alpha   Beta "
      expect(screen.getByText('AB')).toBeTruthy();

      // Should NOT render a broken <a href="mailto:No Email"> link
      const brokenMailto = container.querySelector('a[href="mailto:No Email"]');
      expect(Boolean(brokenMailto)).toBe(false);

      // Clicking Edit should normalize 'No Email' to '' so type="email" input is not pre-filled with invalid 'No Email'
      const editBtn = screen.getByRole('button', { name: /Edit/i });
      await fireEvent.click(editBtn);

      const emailInput = container.querySelector('#edit-email') as HTMLInputElement | null;
      expect(emailInput).toBeTruthy();
      expect(emailInput?.value).toBe('');
      expect(emailInput?.required).toBe(false);
    } finally {
      getLeadSpy.mockRestore();
      getActivitiesSpy.mockRestore();
      getDealsSpy.mockRestore();
    }
  });

  test('ZaloAccountsSettings unsubscribes in-flight QR listener immediately if unmounted before listen() resolves', async () => {
    const pendingListenResolvers: Array<() => void> = [];
    let unlistenCallCount = 0;

    const listenSpy = spyOn(tauriEvent, 'listen').mockImplementation(
      () =>
        new Promise<() => void>((resolve) => {
          pendingListenResolvers.push(() => {
            resolve(() => {
              unlistenCallCount += 1;
            });
          });
        })
    );

    try {
      const { unmount } = render(ZaloAccountsSettings);

      const addBtn = screen.getByRole('button', { name: /Add Zalo account/i });
      await fireEvent.click(addBtn);

      expect(pendingListenResolvers.length).toBeGreaterThan(0);

      // Unmount while first listen('zalo-qr-code') is still pending
      unmount();

      // Resolve the pending listen() call after unmount
      for (const resolveListen of pendingListenResolvers) {
        resolveListen();
      }
      await new Promise((r) => setTimeout(r, 10));

      expect(unlistenCallCount).toBe(1);
    } finally {
      listenSpy.mockRestore();
    }
  });
});
