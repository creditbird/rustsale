/// <reference types="bun-types" />
import { render, screen, fireEvent } from '@testing-library/svelte';
import Sidebar from '../Sidebar.svelte';
import { expect, test } from 'bun:test';

test('renders Sidebar with CRM logo and options', () => {
  render(Sidebar, { props: { activeTab: 'dashboard' } });
  expect(screen.getByText('RustSale')).toBeTruthy();
  expect(screen.getByText('CRM & Outreach Pro')).toBeTruthy();
  expect(screen.getByText('Dashboard')).toBeTruthy();
  expect(screen.getByText('Conversations')).toBeTruthy();
  expect(screen.getByText('Email Outreach')).toBeTruthy();
  expect(screen.getByText('Settings')).toBeTruthy();
});

test('updates activeTab on navigation item click', async () => {
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
  let opened = false;
  render(Sidebar, {
    props: {
      activeTab: 'dashboard',
      onOpenLicense: () => { opened = true; }
    }
  });

  const accountCard = screen.getByTitle('Tài khoản & Quản lý Bản quyền');
  expect(accountCard).toBeTruthy();
  expect(screen.getByText('Admin Workspace')).toBeTruthy();
  expect(screen.getByText('HWID Active')).toBeTruthy();

  await fireEvent.click(accountCard);
  expect(opened).toBe(true);
});

