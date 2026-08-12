# Global Administrative Catalog Design

**Date:** 2026-08-12
**Status:** Approved for implementation
**Scope:** Replace hard-coded Google Maps location presets with a complete, refreshable country and administrative-area catalog. Preserve the existing browser scraper, durable job engine, URL construction, result extraction, and 500-unit job safety limit.

## 1. Purpose

RustSale currently stores a small hand-curated list of countries and locations in `world_locations.ts` and a legacy 63-province Vietnam hierarchy in `vietnam-divisions.json`. This data cannot represent all countries, varies in administrative depth, and becomes stale when borders or local-government structures change.

The replacement must expose every country and territory in the GeoNames country catalog and load current administrative divisions for a selected country without embedding the worldwide gazetteer in the application bundle.

## 2. Product Decisions

- GeoNames daily country dumps are the canonical operational source.
- Bundle a generated snapshot of `countryInfo.txt` so all countries remain selectable offline.
- Download `https://download.geonames.org/export/dump/{ISO2}.zip` only when a country is first used or explicitly refreshed.
- Cache parsed country data in an application-data SQLite database separate from CRM data.
- Import current `ADM1` through `ADM5` records and current populated-place records whose feature codes begin with `PPL`.
- Exclude historical, abandoned, destroyed, and section-only records (`*H`, `PPLQ`, `PPLW`, `PPLCH`, `PPLX`).
- Construct parent relationships from GeoNames administrative codes and expose a full canonical path for Google Maps text queries.
- Keep free-form location entry available if a catalog download fails.
- Keep the backend 500-location safety limit. The UI prevents selecting more than 500 administrative targets for one campaign.
- Do not modify `GoogleMapsScraper::search_url`, browser automation, CAPTCHA handling, scrolling, extraction, durable units, retry policy, or lead import.

## 3. Data Contract

### 3.1 Country

Each bundled country contains:

- ISO-3166 alpha-2 and alpha-3 codes;
- English display name;
- continent code;
- capital;
- GeoNames country ID;
- supported language codes.

The bundled snapshot is generated from GeoNames `countryInfo.txt` and includes countries and dependent territories represented by an ISO alpha-2 code.

### 3.2 Administrative Area

Each cached area contains:

- GeoNames ID;
- country code;
- parent GeoNames ID when known;
- primary UTF-8 name;
- ASCII/transliterated name;
- comma-separated aliases supplied in the country dump;
- feature code (`ADM1`–`ADM5`, `PPL*`);
- normalized administrative level;
- latitude and longitude;
- population;
- source modification date;
- canonical path assembled from the area through its ancestors and country.

### 3.3 Catalog Status

Each country cache records:

- state: `not_loaded`, `ready`, or `failed`;
- fetched timestamp;
- newest source modification date observed;
- imported record count;
- source URL;
- last error, when applicable.

The country bounding box is computed from loaded current records and is not
claimed before a country dump has been loaded.

## 4. Backend Architecture

Create `src-tauri/src/administrative_catalog/`:

- `types.rs`: serialized DTOs and internal parsed-record types.
- `countries.rs`: parses the bundled country snapshot.
- `parser.rs`: sans-I/O GeoNames TSV parsing, filtering, parent resolution, and canonical-path construction.
- `repository.rs`: SQLite schema, transactional country replacement, status, child listing, and bounded search.
- `service.rs`: validates ISO codes, downloads bounded ZIP files, parses the matching `{ISO2}.txt`, and coordinates cache reuse/refresh.
- `commands.rs`: Tauri commands for countries, status, loading, children, and search.

Managed state owns one `AdministrativeCatalogService` rooted at `<app-data>/administrative-catalog/catalog.db`.

## 5. Frontend Architecture

Create `src/lib/services/administrativeCatalogService.ts` for typed Tauri calls and `src/lib/components/AdministrativeAreaSelector.svelte` for country selection, download/refresh state, hierarchical browsing, bounded search, selected-target chips, and GeoNames attribution.

`GoogleMapsScraper.svelte` consumes selected areas as canonical location strings. If no area is selected during deep scan, the selected country name remains the single location. Grid mode keeps the existing preset, geocode, and manual bounding-box controls; selecting a country may initialize its country bounding box but does not change grid planning.

## 6. Failure and Safety Rules

- Accept only two-letter ASCII country codes present in the bundled catalog.
- Use HTTPS and a fixed GeoNames host.
- Limit compressed downloads to 128 MiB and extracted country text to 1 GiB.
- Parse and replace one country transactionally; a failed refresh leaves the previous ready snapshot available.
- Limit child/search responses to 500 rows and search queries to 200 characters.
- Reject more than 500 selected targets in the UI before job creation.
- Never silently relabel failed downloads as complete coverage.

## 7. Testing

- Country snapshot contains at least 249 ISO-coded entries and includes `VN`, `KR`, `US`, and `AQ`.
- TSV parser covers `ADM1`–`ADM5`, populated places, historic filtering, parent resolution, aliases, malformed rows, and deterministic ordering.
- Repository tests cover transactional replacement, stale refresh failure, child listing, search, and canonical paths.
- Command/service tests use injected HTTP bytes and never depend on live network.
- Frontend service tests cover command payloads.
- Component tests cover all-country rendering, country load, hierarchy/search selection, 500-target guard, and unchanged scrape-job payload semantics.
- Existing Google Maps scraper and job suites remain green without modifications to scraper code.

## 8. Attribution

The UI displays: `Location data © GeoNames, licensed under CC BY 4.0`, plus fetched and source modification dates for the selected country.
