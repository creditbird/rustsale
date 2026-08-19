# Global Grid Crawl Bounding Box & Grid Planner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enable full global country and regional targeting in Google Maps Grid Crawl mode by reusing `AdministrativeAreaSelector`, resolving areas to bounding boxes via cached structured geocoding with centroid-radius fallback, and providing an adaptive `GridPlanner` with safety policies and transparent UX.

**Architecture:** 
- A Rust backend `GeoBoundsResolver` service caches resolved bounding boxes in SQLite (`administrative_bounds_cache`) with structured OpenStreetMap Nominatim queries and centroid radius fallback.
- A `GridPlanner` calculates spatial dimensions, estimated cell counts, and adaptive minimum safe steps (`minimum_step ≈ sqrt(area / max_cells)`).
- Frontend `GoogleMapsScraper.svelte` shares the global `AdministrativeAreaSelector` between Keyword Deep Scan and Grid Crawl, seamlessly resolving selected administrative units into exact bounding boxes with live area calculation and one-click recommended step adjustments.

**Tech Stack:** Rust, Tauri 2, rusqlite, reqwest, Svelte 5 (Runes), TypeScript, Bun test, Cargo test.

## Global Constraints

- **ALWAYS use `bun` and `bunx` instead of `node`, `npm`, and `npx`.**
- **Preserve backward compatibility** with existing manual coordinate entry and Vietnam presets.
- **Strict safety budget:** 10,000 max initial cells per grid scrape job with transparent UX alerts and one-click recommendation.
- **Offline & Rate-limit resilience:** Structured geocoding results must be permanently cached locally in SQLite.

---

### Task 1: SQLite Bounds Cache Schema & GeoBoundsResolver in Rust

**Files:**
- Create: `src-tauri/src/administrative_catalog/bounds.rs`
- Modify: `src-tauri/src/administrative_catalog/repository.rs`
- Modify: `src-tauri/src/administrative_catalog/service.rs`
- Modify: `src-tauri/src/administrative_catalog/commands.rs`
- Modify: `src-tauri/src/administrative_catalog/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/administrative_catalog/bounds.rs`

**Interfaces:**
- Consumes: `administrative_catalog_areas` table records (`geoname_id`, `name`, `country_code`, `administrative_level`, `latitude`, `longitude`, `parent_id`).
- Produces: Tauri command `resolve_administrative_bounds(geoname_id: i64) -> Result<AdministrativeBoundsDto, String>`.

- [ ] **Step 1: Write the failing test for bounds caching and resolution**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bounds_cache_lookup_and_centroid_fallback() {
        let repo = AdministrativeCatalogRepository::new_in_memory().unwrap();
        let resolver = GeoBoundsResolver::new(repo);
        
        // When geoname has no cache, resolver falls back to centroid radius or geocoder
        let bounds = resolver.resolve_with_fallback(1566083, "VN", 10.7769, 106.7009, Some(2)).await.unwrap();
        assert!(bounds.min_lat < bounds.max_lat);
        assert!(bounds.min_lng < bounds.max_lng);
        assert!(bounds.min_lat <= 10.7769 && bounds.max_lat >= 10.7769);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib administrative_catalog::bounds`
Expected: FAIL with "module not found" or "unresolved import"

- [ ] **Step 3: Implement SQLite table `administrative_bounds_cache` and `GeoBoundsResolver`**

1. Add table schema in `src-tauri/src/administrative_catalog/repository.rs`:
```sql
CREATE TABLE IF NOT EXISTS administrative_bounds_cache (
    geoname_id INTEGER PRIMARY KEY,
    country_code TEXT NOT NULL,
    min_lat REAL NOT NULL,
    max_lat REAL NOT NULL,
    min_lng REAL NOT NULL,
    max_lng REAL NOT NULL,
    source TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_bounds_cache_country ON administrative_bounds_cache(country_code);
```
2. Implement `resolve_administrative_bounds` in `bounds.rs` and wire into `commands.rs` & `lib.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib administrative_catalog::bounds`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/administrative_catalog/
git commit -m "feat(backend): add administrative bounds cache and resolver"
```

---

### Task 2: GridPlanner & GridSafetyPolicy in Rust

**Files:**
- Create: `src-tauri/src/scrapers/grid_planner.rs`
- Modify: `src-tauri/src/scrapers/mod.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/scrapers/grid_planner.rs`

**Interfaces:**
- Consumes: Bounding box coordinates `[min_lat, max_lat, min_lng, max_lng]` and `requested_step_km: f64`.
- Produces: `GridPlan` struct with `area_km2`, `width_km`, `height_km`, `estimated_cells`, `recommended_step_km`, `exceeds_limit` and Tauri command `plan_grid_crawl`.

- [ ] **Step 1: Write failing unit tests for GridPlanner**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_planner_calculates_dimensions_and_adaptive_step() {
        // District 1 HCM (~7.7 km2) with 1.0 km step
        let plan = GridPlanner::plan([10.754, 10.798, 106.680, 106.714], 1.0, 10_000).unwrap();
        assert!(plan.area_km2 > 0.0);
        assert!(plan.estimated_cells < 100);
        assert!(!plan.exceeds_limit);

        // Huge bounding box with small 0.5 km step -> exceeds 10,000 limit
        let huge_plan = GridPlanner::plan([8.0, 23.0, 102.0, 110.0], 0.5, 10_000).unwrap();
        assert!(huge_plan.exceeds_limit);
        assert!(huge_plan.recommended_step_km.is_some());
        assert!(huge_plan.recommended_step_km.unwrap() > 5.0);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib scrapers::grid_planner`
Expected: FAIL with "module not found"

- [ ] **Step 3: Implement `GridPlanner` & `GridSafetyPolicy`**

Implement spatial distance calculations (Haversine/equatorial projection), cell estimation, adaptive step formula `sqrt(area_km2 / max_cells)`, and register `plan_grid_crawl` Tauri command.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib scrapers::grid_planner`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/scrapers/grid_planner.rs src-tauri/src/scrapers/mod.rs src-tauri/src/lib.rs
git commit -m "feat(backend): add GridPlanner and adaptive step estimation"
```

---

### Task 3: Typed Frontend Services & Administrative Bounds Client

**Files:**
- Modify: `src/lib/services/administrativeCatalogService.ts`
- Modify: `src/lib/services/googleMapsScraperService.ts`
- Create: `src/lib/services/__tests__/gridPlannerService.spec.ts`
- Test: `src/lib/services/__tests__/gridPlannerService.spec.ts`

**Interfaces:**
- Consumes: Tauri IPC commands `resolve_administrative_bounds` and `plan_grid_crawl`.
- Produces: Typed TypeScript APIs `resolveAdministrativeBounds(geonameId: number)` and `planGridCrawl(bbox, stepKm)`.

- [ ] **Step 1: Write failing TypeScript unit tests**

```typescript
import { describe, it, expect } from 'bun:test';
import { resolveAdministrativeBounds, planGridCrawl } from '$lib/services/administrativeCatalogService';

describe('administrativeCatalogService grid bounds & planning', () => {
  it('resolves administrative bounds and plans grid crawl', async () => {
    const bounds = await resolveAdministrativeBounds(1566083);
    expect(bounds.min_lat).toBeLessThan(bounds.max_lat);
    expect(bounds.min_lng).toBeLessThan(bounds.max_lng);

    const plan = await planGridCrawl([bounds.min_lat, bounds.max_lat, bounds.min_lng, bounds.max_lng], 2.0);
    expect(plan.estimated_cells).toBeGreaterThan(0);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `bun test src/lib/services/__tests__/gridPlannerService.spec.ts`
Expected: FAIL with missing functions

- [ ] **Step 3: Implement client wrappers in `administrativeCatalogService.ts`**

- [ ] **Step 4: Run test to verify it passes**

Run: `bun test src/lib/services/__tests__/gridPlannerService.spec.ts`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/lib/services/
git commit -m "feat(frontend): add administrative bounds and grid planner client services"
```

---

### Task 4: Global Administrative Targeting & Grid Planner UI Integration

**Files:**
- Modify: `src/lib/components/GoogleMapsScraper.svelte`
- Modify: `src/lib/components/__tests__/GoogleMapsScraper.spec.ts`
- Test: `src/lib/components/__tests__/GoogleMapsScraper.spec.ts`

**Interfaces:**
- Reuses: `AdministrativeAreaSelector.svelte` inside Grid Mode.
- Exposes: Selection modes (By Administrative Area vs Manual / Radius Scan), live area metrics ($km^2$), transparent cell warnings, and `[Use recommended step]` button.

- [ ] **Step 1: Write failing component tests for Grid Mode Administrative Area selection**

```typescript
it('resolves bounds when selecting administrative area in Grid Mode and updates coordinates', async () => {
  // Switch to Grid Mode -> Select Administrative Area -> bounds auto-populate and grid estimate updates
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `bun test src/lib/components/__tests__/GoogleMapsScraper.spec.ts`
Expected: FAIL

- [ ] **Step 3: Update `GoogleMapsScraper.svelte`**

1. In Grid mode, provide a sub-mode toggle:
   - "Select Administrative Area (Global)" (renders `AdministrativeAreaSelector`).
   - "Manual Coordinates / Custom Radius Scan" (renders custom Lat/Lng inputs and OSM search/presets).
2. Wire `onSelectionChange` / `onAreaSelect` to `resolveAdministrativeBounds` to auto-fill Bounding Box.
3. Wire `planGridCrawl` to update real-time statistics and provide one-click `[Use recommended step]` button.

- [ ] **Step 4: Run component tests and type check**

Run: `bun test src/lib/components/__tests__/GoogleMapsScraper.spec.ts && bun run check`
Expected: PASS with 0 type errors.

- [ ] **Step 5: Commit**

```bash
git add src/lib/components/GoogleMapsScraper.svelte src/lib/components/__tests__/GoogleMapsScraper.spec.ts
git commit -m "feat(ui): integrate global administrative selector and adaptive grid planner in GoogleMapsScraper"
```

---

### Task 5: End-to-End Verification, /gpt-web-review & Merge Loop

**Files:**
- Repository-wide test & build verification.

- [ ] **Step 1: Run full frontend test suite**
Run: `bun test`
Expected: All tests pass.

- [ ] **Step 2: Run full backend Rust test suite**
Run: `cargo test --lib`
Expected: All tests pass.

- [ ] **Step 3: Run GitNexus change detection**
Run: MCP `gitnexus_detect_changes()` to ensure only expected symbols changed.

- [ ] **Step 4: Commit and Push to Feature Branch**
```bash
git checkout -b feat/global-grid-crawl-bounds-planner
git push -u origin feat/global-grid-crawl-bounds-planner
```

- [ ] **Step 5: Create Pull Request via GitHub CLI**
```bash
gh pr create --title "feat: Global Administrative Area Selector and Adaptive Grid Planner for Grid Crawl" --body "..."
```

- [ ] **Step 6: Invoke /gpt-web-review**
Perform ChatGPT Web review round via Chrome DevTools MCP, apply any valid fixes with `/pr-fix`, verify approval, and merge into `master`.
