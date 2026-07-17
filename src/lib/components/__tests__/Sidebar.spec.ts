/// <reference types="bun-types" />
import { render, screen, fireEvent } from '@testing-library/svelte';
import Sidebar from '../Sidebar.svelte';
import { expect, test } from 'bun:test';

test('renders Sidebar with CRM logo and options', () => {
  render(Sidebar, { props: { activeTab: 'dashboard' } });
  expect(screen.getByText('RustSale Pro')).toBeTruthy();
  expect(screen.getByText('Dashboard')).toBeTruthy();
});

test('updates activeTab on navigation item click', async () => {
  let activeTab = 'dashboard';
  render(Sidebar, {
    props: {
      get activeTab() { return activeTab; },
      set activeTab(val) { activeTab = val; }
    }
  });

  const leadsButton = screen.getByText('Leads & Contacts');
  await fireEvent.click(leadsButton);
  expect(activeTab).toBe('leads');
});
