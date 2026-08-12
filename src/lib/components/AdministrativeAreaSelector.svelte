<script lang="ts">
  import { onMount } from 'svelte';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { Input } from '$lib/components/ui/input';
  import * as ScrollArea from '$lib/components/ui/scroll-area';
  import * as Select from '$lib/components/ui/select';
  import { Skeleton } from '$lib/components/ui/skeleton';
  import {
    getAdministrativeCountryStatus,
    listAdministrativeAreas,
    listAdministrativeCountries,
    loadAdministrativeCountry,
    searchAdministrativeAreas,
    type AdministrativeAreaSummary,
    type AdministrativeCountry,
    type AdministrativeCountryStatus
  } from '$lib/services/administrativeCatalogService';
  import { ArrowLeft, Database, Globe2, RefreshCw, Search, X } from 'lucide-svelte';

  interface Props {
    selectedCountryCode?: string;
    selectedAreas?: AdministrativeAreaSummary[];
    disabled?: boolean;
    onCountryChange?: (country: AdministrativeCountry) => void;
    onSelectionChange?: (areas: AdministrativeAreaSummary[]) => void;
    onBoundingBoxChange?: (bbox: [number, number, number, number]) => void;
  }

  interface RegionOption {
    code: string;
    label: string;
  }

  interface LevelDefinition {
    level: number;
    title: string;
    filterLabel: string;
  }

  const REGION_OPTIONS: RegionOption[] = [
    { code: 'AF', label: 'Africa / Châu Phi' },
    { code: 'AN', label: 'Antarctica / Nam Cực' },
    { code: 'AS', label: 'Asia / Châu Á' },
    { code: 'EU', label: 'Europe / Châu Âu' },
    { code: 'NA', label: 'North America / Bắc Mỹ' },
    { code: 'OC', label: 'Oceania / Châu Đại Dương' },
    { code: 'SA', label: 'South America / Nam Mỹ' },
    { code: 'OTHER', label: 'Other / Khác' }
  ];

  const LEVEL_DEFINITIONS: LevelDefinition[] = [
    {
      level: 1,
      title: 'Provinces / States / Cities',
      filterLabel: 'Filter provinces, states, or cities'
    },
    {
      level: 2,
      title: 'Districts / Counties / Cities',
      filterLabel: 'Filter districts, counties, or cities'
    },
    {
      level: 3,
      title: 'Communes / Wards / Municipalities',
      filterLabel: 'Filter communes, wards, or municipalities'
    },
    {
      level: 4,
      title: 'Wards / Villages',
      filterLabel: 'Filter wards or villages'
    },
    {
      level: 5,
      title: 'Villages / Localities',
      filterLabel: 'Filter villages or localities'
    }
  ];

  const REGION_BY_CONTINENT: Record<string, string> = {
    AF: 'AF',
    AN: 'AN',
    AS: 'AS',
    EU: 'EU',
    NA: 'NA',
    OC: 'OC',
    SA: 'SA'
  };

  let {
    selectedCountryCode = $bindable('VN'),
    selectedAreas = $bindable<AdministrativeAreaSummary[]>([]),
    disabled = false,
    onCountryChange = () => {},
    onSelectionChange = () => {},
    onBoundingBoxChange = () => {}
  }: Props = $props();

  let countries = $state<AdministrativeCountry[]>([]);
  let countryStatus = $state<AdministrativeCountryStatus | null>(null);
  let levelAreas = $state<AdministrativeAreaSummary[][]>(emptyAreaLevels());
  let expandedParentIds = $state<Set<number>[]>(emptyExpandedLevels());
  let levelFilters = $state<string[]>(emptyLevelFilters());
  let loadingLevels = $state<boolean[]>(emptyLevelLoading());
  let searchResults = $state<AdministrativeAreaSummary[] | null>(null);
  let selectedRegionCode = $state('AS');
  let query = $state('');
  let loadingCountries = $state(true);
  let loadingCountry = $state(false);
  let loadingAreas = $state(false);
  let catalogError = $state<string | null>(null);
  let selectionError = $state<string | null>(null);
  let requestGeneration = 0;

  let currentCountry = $derived(
    countries.find((country) => country.code === selectedCountryCode.trim().toUpperCase()) ?? null
  );
  let regionCountries = $derived(
    countries.filter((country) => regionForCountry(country) === selectedRegionCode)
  );
  let selectedIds = $derived(new Set(selectedAreas.map((area) => area.geoname_id)));

  onMount(() => {
    void initialize();
  });

  function emptyAreaLevels(): AdministrativeAreaSummary[][] {
    return Array.from({ length: LEVEL_DEFINITIONS.length }, () => []);
  }

  function emptyExpandedLevels(): Set<number>[] {
    return Array.from({ length: LEVEL_DEFINITIONS.length }, () => new Set<number>());
  }

  function emptyLevelFilters(): string[] {
    return Array.from({ length: LEVEL_DEFINITIONS.length }, () => '');
  }

  function emptyLevelLoading(): boolean[] {
    return Array.from({ length: LEVEL_DEFINITIONS.length }, () => false);
  }

  function regionForCountry(country: AdministrativeCountry): string {
    return REGION_BY_CONTINENT[country.continent_code.trim().toUpperCase()] ?? 'OTHER';
  }

  function countriesInRegion(regionCode: string): AdministrativeCountry[] {
    return countries.filter((country) => regionForCountry(country) === regionCode);
  }

  function preferredCountryForRegion(regionCode: string): AdministrativeCountry | null {
    const candidates = countriesInRegion(regionCode);
    return (
      candidates.find((country) => country.code === selectedCountryCode.trim().toUpperCase()) ??
      (regionCode === 'AS' ? candidates.find((country) => country.code === 'VN') : null) ??
      candidates[0] ??
      null
    );
  }

  async function initialize() {
    loadingCountries = true;
    catalogError = null;
    try {
      countries = await listAdministrativeCountries();
      const requestedCode = selectedCountryCode.trim().toUpperCase();
      const selected =
        countries.find((country) => country.code === requestedCode) ??
        countries.find((country) => country.code === 'VN') ??
        countries[0];
      if (!selected) {
        throw new Error('The bundled country catalog is empty.');
      }
      selectedCountryCode = selected.code;
      selectedRegionCode = regionForCountry(selected);
      onCountryChange(selected);
      await initializeCountry(false);
    } catch (error) {
      catalogError = errorMessage(error);
    } finally {
      loadingCountries = false;
    }
  }

  function resetHierarchy() {
    levelAreas = emptyAreaLevels();
    expandedParentIds = emptyExpandedLevels();
    levelFilters = emptyLevelFilters();
    loadingLevels = emptyLevelLoading();
    searchResults = null;
    query = '';
  }

  async function initializeCountry(forceRefresh: boolean) {
    const generation = ++requestGeneration;
    loadingCountry = true;
    loadingAreas = false;
    catalogError = null;
    selectionError = null;
    countryStatus = null;
    resetHierarchy();
    try {
      let status = forceRefresh
        ? await loadAdministrativeCountry(selectedCountryCode, true)
        : await getAdministrativeCountryStatus(selectedCountryCode);
      if (generation !== requestGeneration) return;
      if (!forceRefresh && status.state !== 'ready') {
        status = await loadAdministrativeCountry(selectedCountryCode, false);
      }
      if (generation !== requestGeneration) return;
      countryStatus = status;
      if (status.bbox) onBoundingBoxChange(status.bbox);
      if (status.state !== 'ready') {
        catalogError = status.last_error ?? 'Country data could not be loaded.';
        return;
      }
      if (status.last_error) catalogError = status.last_error;
      await loadRootLevel(generation);
    } catch (error) {
      if (generation === requestGeneration) catalogError = errorMessage(error);
    } finally {
      if (generation === requestGeneration) loadingCountry = false;
    }
  }

  async function loadRootLevel(generation = requestGeneration) {
    loadingAreas = true;
    setLevelLoading(0, true);
    try {
      const areas = await listAdministrativeAreas(selectedCountryCode, null, 500);
      if (generation !== requestGeneration) return;
      setLevelAreas(0, deduplicateAreas(areas));
    } catch (error) {
      if (generation === requestGeneration) catalogError = errorMessage(error);
    } finally {
      if (generation === requestGeneration) {
        setLevelLoading(0, false);
        loadingAreas = false;
      }
    }
  }

  async function refreshCascadeFrom(expansionLevel: number, generation = requestGeneration) {
    loadingAreas = true;
    try {
      for (let panelIndex = expansionLevel + 1; panelIndex < LEVEL_DEFINITIONS.length; panelIndex += 1) {
        if (generation !== requestGeneration) return;
        const parentIds = [...expandedParentIds[panelIndex - 1]];
        if (parentIds.length === 0) {
          clearAreaLevelsFrom(panelIndex);
          break;
        }
        setLevelLoading(panelIndex, true);
        try {
          const areas = await fetchChildren(parentIds, generation);
          if (generation !== requestGeneration) return;
          setLevelAreas(panelIndex, areas);
        } finally {
          if (generation === requestGeneration) setLevelLoading(panelIndex, false);
        }
      }
    } catch (error) {
      if (generation === requestGeneration) catalogError = errorMessage(error);
    } finally {
      if (generation === requestGeneration) loadingAreas = false;
    }
  }

  async function fetchChildren(parentIds: number[], generation: number): Promise<AdministrativeAreaSummary[]> {
    const responses = await Promise.all(
      parentIds.map((parentId) => listAdministrativeAreas(selectedCountryCode, parentId, 500))
    );
    if (generation !== requestGeneration) return [];
    return deduplicateAreas(responses.flat());
  }

  function setLevelAreas(levelIndex: number, areas: AdministrativeAreaSummary[]) {
    levelAreas = levelAreas.map((current, index) => (index === levelIndex ? areas : current));
  }

  function clearAreaLevelsFrom(levelIndex: number) {
    levelAreas = levelAreas.map((areas, index) => (index >= levelIndex ? [] : areas));
    loadingLevels = loadingLevels.map((loading, index) => (index >= levelIndex ? false : loading));
  }

  function setLevelLoading(levelIndex: number, loading: boolean) {
    loadingLevels = loadingLevels.map((current, index) => (index === levelIndex ? loading : current));
  }

  function deduplicateAreas(areas: AdministrativeAreaSummary[]): AdministrativeAreaSummary[] {
    const byId = new Map<number, AdministrativeAreaSummary>();
    for (const area of areas) byId.set(area.geoname_id, area);
    return [...byId.values()].sort(
      (left, right) => left.canonical_path.localeCompare(right.canonical_path) || left.geoname_id - right.geoname_id
    );
  }

  async function handleRegionChange(regionCode: string | undefined) {
    if (!regionCode || regionCode === selectedRegionCode) return;
    selectedRegionCode = regionCode;
    const country = preferredCountryForRegion(regionCode);
    if (country) {
      await handleCountryChange(country.code);
    } else {
      selectedAreas = [];
      onSelectionChange([]);
      resetHierarchy();
      countryStatus = null;
    }
  }

  async function handleCountryChange(countryCode: string | undefined) {
    if (!countryCode) return;
    const normalizedCode = countryCode.toUpperCase();
    if (normalizedCode === selectedCountryCode.trim().toUpperCase()) return;
    const country = countries.find((candidate) => candidate.code === normalizedCode);
    if (!country) return;
    selectedCountryCode = normalizedCode;
    selectedRegionCode = regionForCountry(country);
    selectedAreas = [];
    onSelectionChange([]);
    onCountryChange(country);
    await initializeCountry(false);
  }

  function updateExpanded(levelIndex: number, ids: Set<number>) {
    expandedParentIds = expandedParentIds.map((current, index) => (index === levelIndex ? ids : current));
  }

  async function browseArea(area: AdministrativeAreaSummary, levelIndex: number) {
    if (!area.has_children || loadingAreas || levelIndex >= LEVEL_DEFINITIONS.length - 1) return;
    const nextIds = new Set(expandedParentIds[levelIndex]);
    nextIds.add(area.geoname_id);
    updateExpanded(levelIndex, nextIds);
    searchResults = null;
    query = '';
    await refreshCascadeFrom(levelIndex);
  }

  function filteredAreas(levelIndex: number): AdministrativeAreaSummary[] {
    const filter = levelFilters[levelIndex]?.trim().toLocaleLowerCase() ?? '';
    if (!filter) return levelAreas[levelIndex] ?? [];
    return (levelAreas[levelIndex] ?? []).filter((area) =>
      `${area.name} ${area.ascii_name} ${area.canonical_path}`.toLocaleLowerCase().includes(filter)
    );
  }

  function shouldShowLevel(levelIndex: number): boolean {
    return levelIndex === 0 || expandedParentIds[levelIndex - 1].size > 0;
  }

  function selectedCountForLevel(levelIndex: number): number {
    const visibleIds = new Set((levelAreas[levelIndex] ?? []).map((area) => area.geoname_id));
    return selectedAreas.filter((area) => visibleIds.has(area.geoname_id)).length;
  }

  function levelIndexForArea(area: AdministrativeAreaSummary): number {
    const level = area.administrative_level ?? 1;
    return Math.max(0, Math.min(LEVEL_DEFINITIONS.length - 1, level - 1));
  }

  function knownAreas(): AdministrativeAreaSummary[] {
    return [...levelAreas.flat(), ...selectedAreas];
  }

  function collectSubtreeIds(rootIds: Set<number>): Set<number> {
    const ids = new Set(rootIds);
    let changed = true;
    while (changed) {
      changed = false;
      for (const area of knownAreas()) {
        if (area.parent_id !== null && ids.has(area.parent_id) && !ids.has(area.geoname_id)) {
          ids.add(area.geoname_id);
          changed = true;
        }
      }
    }
    return ids;
  }

  function removeSubtrees(rootIds: Set<number>, expansionLevel: number) {
    const removedIds = collectSubtreeIds(rootIds);
    selectedAreas = selectedAreas.filter((area) => !removedIds.has(area.geoname_id));
    expandedParentIds = expandedParentIds.map((ids, index) => {
      if (index < expansionLevel) return ids;
      return new Set([...ids].filter((id) => !removedIds.has(id)));
    });
    clearAreaLevelsFrom(expansionLevel + 1);
    onSelectionChange([...selectedAreas]);
  }

  async function toggleArea(area: AdministrativeAreaSummary, checked: boolean, levelIndex: number) {
    selectionError = null;
    if (checked) {
      if (selectedIds.has(area.geoname_id)) return;
      if (selectedAreas.length >= 500) {
        selectionError = 'Maximum 500 administrative targets per campaign.';
        return;
      }
      selectedAreas = [...selectedAreas, area];
      onSelectionChange([...selectedAreas]);
      if (area.has_children && levelIndex < LEVEL_DEFINITIONS.length - 1) {
        const nextIds = new Set(expandedParentIds[levelIndex]);
        nextIds.add(area.geoname_id);
        updateExpanded(levelIndex, nextIds);
        await refreshCascadeFrom(levelIndex);
      }
      return;
    }

    removeSubtrees(new Set([area.geoname_id]), levelIndex);
    await refreshCascadeFrom(levelIndex);
  }

  async function selectAllLevel(levelIndex: number) {
    const areas = filteredAreas(levelIndex);
    const additions = areas.filter((area) => !selectedIds.has(area.geoname_id));
    const room = Math.max(0, 500 - selectedAreas.length);
    if (additions.length > room) {
      selectionError = 'Maximum 500 administrative targets per campaign.';
    }
    selectedAreas = [...selectedAreas, ...additions.slice(0, room)];
    onSelectionChange([...selectedAreas]);
    const expandableIds = new Set(
      areas.filter((area) => area.has_children).map((area) => area.geoname_id)
    );
    if (expandableIds.size > 0 && levelIndex < LEVEL_DEFINITIONS.length - 1) {
      updateExpanded(
        levelIndex,
        new Set([...expandedParentIds[levelIndex], ...expandableIds])
      );
      await refreshCascadeFrom(levelIndex);
    }
  }

  async function clearLevel(levelIndex: number) {
    selectionError = null;
    const rootIds = new Set((levelAreas[levelIndex] ?? []).map((area) => area.geoname_id));
    removeSubtrees(rootIds, levelIndex);
    expandedParentIds = expandedParentIds.map((ids, index) =>
      index >= levelIndex ? new Set<number>() : ids
    );
    if (levelIndex > 0) await refreshCascadeFrom(levelIndex - 1);
  }

  async function runSearch() {
    const normalizedQuery = query.trim();
    if (!normalizedQuery) {
      searchResults = null;
      return;
    }
    loadingAreas = true;
    catalogError = null;
    try {
      searchResults = await searchAdministrativeAreas(selectedCountryCode, normalizedQuery, 100);
    } catch (error) {
      catalogError = errorMessage(error);
    } finally {
      loadingAreas = false;
    }
  }

  function restoreHierarchy() {
    searchResults = null;
    query = '';
  }

  function toggleSearchResult(area: AdministrativeAreaSummary, checked: boolean) {
    void toggleArea(area, checked, levelIndexForArea(area));
  }

  async function removeArea(geonameId: number) {
    const area = selectedAreas.find((candidate) => candidate.geoname_id === geonameId);
    if (!area) return;
    const levelIndex = levelIndexForArea(area);
    removeSubtrees(new Set([geonameId]), levelIndex);
    await refreshCascadeFrom(levelIndex);
  }

  function clearSelection() {
    selectedAreas = [];
    selectionError = null;
    onSelectionChange([]);
  }

  function errorMessage(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }

  function sourceDate(status: AdministrativeCountryStatus | null): string {
    return status?.source_updated_at ?? 'not loaded';
  }

  function fetchedDate(status: AdministrativeCountryStatus | null): string {
    return status?.fetched_at ?? 'not loaded';
  }
</script>

<section
  class="flex flex-col gap-3 rounded-lg border border-border bg-muted/30 p-3 text-xs"
  data-testid="administrative-area-selector"
>
  <div class="flex flex-wrap items-start justify-between gap-2">
    <div class="flex min-w-0 items-start gap-2">
      <Globe2 class="mt-0.5 size-4 shrink-0 text-emerald-600" />
      <div class="min-w-0">
        <p class="font-bold text-foreground">Global Administrative Catalog</p>
        <p class="text-[11px] text-muted-foreground">
          {loadingCountries ? 'Loading countries…' : `${countries.length} countries and territories`}
        </p>
      </div>
    </div>
    <div class="flex items-center gap-2">
      {#if countryStatus}
        <Badge variant={countryStatus.state === 'ready' ? 'secondary' : 'outline'}>
          {countryStatus.state.replace('_', ' ')}
        </Badge>
      {/if}
      <Button
        type="button"
        variant="outline"
        size="xs"
        aria-label="Refresh country data"
        disabled={disabled || loadingCountry || loadingCountries}
        onclick={() => void initializeCountry(true)}
      >
        <RefreshCw data-icon="inline-start" class={loadingCountry ? 'animate-spin' : undefined} />
        Refresh
      </Button>
    </div>
  </div>

  <div class="grid grid-cols-1 gap-2 md:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)_auto]">
    <div class="flex min-w-0 flex-col gap-1">
      <label for="administrative-region" class="text-[11px] font-medium text-muted-foreground">
        Region / Khu vực
      </label>
      {#if loadingCountries}
        <Skeleton class="h-8 w-full" />
      {:else}
        <Select.Root
          type="single"
          value={selectedRegionCode}
          disabled={disabled || loadingCountry}
          onValueChange={(value) => void handleRegionChange(value)}
        >
          <Select.Trigger
            id="administrative-region"
            aria-label="Region / Khu vực"
            class="h-8 w-full bg-background text-xs"
          >
            {REGION_OPTIONS.find((region) => region.code === selectedRegionCode)?.label ?? 'Select region'}
          </Select.Trigger>
          <Select.Content class="max-h-72">
            <Select.Group>
              {#each REGION_OPTIONS as region (region.code)}
                <Select.Item value={region.code}>{region.label}</Select.Item>
              {/each}
            </Select.Group>
          </Select.Content>
        </Select.Root>
      {/if}
    </div>

    <div class="flex min-w-0 flex-col gap-1">
      <label for="administrative-country" class="text-[11px] font-medium text-muted-foreground">
        Country / Quốc gia
      </label>
      {#if loadingCountries}
        <Skeleton class="h-8 w-full" />
      {:else}
        <Select.Root
          type="single"
          value={selectedCountryCode}
          disabled={disabled || loadingCountry || regionCountries.length === 0}
          onValueChange={(value) => void handleCountryChange(value)}
        >
          <Select.Trigger
            id="administrative-country"
            aria-label="Country / Quốc gia"
            class="h-8 w-full bg-background text-xs"
          >
            {currentCountry?.name ?? 'Select country'}
          </Select.Trigger>
          <Select.Content class="max-h-72">
            <Select.Group>
              {#each regionCountries as country (country.code)}
                <Select.Item value={country.code}>{country.name} ({country.code})</Select.Item>
              {/each}
            </Select.Group>
          </Select.Content>
        </Select.Root>
      {/if}
    </div>

    <div class="flex items-end">
      <Badge variant="outline" class="h-8 px-3">
        <Database data-icon="inline-start" />
        {countryStatus?.record_count.toLocaleString('en-US') ?? '0'} places
      </Badge>
    </div>
  </div>

  {#if catalogError}
    <div
      class="rounded-md border border-destructive/20 bg-destructive/10 p-2 text-[11px] text-destructive"
      role="alert"
    >
      <p class="font-semibold">{catalogError}</p>
      <p class="mt-0.5">You can still use the free-form location field without catalog data.</p>
    </div>
  {/if}

  {#if selectionError}
    <div
      class="rounded-md border border-destructive/20 bg-destructive/10 p-2 text-[11px] text-destructive"
      role="alert"
    >
      {selectionError}
    </div>
  {/if}

  {#if countryStatus?.state === 'ready'}
    <form
      class="flex items-center gap-2"
      onsubmit={(event) => {
        event.preventDefault();
        void runSearch();
      }}
    >
      <Input
        aria-label="Search administrative areas"
        placeholder="Search states, cities, districts, wards…"
        class="h-8 text-xs"
        bind:value={query}
        disabled={disabled || loadingAreas}
      />
      <Button
        type="submit"
        variant="outline"
        size="icon-sm"
        aria-label="Search catalog"
        disabled={disabled || loadingAreas || !query.trim()}
      >
        <Search />
      </Button>
    </form>

    {#if searchResults !== null}
      <div class="flex min-h-8 flex-wrap items-center justify-between gap-2">
        <div class="flex min-w-0 items-center gap-2 text-[11px] text-muted-foreground">
          <Button type="button" variant="ghost" size="xs" onclick={restoreHierarchy}>
            <ArrowLeft data-icon="inline-start" />
            Browse hierarchy
          </Button>
          <span>{searchResults.length} results</span>
        </div>
        <Badge variant="outline">{selectedAreas.length}/500 selected</Badge>
      </div>

      <ScrollArea.Root class="h-44 rounded-md border border-border bg-background/70">
        <div class="flex flex-col gap-1 p-2" data-testid="administrative-search-results">
          {#if loadingAreas}
            {#each Array(4) as _, index (index)}
              <Skeleton class="h-10 w-full" />
            {/each}
          {:else if searchResults.length === 0}
            <div class="flex h-28 items-center justify-center px-4 text-center text-muted-foreground">
              No administrative areas found.
            </div>
          {:else}
            {#each searchResults as area (area.geoname_id)}
              <div class="flex min-w-0 items-center gap-2 rounded-md px-2 py-1.5 hover:bg-muted/60">
                <Checkbox
                  checked={selectedIds.has(area.geoname_id)}
                  aria-label={`Select ${area.name}`}
                  disabled={disabled}
                  onCheckedChange={(checked) => toggleSearchResult(area, Boolean(checked))}
                />
                <div class="min-w-0 flex-1">
                  <p class="truncate font-medium text-foreground">{area.name}</p>
                  <p class="truncate text-[10px] text-muted-foreground" title={area.canonical_path}>
                    {area.canonical_path}
                  </p>
                </div>
                <Badge variant="outline" class="hidden sm:inline-flex">{area.feature_code}</Badge>
              </div>
            {/each}
          {/if}
        </div>
      </ScrollArea.Root>
    {:else}
      <div class="flex min-h-8 items-center justify-end">
        <Badge variant="outline">{selectedAreas.length}/500 selected</Badge>
      </div>
      {#each LEVEL_DEFINITIONS as definition, levelIndex (definition.level)}
        {#if shouldShowLevel(levelIndex)}
          <div class="flex flex-col gap-2" data-testid={`administrative-level-${definition.level}`}>
            <div class="flex flex-col gap-2 lg:flex-row lg:items-center lg:justify-between">
              <div class="flex items-center gap-2">
                <span class="font-semibold text-foreground">{definition.title}</span>
                <Badge variant="outline">{selectedCountForLevel(levelIndex)} selected</Badge>
              </div>
              <div class="flex flex-wrap items-center gap-1.5">
                <Input
                  aria-label={definition.filterLabel}
                  placeholder="Filter…"
                  class="h-7 w-40 text-[11px]"
                  value={levelFilters[levelIndex]}
                  disabled={disabled || loadingAreas}
                  oninput={(event) => {
                    levelFilters = levelFilters.map((filter, index) =>
                      index === levelIndex ? (event.currentTarget as HTMLInputElement).value : filter
                    );
                  }}
                />
                <Button
                  type="button"
                  variant="ghost"
                  size="xs"
                  aria-label={`Select all ${definition.title}`}
                  onclick={() => void selectAllLevel(levelIndex)}
                  disabled={disabled || loadingAreas || filteredAreas(levelIndex).length === 0}
                >
                  Select All
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  size="xs"
                  aria-label={`Clear ${definition.title}`}
                  onclick={() => void clearLevel(levelIndex)}
                  disabled={disabled || loadingAreas}
                >
                  Clear
                </Button>
              </div>
            </div>

            <ScrollArea.Root class="h-44 rounded-md border border-border bg-background/70">
              <div class="grid grid-cols-1 gap-1 p-2 sm:grid-cols-2 lg:grid-cols-3">
                {#if loadingLevels[levelIndex]}
                  {#each Array(4) as _, skeletonIndex (skeletonIndex)}
                    <Skeleton class="h-10 w-full" />
                  {/each}
                {:else if filteredAreas(levelIndex).length === 0}
                  <div class="col-span-full flex h-28 items-center justify-center px-4 text-center text-muted-foreground">
                    No administrative areas found for this level.
                  </div>
                {:else}
                  {#each filteredAreas(levelIndex) as area (area.geoname_id)}
                    <div
                      class="flex min-w-0 items-center gap-2 rounded-md px-2 py-1.5 hover:bg-muted/60"
                    >
                      <Checkbox
                        checked={selectedIds.has(area.geoname_id)}
                        aria-label={`Select ${area.name}`}
                        disabled={disabled || loadingCountry}
                        onCheckedChange={(checked) => void toggleArea(area, Boolean(checked), levelIndex)}
                      />
                      <div class="min-w-0 flex-1">
                        <p class="truncate font-medium text-foreground">{area.name}</p>
                        <p class="truncate text-[10px] text-muted-foreground" title={area.canonical_path}>
                          {area.canonical_path}
                        </p>
                      </div>
                      <Badge variant="outline" class="hidden sm:inline-flex">{area.feature_code}</Badge>
                      {#if area.has_children && levelIndex < LEVEL_DEFINITIONS.length - 1}
                        <Button
                          type="button"
                          variant="ghost"
                          size="icon-xs"
                          aria-label={`Browse ${area.name}`}
                          disabled={disabled || loadingAreas}
                          onclick={() => void browseArea(area, levelIndex)}
                        >
                          <span aria-hidden="true">›</span>
                        </Button>
                      {/if}
                    </div>
                  {/each}
                {/if}
              </div>
            </ScrollArea.Root>
          </div>
        {/if}
      {/each}
    {/if}
  {/if}

  {#if selectedAreas.length > 0}
    <div class="flex flex-col gap-2 border-t border-border/60 pt-2">
      <div class="flex items-center justify-between gap-2">
        <span class="font-semibold text-foreground">Selected scrape targets</span>
        <Button type="button" variant="ghost" size="xs" onclick={clearSelection} disabled={disabled}>
          Clear all
        </Button>
      </div>
      <div class="flex flex-wrap gap-1.5">
        {#each selectedAreas.slice(0, 12) as area (area.geoname_id)}
          <Badge variant="secondary" class="max-w-full gap-1">
            <span class="max-w-64 truncate" title={area.canonical_path}>{area.canonical_path}</span>
            <button
              type="button"
              class="rounded-full p-0.5 hover:bg-background/70"
              aria-label={`Remove ${area.name}`}
              disabled={disabled}
              onclick={() => void removeArea(area.geoname_id)}
            >
              <X class="size-3" />
            </button>
          </Badge>
        {/each}
        {#if selectedAreas.length > 12}
          <Badge variant="outline">+{selectedAreas.length - 12} more</Badge>
        {/if}
      </div>
    </div>
  {/if}

  <p class="text-[10px] leading-relaxed text-muted-foreground">
    Location data © GeoNames, licensed under CC BY 4.0. Fetched: {fetchedDate(countryStatus)}.
    Source updated: {sourceDate(countryStatus)}.
  </p>
</section>
