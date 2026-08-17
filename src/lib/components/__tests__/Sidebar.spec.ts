import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import Sidebar from '../Sidebar.svelte';
import { expect, test, beforeEach, afterEach } from 'bun:test';
import { setLocale } from '$lib/i18n';

beforeEach(() => {
  setLocale('en');
});

afterEach(() => {
  cleanup();
});

test('renders Sidebar with CRM logo and options in English', () => {
  setLocale('en');
  render(Sidebar, { props: { activeTab: 'dashboard' } });
  expect(screen.getByText('RustSale')).toBeTruthy();
  expect(screen.getByText('CRM & Outreach Pro')).toBeTruthy();
  expect(screen.getByText('Dashboard')).toBeTruthy();
  expect(screen.getByText('Conversations')).toBeTruthy();
  expect(screen.getByText('Email Outreach')).toBeTruthy();
  expect(screen.getByText('Settings')).toBeTruthy();
});

test('renders Sidebar with Vietnamese options when locale is vi', () => {
  setLocale('vi');
  render(Sidebar, { props: { activeTab: 'dashboard' } });
  expect(screen.getByText('Bảng điều khiển')).toBeTruthy();
  expect(screen.getByText('Hội thoại')).toBeTruthy();
  expect(screen.getByText('Tiếp cận Email')).toBeTruthy();
  expect(screen.getByText('Cài đặt Hệ thống')).toBeTruthy();
});

test('updates activeTab on navigation item click', async () => {
  setLocale('en');
  let activeTab = 'dashboard';
  render(Sidebar, {
    props: {
      get activeTab() { return activeTab; },
      set activeTab(val) { activeTab = val; }
    }
  });

  const conversationButton = screen.getByText('Conversations');
  await fireEvent.click(conversationButton);
  expect(activeTab).toBe('conversation');

  await fireEvent.click(screen.getByText('Settings'));
  expect(activeTab).toBe('settings');
});

test('renders interactive Account & License card in footer and triggers onOpenLicense when clicked', async () => {
  setLocale('en');
  let opened = false;
  render(Sidebar, {
    props: {
      activeTab: 'dashboard',
      onOpenLicense: () => { opened = true; }
    }
  });

  const accountCard = screen.getByTitle('Account & License Management');
  expect(accountCard).toBeTruthy();
  expect(screen.getByText('Admin Workspace')).toBeTruthy();
  expect(screen.getByText('HWID Active')).toBeTruthy();

  await fireEvent.click(accountCard);
  expect(opened).toBe(true);
});
