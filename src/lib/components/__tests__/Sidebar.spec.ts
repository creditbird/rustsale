/// <reference types="bun-types" />
import { render, screen } from '@testing-library/svelte';
import Sidebar from '../Sidebar.svelte';
import { expect, test } from 'bun:test';

test('renders Sidebar with CRM logo and options', () => {
  render(Sidebar, { activeTab: 'dashboard' });
  expect(screen.getByText('RustSale Pro')).toBeTruthy();
  expect(screen.getByText('Dashboard')).toBeTruthy();
});
