<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import divisions from '$lib/constants/vietnam-divisions.json';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Badge } from '$lib/components/ui/badge';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import * as Card from '$lib/components/ui/card';
  import * as Select from '$lib/components/ui/select';
  import { Progress } from '$lib/components/ui/progress';
  import { Search, MapPin, Play, Square, Terminal, Check, X, Building, Grid, Locate } from 'lucide-svelte';

  let mode = $state<'keyword' | 'grid'>('keyword');
  let keyword = $state('');
  let location = $state('');

  let minLat = $state(10.7);
  let maxLat = $state(10.8);
  let minLng = $state(106.6);
  let maxLng = $state(106.7);
  let stepKm = $state(2.0);

  let geocodeQuery = $state('');
  let isGeocoding = $state(false);

  let deepScan = $state(true);
  let selectedCities = $state<string[]>(['thanh_pho_ho_chi_minh']);
  let selectedDistricts = $state<number[]>([]);
  let selectedWards = $state<number[]>([]);
  let citySearchQuery = $state('');
  let showCityDropdown = $state(false);
  let districtFilter = $state('');
  let wardFilter = $state('');

  let filteredCities = $derived.by(() => {
    const q = citySearchQuery.trim().toLowerCase();
    if (!q) return divisions;
    return divisions.filter(p => p.name.toLowerCase().includes(q));
  });

  let availableDistricts = $derived.by(() => {
    let list: { name: string; code: number; provinceName: string; provinceCodename: string; wards: any[] }[] = [];
    for (const cityCodename of selectedCities) {
      const province = divisions.find(p => p.codename === cityCodename);
      if (province) {
        const districts = province.districts || [];
        for (const d of districts) {
          list.push({
            name: d.name,
            code: d.code,
            provinceName: province.name,
            provinceCodename: province.codename,
            wards: d.wards || []
          });
        }
      }
    }
    return list.sort((a, b) => a.provinceName.localeCompare(b.provinceName) || a.name.localeCompare(b.name));
  });

  let filteredDistricts = $derived.by(() => {
    const q = districtFilter.trim().toLowerCase();
    if (!q) return availableDistricts;
    return availableDistricts.filter(d => d.name.toLowerCase().includes(q) || d.provinceName.toLowerCase().includes(q));
  });

  let availableWards = $derived.by(() => {
    let list: { name: string; code: number; districtName: string; districtCode: number; provinceName: string }[] = [];
    for (const districtCode of selectedDistricts) {
      const dist = availableDistricts.find(d => d.code === districtCode);
      if (dist) {
        const wards = dist.wards || [];
        for (const ward of wards) {
          list.push({ 
            name: ward.name, 
            code: ward.code, 
            districtName: dist.name,
            districtCode: dist.code,
            provinceName: dist.provinceName
          });
        }
      }
    }
    return list;
  });

  let filteredWards = $derived.by(() => {
    const q = wardFilter.trim().toLowerCase();
    if (!q) return availableWards;
    return availableWards.filter(w => w.name.toLowerCase().includes(q) || w.districtName.toLowerCase().includes(q));
  });

  function toggleCity(cityCodename: string) {
    if (selectedCities.includes(cityCodename)) {
      selectedCities = selectedCities.filter(c => c !== cityCodename);
      const province = divisions.find(p => p.codename === cityCodename);
      if (province) {
        const districtCodes = (province.districts || []).map(d => d.code);
        selectedDistricts = selectedDistricts.filter(code => !districtCodes.includes(code));
        let wardCodes: number[] = [];
        for (const d of province.districts || []) {
          const wards = d.wards || [];
          wardCodes.push(...wards.map(w => w.code));
        }
        selectedWards = selectedWards.filter(code => !wardCodes.includes(code));
      }
    } else {
      selectedCities = [...selectedCities, cityCodename];
    }
    districtFilter = '';
    wardFilter = '';
  }

  function clearAllCities() {
    selectedCities = [];
    selectedDistricts = [];
    selectedWards = [];
    citySearchQuery = '';
    districtFilter = '';
    wardFilter = '';
  }

  function selectAllDistricts() {
    selectedDistricts = availableDistricts.map(d => d.code);
    selectedWards = [];
    wardFilter = '';
  }

  function deselectAllDistricts() {
    selectedDistricts = [];
    selectedWards = [];
    wardFilter = '';
  }

  function selectAllWards() {
    selectedWards = availableWards.map(w => w.code);
  }

  function deselectAllWards() {
    selectedWards = [];
  }

  function toggleDistrict(districtCode: number, checked: boolean) {
    if (checked) {
      if (!selectedDistricts.includes(districtCode)) {
        selectedDistricts = [...selectedDistricts, districtCode];
      }
    } else {
      const dist = availableDistricts.find(d => d.code === districtCode);
      const wardCodesToRemove = dist ? (dist.wards || []).map(w => w.code) : [];
      selectedDistricts = selectedDistricts.filter(c => c !== districtCode);
      selectedWards = selectedWards.filter(code => !wardCodesToRemove.includes(code));
    }
  }

  async function detectCoordinates() {
    if (!geocodeQuery.trim()) {
      appendLog("Error: Please enter a region or address to search.");
      return;
    }
    isGeocoding = true;
    appendLog(`Geocoding coordinates for: "${geocodeQuery}"...`);
    try {
      const url = `https://nominatim.openstreetmap.org/search?q=${encodeURIComponent(geocodeQuery + ", Vietnam")}&format=json&limit=1`;
      const response = await fetch(url, { headers: { 'User-Agent': 'RustSale-CRM-v2-Client' } });
      if (!response.ok) throw new Error(`HTTP error! status: ${response.status}`);
      const data = await response.json();
      if (data && data.length > 0) {
        const bbox = data[0].boundingbox;
        minLat = parseFloat(bbox[0]);
        maxLat = parseFloat(bbox[1]);
        minLng = parseFloat(bbox[2]);
        maxLng = parseFloat(bbox[3]);
        appendLog(`Successfully detected coordinates for "${data[0].display_name}": Lat: ${minLat}-${maxLat}, Lng: ${minLng}-${maxLng}`);
      } else {
        appendLog(`Error: Could not find coordinates for "${geocodeQuery}".`);
      }
    } catch (err) {
      appendLog(`Geocoding failed: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      isGeocoding = false;
    }
  }

  let isScraping = $state(false);
  let progress = $state(0);
  let statusMessage = $state('Ready');
  let logs = $state<string[]>([]);
  let headless = $state(true);
  let presetProvinces = $state<Record<string, { minLat: number; maxLat: number; minLng: number; maxLng: number }>>({});
  let consoleElement: HTMLDivElement | null = null;

  let unlistenProgress: UnlistenFn | null = null;
  let unlistenLog: UnlistenFn | null = null;
  let unlistenComplete: UnlistenFn | null = null;

  function appendLog(msg: string) {
    const timestamp = new Date().toLocaleTimeString();
    logs = [...logs, `[${timestamp}] ${msg}`];
    setTimeout(() => {
      if (consoleElement) consoleElement.scrollTop = consoleElement.scrollHeight;
    }, 50);
  }

  async function stopScrape() {
    try {
      appendLog("Requesting to stop campaign...");
      await invoke('stop_google_maps_scrape');
      appendLog("Campaign stop command sent successfully.");
    } catch (err) {
      appendLog(`Error stopping campaign: ${err}`);
    }
  }

  async function startScrape() {
    if (!keyword.trim()) {
      appendLog("Error: Keyword is required.");
      return;
    }

    if (mode === 'grid') {
      const latMin = Number(minLat);
      const latMax = Number(maxLat);
      const lngMin = Number(minLng);
      const lngMax = Number(maxLng);
      const step = Number(stepKm);

      if (isNaN(latMin) || isNaN(latMax) || isNaN(lngMin) || isNaN(lngMax) || isNaN(step)) {
        appendLog("Error: Bounding box coordinates and step size must be valid numbers.");
        return;
      }
      if (latMin < -90 || latMin > 90 || latMax < -90 || latMax > 90 ||
          lngMin < -180 || lngMin > 180 || lngMax < -180 || lngMax > 180) {
        appendLog("Error: Latitude must be between -90 and 90, Longitude must be between -180 and 180.");
        return;
      }
      if (latMin >= latMax) {
        appendLog("Error: Min Latitude must be less than Max Latitude.");
        return;
      }
      if (lngMin >= lngMax) {
        appendLog("Error: Min Longitude must be less than Max Longitude.");
        return;
      }
      if (step <= 0) {
        appendLog("Error: Step size must be greater than 0.");
        return;
      }
    }

    let districtsVal: string[] | null = null;
    let locationVal = mode === 'keyword' ? location : null;
    if (mode === 'keyword' && deepScan) {
      if (selectedDistricts.length === 0) {
        appendLog("Error: Please select at least one district for Deep Scan.");
        return;
      }
      locationVal = "Việt Nam";
      if (selectedWards.length > 0) {
        const wardMap = new Map(availableWards.map(w => [w.code, w]));
        districtsVal = selectedWards.map(code => {
          const ward = wardMap.get(code);
          return ward ? `${ward.name}, ${ward.districtName}, ${ward.provinceName}` : '';
        }).filter(Boolean);
      } else {
        const distMap = new Map(availableDistricts.map(d => [d.code, d]));
        districtsVal = selectedDistricts.map(code => {
          const dist = distMap.get(code);
          return dist ? `${dist.name}, ${dist.provinceName}` : '';
        }).filter(Boolean);
      }
    }

    isScraping = true;
    progress = 0;
    statusMessage = 'Initializing scrape...';
    logs = [];
    appendLog(`Starting Google Maps Scraper in ${mode} mode...`);

    try {
      const bbox = mode === 'grid' ? [Number(minLat), Number(maxLat), Number(minLng), Number(maxLng)] : null;
      const stepVal = mode === 'grid' ? Number(stepKm) : null;

      await invoke('start_google_maps_scrape', {
        keyword,
        mode,
        location: locationVal,
        bbox,
        stepKm: stepVal,
        districts: districtsVal,
        headless
      });
      appendLog('Scrape command invoked successfully.');
    } catch (err) {
      isScraping = false;
      statusMessage = 'Failed to start scrape';
      appendLog(`Error: ${err}`);
    }
  }

  let docClickHandler: (e: MouseEvent) => void;

  onMount(async () => {
    try {
      presetProvinces = await invoke('get_preset_provinces');
    } catch (err) {
      appendLog(`Error loading preset provinces: ${err}`);
    }

    unlistenProgress = await listen<{ progress: number; status: string; message: string }>(
      'scrape-progress',
      (event) => {
        progress = event.payload.progress;
        statusMessage = event.payload.message;
        if (event.payload.status === 'completed' || event.payload.status === 'failed') {
          isScraping = false;
        }
      }
    );

    unlistenLog = await listen<{ message: string }>('scrape-log', (event) => {
      appendLog(event.payload.message);
    });

    unlistenComplete = await listen<{ total_scraped: number; message: string }>('scrape-complete', (event) => {
      isScraping = false;
      progress = 100;
      statusMessage = event.payload.message;
      appendLog(`[COMPLETE] ${event.payload.message}`);
    });

    docClickHandler = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      if (!target.closest('.city-multiselect-container')) {
        showCityDropdown = false;
      }
    };
    document.addEventListener('click', docClickHandler);
  });

  onDestroy(() => {
    if (unlistenProgress) unlistenProgress();
    if (unlistenLog) unlistenLog();
    if (unlistenComplete) unlistenComplete();
    if (docClickHandler) document.removeEventListener('click', docClickHandler);
  });
</script>

<div class="flex flex-col gap-4">
  <!-- Top Header Bar -->
  <div class="flex items-center justify-between">
    <div class="flex flex-col">
      <h2 class="text-xl font-bold text-foreground tracking-tight">Google Maps Scraper</h2>
      <p class="text-xs text-muted-foreground mt-0.5">Extract local business lead data dynamically across regions</p>
    </div>
  </div>

  <!-- Main Grid -->
  <div class="grid grid-cols-1 lg:grid-cols-12 gap-5">
    <!-- Left Form Configuration Panel -->
    <Card.Root class="lg:col-span-7 flex flex-col justify-between">
      <Card.Header class="pb-3 border-b border-border">
        <Card.Title class="text-base font-bold flex items-center gap-2">
          <Building class="size-4 text-emerald-600" />
          Scraper Configuration
        </Card.Title>
      </Card.Header>
      <Card.Content class="p-5 flex flex-col gap-4 text-xs">
        <!-- Mode Switcher Tabs -->
        <div class="flex items-center gap-1 p-1 bg-muted rounded-lg w-full">
          <button
            type="button"
            class="flex-1 py-1.5 px-3 rounded-md text-xs font-semibold flex items-center justify-center gap-2 transition-all cursor-pointer {mode === 'keyword' ? 'bg-background text-foreground shadow-xs' : 'text-muted-foreground hover:text-foreground'}"
            onclick={() => mode = 'keyword'}
            disabled={isScraping}
          >
            <Search class="size-3.5" /> Keyword Search
          </button>
          <button
            type="button"
            class="flex-1 py-1.5 px-3 rounded-md text-xs font-semibold flex items-center justify-center gap-2 transition-all cursor-pointer {mode === 'grid' ? 'bg-background text-foreground shadow-xs' : 'text-muted-foreground hover:text-foreground'}"
            onclick={() => mode = 'grid'}
            disabled={isScraping}
          >
            <Grid class="size-3.5" /> Grid Crawl
          </button>
        </div>

        <!-- Search Keyword -->
        <div class="flex flex-col gap-1">
          <label for="keyword-input" class="font-semibold text-foreground">Search Keyword *</label>
          <div class="relative">
            <Search class="size-4 absolute left-3 top-2.5 text-muted-foreground" />
            <Input id="keyword-input" bind:value={keyword} placeholder="e.g. Cafe, Spa, Dental Clinic..." class="pl-9 text-xs" disabled={isScraping} />
          </div>
        </div>

        {#if mode === 'keyword'}
          <!-- Location Targeting Mode Switcher -->
          <div class="flex flex-col gap-1.5">
            <span class="font-semibold text-foreground">Location Targeting Mode</span>
            <div class="flex items-center gap-1 p-1 bg-muted/60 rounded-md">
              <button
                type="button"
                class="flex-1 py-1 px-2.5 rounded text-xs font-medium transition-all cursor-pointer {!deepScan ? 'bg-background text-foreground shadow-2xs font-semibold' : 'text-muted-foreground hover:text-foreground'}"
                onclick={() => deepScan = false}
                disabled={isScraping}
              >
                Text Search (Simple)
              </button>
              <button
                type="button"
                class="flex-1 py-1 px-2.5 rounded text-xs font-medium transition-all cursor-pointer {deepScan ? 'bg-background text-foreground shadow-2xs font-semibold' : 'text-muted-foreground hover:text-foreground'}"
                onclick={() => deepScan = true}
                disabled={isScraping}
              >
                Select by District/Ward (Deep Scan)
              </button>
            </div>
          </div>

          {#if !deepScan}
            <!-- Simple Location Input -->
            <div class="flex flex-col gap-1">
              <label for="location-input" class="font-semibold text-foreground">Location / Area</label>
              <div class="relative">
                <MapPin class="size-4 absolute left-3 top-2.5 text-muted-foreground" />
                <Input id="location-input" bind:value={location} placeholder="e.g. Quận 1, Ho Chi Minh City" class="pl-9 text-xs" disabled={isScraping} />
              </div>
              <p class="text-[11px] text-muted-foreground">Focuses search on a specific neighborhood or town name.</p>
            </div>
          {:else}
            <!-- Deep Scan Configurations -->
            <div class="flex flex-col gap-3 border-t border-border pt-3">
              <!-- Target Cities / Provinces -->
              <div class="flex flex-col gap-2 relative city-multiselect-container">
                <label for="city-search" class="font-semibold text-foreground">Target Cities / Provinces</label>
                <div class="relative">
                  <Search class="size-3.5 absolute left-3 top-2.5 text-muted-foreground" />
                  <Input
                    id="city-search"
                    type="text"
                    placeholder="Search & add cities..."
                    bind:value={citySearchQuery}
                    onfocus={() => showCityDropdown = true}
                    disabled={isScraping}
                    class="pl-8 text-xs h-9"
                  />
                </div>

                {#if selectedCities.length > 0}
                  <div class="flex flex-wrap gap-1.5 pt-1">
                    {#each selectedCities as cityCodename (cityCodename)}
                      {@const cityName = divisions.find(p => p.codename === cityCodename)?.name}
                      <Badge variant="secondary" class="gap-1 text-[11px]">
                        {cityName}
                        <button type="button" onclick={() => toggleCity(cityCodename)} class="hover:text-destructive cursor-pointer">
                          <X class="size-3" />
                        </button>
                      </Badge>
                    {/each}
                    <Button variant="ghost" size="sm" onclick={clearAllCities} class="h-6 text-[10px] px-2 text-muted-foreground">Clear All</Button>
                  </div>
                {/if}

                {#if showCityDropdown}
                  <div class="absolute left-0 right-0 top-full mt-1 z-30 max-h-48 overflow-y-auto rounded-md border border-border bg-popover p-1 shadow-md text-xs flex flex-col gap-0.5">
                    {#each filteredCities as city (city.codename)}
                      {@const isSelected = selectedCities.includes(city.codename)}
                      <button
                        type="button"
                        class="w-full px-3 py-1.5 rounded flex items-center justify-between text-left hover:bg-accent transition-colors cursor-pointer {isSelected ? 'bg-primary/10 text-primary font-semibold' : ''}"
                        onclick={() => toggleCity(city.codename)}
                      >
                        <span>{city.name}</span>
                        {#if isSelected}<Check class="size-3.5 text-primary" />{/if}
                      </button>
                    {/each}
                  </div>
                {/if}
              </div>

              <!-- Districts Selection Grid -->
              <div class="flex flex-col gap-2">
                <div class="flex items-center justify-between text-xs">
                  <span class="font-semibold text-foreground">Select Districts ({selectedDistricts.length} selected)</span>
                  <div class="flex items-center gap-2">
                    <Input type="text" placeholder="Filter districts..." bind:value={districtFilter} class="h-7 w-32 text-[11px]" disabled={isScraping} />
                    <Button variant="ghost" size="sm" class="h-7 text-[10px] px-2" onclick={selectAllDistricts} disabled={isScraping}>Select All</Button>
                    <Button variant="ghost" size="sm" class="h-7 text-[10px] px-2 text-muted-foreground" onclick={deselectAllDistricts} disabled={isScraping}>Clear</Button>
                  </div>
                </div>
                <div class="max-h-36 overflow-y-auto p-2 rounded-md border border-border bg-muted/20 grid grid-cols-2 sm:grid-cols-3 gap-1.5 text-xs">
                  {#each filteredDistricts as district (district.code)}
                    {@const isChecked = selectedDistricts.includes(district.code)}
                    <div 
                      class="flex items-center gap-2 p-1.5 rounded hover:bg-muted/50 cursor-pointer text-[11px] truncate transition-colors {isChecked ? 'bg-primary/5 font-semibold' : ''}"
                      onclick={() => !isScraping && toggleDistrict(district.code, !isChecked)}
                      role="button"
                      tabindex="0"
                      onkeydown={(e) => e.key === 'Enter' && !isScraping && toggleDistrict(district.code, !isChecked)}
                    >
                      <Checkbox
                        checked={isChecked}
                        disabled={isScraping}
                      />
                      <span class="truncate">{district.name} <small class="text-muted-foreground">({district.provinceName.replace('Thành phố ', 'TP. ').replace('Tỉnh ', 'T.')})</small></span>
                    </div>
                  {/each}
                </div>
              </div>

              <!-- Wards Selection Grid -->
              {#if selectedDistricts.length > 0}
                <div class="flex flex-col gap-2 border-t border-border/50 pt-2">
                  <div class="flex items-center justify-between text-xs">
                    <span class="font-semibold text-foreground">Select Wards ({selectedWards.length} selected)</span>
                    <div class="flex items-center gap-2">
                      <Input type="text" placeholder="Filter wards..." bind:value={wardFilter} class="h-7 w-32 text-[11px]" disabled={isScraping} />
                      <Button variant="ghost" size="sm" class="h-7 text-[10px] px-2" onclick={selectAllWards} disabled={isScraping}>Select All</Button>
                      <Button variant="ghost" size="sm" class="h-7 text-[10px] px-2 text-muted-foreground" onclick={deselectAllWards} disabled={isScraping}>Clear</Button>
                    </div>
                  </div>
                  <div class="max-h-36 overflow-y-auto p-2 rounded-md border border-border bg-muted/20 grid grid-cols-2 sm:grid-cols-3 gap-1.5 text-xs">
                    {#each filteredWards as ward (ward.code)}
                      {@const isChecked = selectedWards.includes(ward.code)}
                      <div 
                        class="flex items-center gap-2 p-1.5 rounded hover:bg-muted/50 cursor-pointer text-[11px] truncate transition-colors {isChecked ? 'bg-primary/5 font-semibold' : ''}"
                        onclick={() => {
                          if (isScraping) return;
                          if (isChecked) selectedWards = selectedWards.filter(w => w !== ward.code);
                          else selectedWards = [...selectedWards, ward.code];
                        }}
                        role="button"
                        tabindex="0"
                        onkeydown={(e) => {
                          if (e.key === 'Enter' && !isScraping) {
                            if (isChecked) selectedWards = selectedWards.filter(w => w !== ward.code);
                            else selectedWards = [...selectedWards, ward.code];
                          }
                        }}
                      >
                        <Checkbox
                          checked={isChecked}
                          disabled={isScraping}
                        />
                        <span class="truncate">{ward.name} <small class="text-muted-foreground">({ward.districtName.replace('Quận ', 'Q.').replace('Huyện ', 'H.')})</small></span>
                      </div>
                    {/each}
                  </div>
                </div>
              {/if}
            </div>
          {/if}

        {:else}
          <!-- Grid Mode Settings -->
          <div class="flex flex-col gap-3 border-t border-border pt-3">
            <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
              <div class="flex flex-col gap-1">
                <label for="preset-select" class="font-semibold text-foreground">Quick Select Province / City</label>
                <Select.Root
                  type="single"
                  disabled={isScraping}
                  onValueChange={(val) => {
                    if (val && presetProvinces[val]) {
                      const coords = presetProvinces[val];
                      minLat = coords.minLat;
                      maxLat = coords.maxLat;
                      minLng = coords.minLng;
                      maxLng = coords.maxLng;
                      appendLog(`Applied preset coords for ${val}`);
                    }
                  }}
                >
                  <Select.Trigger class="w-full h-9 text-xs">
                    -- Select Province / City --
                  </Select.Trigger>
                  <Select.Content>
                    {#each Object.keys(presetProvinces) as prov (prov)}
                      <Select.Item value={prov}>{prov}</Select.Item>
                    {/each}
                  </Select.Content>
                </Select.Root>
              </div>

              <div class="flex flex-col gap-1">
                <label for="geocode-query" class="font-semibold text-foreground">Search Administrative Area</label>
                <div class="flex gap-2">
                  <Input id="geocode-query" bind:value={geocodeQuery} placeholder="e.g. Quận 1, TP. HCM" class="text-xs h-9" disabled={isScraping || isGeocoding} />
                  <Button variant="outline" size="sm" onclick={detectCoordinates} disabled={isScraping || isGeocoding} class="h-9 px-3 gap-1 shrink-0">
                    <Locate class="size-3.5" /> {isGeocoding ? 'Detecting...' : 'Detect'}
                  </Button>
                </div>
              </div>
            </div>

            <div class="flex flex-col gap-1.5">
              <span class="font-semibold text-foreground">Bounding Box Coordinates (Lat/Lng)</span>
              <div class="grid grid-cols-2 sm:grid-cols-4 gap-2">
                <div class="flex flex-col gap-1">
                  <label for="min-lat" class="text-[10px] text-muted-foreground">Min Lat</label>
                  <Input id="min-lat" type="number" step="0.0001" bind:value={minLat} disabled={isScraping} class="h-8 text-xs" />
                </div>
                <div class="flex flex-col gap-1">
                  <label for="max-lat" class="text-[10px] text-muted-foreground">Max Lat</label>
                  <Input id="max-lat" type="number" step="0.0001" bind:value={maxLat} disabled={isScraping} class="h-8 text-xs" />
                </div>
                <div class="flex flex-col gap-1">
                  <label for="min-lng" class="text-[10px] text-muted-foreground">Min Lng</label>
                  <Input id="min-lng" type="number" step="0.0001" bind:value={minLng} disabled={isScraping} class="h-8 text-xs" />
                </div>
                <div class="flex flex-col gap-1">
                  <label for="max-lng" class="text-[10px] text-muted-foreground">Max Lng</label>
                  <Input id="max-lng" type="number" step="0.0001" bind:value={maxLng} disabled={isScraping} class="h-8 text-xs" />
                </div>
              </div>
            </div>

            <div class="flex flex-col gap-1">
              <div class="flex justify-between items-center text-xs">
                <label for="step-km" class="font-semibold text-foreground">Grid Step Size</label>
                <span class="font-bold text-emerald-600">{stepKm.toFixed(1)} km</span>
              </div>
              <input id="step-km" type="range" min="0.5" max="10.0" step="0.5" bind:value={stepKm} disabled={isScraping} class="w-full accent-emerald-600 cursor-pointer" />
              <p class="text-[11px] text-muted-foreground">Smaller steps increase accuracy but require more search grid points.</p>
            </div>
          </div>
        {/if}

        <!-- Headless Option & Action Button -->
        <div class="flex items-center justify-between pt-2 border-t border-border">
          <div 
            class="flex items-center gap-2 text-xs font-medium text-muted-foreground cursor-pointer"
            onclick={() => !isScraping && (headless = !headless)}
            role="button"
            tabindex="0"
            onkeydown={(e) => e.key === 'Enter' && !isScraping && (headless = !headless)}
          >
            <Checkbox checked={headless} disabled={isScraping} />
            <span>Headless Mode (Hide browser)</span>
          </div>

          {#if isScraping}
            <Button variant="destructive" size="sm" onclick={stopScrape} class="gap-2">
              <Square class="size-4" /> Stop Scrape
            </Button>
          {:else}
            <Button size="sm" onclick={startScrape} disabled={!keyword.trim()} class="gap-2 bg-emerald-600 hover:bg-emerald-700 text-white">
              <Play class="size-4 fill-white" /> Start Scrape Campaign
            </Button>
          {/if}
        </div>
      </Card.Content>
    </Card.Root>

    <!-- Right Execution Console Panel -->
    <Card.Root class="lg:col-span-5 flex flex-col justify-between">
      <Card.Header class="pb-3 border-b border-border flex flex-row items-center justify-between">
        <Card.Title class="text-base font-bold flex items-center gap-2">
          <Terminal class="size-4 text-emerald-600" /> Execution Logs
        </Card.Title>
        <Badge variant={isScraping ? "default" : "outline"} class="text-[10px]">
          {isScraping ? 'Running' : 'Idle'}
        </Badge>
      </Card.Header>
      <Card.Content class="p-5 flex-1 flex flex-col gap-4">
        <!-- Progress Bar -->
        <div class="flex flex-col gap-1.5">
          <div class="flex items-center justify-between text-xs">
            <span class="font-medium text-foreground">{statusMessage}</span>
            <span class="font-bold text-emerald-600">{Math.round(progress)}%</span>
          </div>
          <Progress value={progress} class="h-2" />
        </div>

        <!-- Console Log Output -->
        <div class="flex-1 min-h-[260px] max-h-[460px] p-3 rounded-lg bg-slate-950 text-slate-100 font-mono text-[11px] overflow-y-auto flex flex-col gap-1 border border-slate-800" bind:this={consoleElement}>
          {#if logs.length === 0}
            <div class="text-slate-500 italic">Console output logs will stream here...</div>
          {:else}
            {#each logs as log, index (index)}
              <div class="leading-relaxed border-b border-slate-900/60 pb-0.5">{log}</div>
            {/each}
          {/if}
        </div>
      </Card.Content>
    </Card.Root>
  </div>
</div>
