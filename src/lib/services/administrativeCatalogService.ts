import { invoke } from '@tauri-apps/api/core';

export type AdministrativeAreaKind = 'administrative' | 'populated_place';
export type AdministrativeCatalogState = 'not_loaded' | 'ready' | 'failed';

export interface AdministrativeCountry {
  code: string;
  iso3: string;
  name: string;
  capital: string;
  continent_code: string;
  languages: string[];
  geoname_id: number;
}

export interface AdministrativeCountryStatus {
  country_code: string;
  state: AdministrativeCatalogState;
  fetched_at: string | null;
  source_updated_at: string | null;
  record_count: number;
  source_url: string | null;
  last_error: string | null;
  bbox: [number, number, number, number] | null;
}

export interface AdministrativeAreaSummary {
  geoname_id: number;
  country_code: string;
  parent_id: number | null;
  name: string;
  ascii_name: string;
  feature_code: string;
  administrative_level: number | null;
  kind: AdministrativeAreaKind;
  latitude: number;
  longitude: number;
  population: number;
  canonical_path: string;
  has_children: boolean;
}

export function listAdministrativeCountries(): Promise<AdministrativeCountry[]> {
  return invoke<AdministrativeCountry[]>('list_administrative_countries');
}

export function getAdministrativeCountryStatus(
  countryCode: string
): Promise<AdministrativeCountryStatus> {
  return invoke<AdministrativeCountryStatus>('get_administrative_country_status', {
    countryCode
  });
}

export function loadAdministrativeCountry(
  countryCode: string,
  forceRefresh = false
): Promise<AdministrativeCountryStatus> {
  return invoke<AdministrativeCountryStatus>('load_administrative_country', {
    countryCode,
    forceRefresh
  });
}

export function listAdministrativeAreas(
  countryCode: string,
  parentId: number | null = null,
  limit = 500
): Promise<AdministrativeAreaSummary[]> {
  return invoke<AdministrativeAreaSummary[]>('list_administrative_areas', {
    countryCode,
    parentId,
    limit
  });
}

export function searchAdministrativeAreas(
  countryCode: string,
  query: string,
  limit = 100
): Promise<AdministrativeAreaSummary[]> {
  return invoke<AdministrativeAreaSummary[]>('search_administrative_areas', {
    countryCode,
    query,
    limit
  });
}

export interface AdministrativeBounds {
  geoname_id: number;
  country_code: string;
  min_lat: number;
  max_lat: number;
  min_lng: number;
  max_lng: number;
  source: string;
  updated_at: string;
}

export interface GridPlan {
  bbox: [number, number, number, number];
  area_km2: number;
  width_km: number;
  height_km: number;
  requested_step_km: number;
  effective_step_km: number;
  estimated_cells: number;
  recommended_step_km: number | null;
  exceeds_limit: boolean;
  estimated_chunks: number;
}

export function resolveAdministrativeBounds(
  geonameId: number
): Promise<AdministrativeBounds> {
  return invoke<AdministrativeBounds>('resolve_administrative_bounds', {
    geonameId
  });
}

export function planGridCrawl(
  bbox: [number, number, number, number],
  stepKm: number
): Promise<GridPlan> {
  return invoke<GridPlan>('plan_grid_crawl', {
    bbox,
    stepKm
  });
}

export function geocodeBoundingBox(
  query: string,
  countryCode?: string
): Promise<[number, number, number, number] | null> {
  return invoke<[number, number, number, number] | null>('geocode_bounding_box', {
    query,
    countryCode: countryCode || null
  });
}

