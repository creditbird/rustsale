import { beforeEach, describe, expect, mock, test } from 'bun:test';

const invocations: Array<{ command: string; args?: unknown }> = [];

mock.module('@tauri-apps/api/core', () => ({
  invoke: async (command: string, args?: unknown) => {
    invocations.push({ command, args });
    return [];
  }
}));

import {
  getAdministrativeCountryStatus,
  listAdministrativeAreas,
  listAdministrativeCountries,
  loadAdministrativeCountry,
  searchAdministrativeAreas
} from '../administrativeCatalogService';

beforeEach(() => {
  invocations.length = 0;
});

describe('administrativeCatalogService', () => {
  test('maps typed calls to exact backend commands and arguments', async () => {
    await listAdministrativeCountries();
    await getAdministrativeCountryStatus('VN');
    await loadAdministrativeCountry('VN');
    await loadAdministrativeCountry('KR', true);
    await listAdministrativeAreas('VN');
    await listAdministrativeAreas('VN', 123, 50);
    await searchAdministrativeAreas('VN', 'Hà Nội');
    await searchAdministrativeAreas('KR', 'Seoul', 25);

    expect(invocations).toEqual([
      { command: 'list_administrative_countries', args: undefined },
      { command: 'get_administrative_country_status', args: { countryCode: 'VN' } },
      {
        command: 'load_administrative_country',
        args: { countryCode: 'VN', forceRefresh: false }
      },
      {
        command: 'load_administrative_country',
        args: { countryCode: 'KR', forceRefresh: true }
      },
      {
        command: 'list_administrative_areas',
        args: { countryCode: 'VN', parentId: null, limit: 500 }
      },
      {
        command: 'list_administrative_areas',
        args: { countryCode: 'VN', parentId: 123, limit: 50 }
      },
      {
        command: 'search_administrative_areas',
        args: { countryCode: 'VN', query: 'Hà Nội', limit: 100 }
      },
      {
        command: 'search_administrative_areas',
        args: { countryCode: 'KR', query: 'Seoul', limit: 25 }
      }
    ]);
  });
});
