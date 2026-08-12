/// <reference types="bun-types" />
import { afterEach, beforeEach, expect, mock, test } from 'bun:test';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';

import type {
  AdministrativeAreaSummary,
  AdministrativeCountry,
  AdministrativeCountryStatus
} from '$lib/services/administrativeCatalogService';

const invocations: Array<{ command: string; args?: any }> = [];
let loadFails = false;
let loadedCountryCodes = new Set<string>();

class TestResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
}

globalThis.ResizeObserver = TestResizeObserver as typeof ResizeObserver;

const syntheticCountryCodes = Array.from({ length: 26 * 26 }, (_, index) =>
  `${String.fromCharCode(65 + Math.floor(index / 26))}${String.fromCharCode(65 + (index % 26))}`
)
  .filter((code) => !['DE', 'KR', 'VN'].includes(code))
  .slice(0, 249);

const countries: AdministrativeCountry[] = [
  {
    code: 'VN',
    iso3: 'VNM',
    name: 'Vietnam',
    capital: 'Hanoi',
    continent_code: 'AS',
    languages: ['vi'],
    geoname_id: 1_562_822
  },
  {
    code: 'KR',
    iso3: 'KOR',
    name: 'South Korea',
    capital: 'Seoul',
    continent_code: 'AS',
    languages: ['ko'],
    geoname_id: 1_836_840
  },
  {
    code: 'DE',
    iso3: 'DEU',
    name: 'Germany',
    capital: 'Berlin',
    continent_code: 'EU',
    languages: ['de'],
    geoname_id: 2_900_000
  },
  ...syntheticCountryCodes.map((code, index) => ({
    code,
    iso3: `X${code}`,
    name: `Country ${String(index).padStart(3, '0')}`,
    capital: '',
    continent_code: 'AN',
    languages: [],
    geoname_id: 10_000 + index
  }))
];

const roots: AdministrativeAreaSummary[] = [
  area(100, null, 'Alpha Province', 'Alpha Province, Vietnam', true),
  area(101, null, 'Extra Province', 'Extra Province, Vietnam', false)
];
const children: AdministrativeAreaSummary[] = [
  area(200, 100, 'Alpha District', 'Alpha District, Alpha Province, Vietnam', true, 'VN', 2)
];
const grandchildren: AdministrativeAreaSummary[] = [
  area(300, 200, 'Alpha Commune', 'Alpha Commune, Alpha District, Alpha Province, Vietnam', true, 'VN', 3)
];
const greatGrandchildren: AdministrativeAreaSummary[] = [
  area(400, 300, 'Alpha Ward', 'Alpha Ward, Alpha Commune, Alpha District, Alpha Province, Vietnam', true, 'VN', 4)
];
const fifthLevel: AdministrativeAreaSummary[] = [
  area(500, 400, 'Alpha Hamlet', 'Alpha Hamlet, Alpha Ward, Alpha Commune, Alpha District, Alpha Province, Vietnam', false, 'VN', 5)
];
const searchResults: AdministrativeAreaSummary[] = [
  area(300, null, 'Seoul', 'Seoul, South Korea', false, 'KR')
];

function area(
  geonameId: number,
  parentId: number | null,
  name: string,
  canonicalPath: string,
  hasChildren: boolean,
  countryCode = 'VN',
  administrativeLevel = parentId === null ? 1 : 2
): AdministrativeAreaSummary {
  return {
    geoname_id: geonameId,
    country_code: countryCode,
    parent_id: parentId,
    name,
    ascii_name: name,
    feature_code: `ADM${administrativeLevel}`,
    administrative_level: administrativeLevel,
    kind: 'administrative',
    latitude: 10,
    longitude: 20,
    population: 0,
    canonical_path: canonicalPath,
    has_children: hasChildren
  };
}

function status(
  state: AdministrativeCountryStatus['state'],
  error: string | null = null,
  countryCode = 'VN'
): AdministrativeCountryStatus {
  return {
    country_code: countryCode,
    state,
    fetched_at: state === 'ready' ? '2026-08-12T06:00:00Z' : null,
    source_updated_at: state === 'ready' ? '2026-08-11' : null,
    record_count: state === 'ready' ? 3 : 0,
    source_url: state === 'ready' ? 'https://download.geonames.org/export/dump/VN.zip' : null,
    last_error: error,
    bbox: state === 'ready' ? [10, 12, 20, 22] : null
  };
}

mock.module('@tauri-apps/api/core', () => ({
  invoke: async (command: string, args?: any) => {
    invocations.push({ command, args });
    switch (command) {
      case 'list_administrative_countries':
        return countries;
      case 'get_administrative_country_status':
        return status(
          loadedCountryCodes.has(args?.countryCode) ? 'ready' : 'not_loaded',
          null,
          args?.countryCode
        );
      case 'load_administrative_country':
        if (loadFails) return status('failed', 'network unavailable', args?.countryCode);
        loadedCountryCodes.add(args?.countryCode);
        return status('ready', null, args?.countryCode);
      case 'list_administrative_areas':
        switch (args?.parentId) {
          case 100:
            return children;
          case 200:
            return grandchildren;
          case 300:
            return greatGrandchildren;
          case 400:
            return fifthLevel;
          default:
            return roots;
        }
      case 'search_administrative_areas':
        return searchResults;
      default:
        throw new Error(`Unexpected command ${command}`);
    }
  }
}));

import AdministrativeAreaSelector from '../AdministrativeAreaSelector.svelte';

beforeEach(() => {
  invocations.length = 0;
  loadFails = false;
  loadedCountryCodes = new Set<string>();
});

afterEach(() => {
  cleanup();
});

test('lists the complete country catalog and lazily downloads the selected country', async () => {
  render(AdministrativeAreaSelector);

  expect(await screen.findByText('252 countries and territories')).toBeTruthy();
  expect(await screen.findByText('Alpha Province')).toBeTruthy();
  expect(
    screen.getByText(
      'Location data © GeoNames, licensed under CC BY 4.0. Fetched: 2026-08-12T06:00:00Z. Source updated: 2026-08-11.'
    )
  ).toBeTruthy();
  expect(invocations).toContainEqual({
    command: 'load_administrative_country',
    args: { countryCode: 'VN', forceRefresh: false }
  });
});

test('restores region-first country selection and exposes the full administrative cascade', async () => {
  render(AdministrativeAreaSelector);

  expect(await screen.findByLabelText('Region / Khu vực')).toBeTruthy();
  expect(screen.getByLabelText('Country / Quốc gia')).toBeTruthy();
  expect(screen.getByTestId('administrative-level-1')).toBeTruthy();
  expect(screen.getByText('Provinces / States / Cities')).toBeTruthy();

  await fireEvent.click(await screen.findByRole('button', { name: 'Browse Alpha Province' }));
  expect(await screen.findByTestId('administrative-level-2')).toBeTruthy();
  expect(screen.getByText('Districts / Counties / Cities')).toBeTruthy();

  await fireEvent.click(await screen.findByRole('button', { name: 'Browse Alpha District' }));
  expect(await screen.findByTestId('administrative-level-3')).toBeTruthy();
  expect(screen.getByText('Communes / Wards / Municipalities')).toBeTruthy();

  await fireEvent.click(await screen.findByRole('button', { name: 'Browse Alpha Commune' }));
  expect(await screen.findByTestId('administrative-level-4')).toBeTruthy();
  await fireEvent.click(await screen.findByRole('button', { name: 'Browse Alpha Ward' }));
  expect(await screen.findByTestId('administrative-level-5')).toBeTruthy();
  expect(screen.getByText('Villages / Localities')).toBeTruthy();
});

test('filters countries by region and preserves select-all/clear controls per level', async () => {
  render(AdministrativeAreaSelector);

  const regionTrigger = await screen.findByLabelText('Region / Khu vực');
  await fireEvent.keyDown(regionTrigger!, { key: 'Enter', code: 'Enter' });
  await fireEvent.pointerUp(await screen.findByRole('option', { name: /Europe/ }), {
    pointerType: 'mouse'
  });

  await waitFor(() => {
    expect(invocations).toContainEqual({
      command: 'load_administrative_country',
      args: { countryCode: 'DE', forceRefresh: false }
    });
  });
  expect(screen.getByText('Germany')).toBeTruthy();

  const regionReset = screen.getByLabelText('Region / Khu vực');
  await fireEvent.keyDown(regionReset!, { key: 'Home' });
  await fireEvent.keyDown(regionReset!, { key: 'Enter', code: 'Enter' });
  await fireEvent.pointerUp(await screen.findByRole('option', { name: /Asia/ }), {
    pointerType: 'mouse'
  });

  await waitFor(() => {
    expect(invocations).toContainEqual({
      command: 'load_administrative_country',
      args: { countryCode: 'VN', forceRefresh: false }
    });
  });
  await fireEvent.click(screen.getByRole('button', { name: 'Select all Provinces / States / Cities' }));
  expect(screen.getByText('2/500 selected')).toBeTruthy();
  await fireEvent.click(screen.getByRole('button', { name: 'Clear Provinces / States / Cities' }));
  expect(screen.getByText('0/500 selected')).toBeTruthy();
});

test('navigates hierarchy and emits selected canonical areas', async () => {
  const selections: AdministrativeAreaSummary[][] = [];
  render(AdministrativeAreaSelector, {
    props: { onSelectionChange: (areas: AdministrativeAreaSummary[]) => selections.push(areas) }
  });

  await fireEvent.click(await screen.findByRole('button', { name: 'Browse Alpha Province' }));
  await fireEvent.click(await screen.findByRole('checkbox', { name: 'Select Alpha District' }));

  expect(screen.getAllByText('Alpha District, Alpha Province, Vietnam')).toHaveLength(2);
  expect(selections.at(-1)?.map((item) => item.canonical_path)).toEqual([
    'Alpha District, Alpha Province, Vietnam'
  ]);
});

test('clears stale selections and loads the newly selected country by ISO code', async () => {
  loadedCountryCodes.add('VN');
  const selections: AdministrativeAreaSummary[][] = [];
  const countriesSeen: AdministrativeCountry[] = [];
  render(AdministrativeAreaSelector, {
    props: {
      selectedAreas: [children[0]],
      onSelectionChange: (areas: AdministrativeAreaSummary[]) => selections.push(areas),
      onCountryChange: (country: AdministrativeCountry) => countriesSeen.push(country)
    }
  });

  await screen.findByText('252 countries and territories');
  const countryTrigger = document.getElementById('administrative-country');
  expect(countryTrigger).not.toBeNull();
  await fireEvent.keyDown(countryTrigger!, { key: 'Enter', code: 'Enter' });
  await fireEvent.pointerUp(await screen.findByRole('option', { name: 'South Korea (KR)' }), {
    pointerType: 'mouse'
  });

  await waitFor(() => {
    expect(invocations).toContainEqual({
      command: 'get_administrative_country_status',
      args: { countryCode: 'KR' }
    });
    expect(invocations).toContainEqual({
      command: 'list_administrative_areas',
      args: { countryCode: 'KR', parentId: null, limit: 500 }
    });
  });
  expect(selections.at(-1)).toEqual([]);
  expect(countriesSeen.at(-1)?.code).toBe('KR');
});

test('searches a loaded country and selects a search result', async () => {
  const selections: AdministrativeAreaSummary[][] = [];
  render(AdministrativeAreaSelector, {
    props: { onSelectionChange: (areas: AdministrativeAreaSummary[]) => selections.push(areas) }
  });

  const input = await screen.findByRole('textbox', { name: 'Search administrative areas' });
  await fireEvent.input(input, { target: { value: 'Seoul' } });
  await fireEvent.click(screen.getByRole('button', { name: 'Search catalog' }));
  await fireEvent.click(await screen.findByRole('checkbox', { name: 'Select Seoul' }));

  expect(invocations).toContainEqual({
    command: 'search_administrative_areas',
    args: { countryCode: 'VN', query: 'Seoul', limit: 100 }
  });
  expect(selections.at(-1)?.[0].canonical_path).toBe('Seoul, South Korea');
});

test('force refreshes the country catalog on demand', async () => {
  render(AdministrativeAreaSelector);

  await fireEvent.click(await screen.findByRole('button', { name: 'Refresh country data' }));

  await waitFor(() => {
    expect(invocations).toContainEqual({
      command: 'load_administrative_country',
      args: { countryCode: 'VN', forceRefresh: true }
    });
  });
});

test('shows download errors while preserving free-form scraping guidance', async () => {
  loadFails = true;
  render(AdministrativeAreaSelector);

  expect((await screen.findByRole('alert')).textContent).toContain('network unavailable');
  expect(screen.getByText(/free-form location/i)).toBeTruthy();
  expect(screen.getByText(/GeoNames.*CC BY 4.0/i)).toBeTruthy();
});

test('enforces the existing 500-target campaign limit', async () => {
  loadedCountryCodes.add('VN');
  const selectedAreas = Array.from({ length: 500 }, (_, index) =>
    area(1_000 + index, null, `Selected ${index}`, `Selected ${index}, Vietnam`, false)
  );
  const selections: AdministrativeAreaSummary[][] = [];
  render(AdministrativeAreaSelector, {
    props: {
      selectedAreas,
      onSelectionChange: (areas: AdministrativeAreaSummary[]) => selections.push(areas)
    }
  });

  await fireEvent.click(await screen.findByRole('checkbox', { name: 'Select Extra Province' }));

  expect((await screen.findByRole('alert')).textContent).toContain('Maximum 500');
  expect(selections).toHaveLength(0);
});
