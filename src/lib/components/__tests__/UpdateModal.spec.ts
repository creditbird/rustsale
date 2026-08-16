/// <reference types="bun-types" />
import { render, screen, cleanup } from '@testing-library/svelte';
import UpdateModal from '../UpdateModal.svelte';
import { updaterService } from '$lib/services/updaterService';
import { expect, test, beforeEach, afterEach, describe } from 'bun:test';

describe('UpdateModal component', () => {
  beforeEach(() => {
    updaterService.setMockAdapter(null);
    updaterService.setMockState({
      status: 'idle',
      currentVersion: '0.1.0',
      availableVersion: null,
      releaseNotes: null,
      releaseDate: null,
      progress: { downloaded: 0, total: 0, percentage: 0 },
      errorMessage: null,
      isModalOpen: false,
    });
  });

  afterEach(() => {
    cleanup();
  });

  test('does not render dialog content when isModalOpen is false', () => {
    render(UpdateModal);
    expect(document.body.textContent).not.toContain('New Software Update');
  });

  test('renders update details when modal is open and update available', async () => {
    updaterService.setMockState({
      status: 'available',
      currentVersion: '0.1.0',
      availableVersion: '0.2.0',
      releaseNotes: 'Fixed Google Maps scraper and sync issue.',
      isModalOpen: true,
    });

    render(UpdateModal);

    expect(document.body.textContent).toContain('New Software Update');
    expect(document.body.textContent).toContain('v0.2.0');
    expect(document.body.textContent).toContain('Fixed Google Maps scraper and sync issue.');
    expect(document.body.textContent).toContain('Update Now');
  });

  test('shows downloading progress when status is downloading', async () => {
    updaterService.setMockState({
      status: 'downloading',
      currentVersion: '0.1.0',
      availableVersion: '0.2.0',
      progress: { downloaded: 52428800, total: 104857600, percentage: 50 },
      isModalOpen: true,
    });

    render(UpdateModal);

    expect(document.body.textContent).toContain('Downloading update...');
    expect(document.body.textContent).toContain('50%');
  });
});
