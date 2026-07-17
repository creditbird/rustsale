<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { CITY_DISTRICTS, DISTRICT_WARDS } from '$lib/constants/vietnam_regions';

  // State runes
  let mode = $state<'keyword' | 'grid'>('keyword');
  let keyword = $state('');
  let location = $state('');

  // Grid coordinates default to HCM City coordinates
  let minLat = $state(10.7);
  let maxLat = $state(10.8);
  let minLng = $state(106.6);
  let maxLng = $state(106.7);
  let stepKm = $state(2.0);

  const PRESET_PROVINCES: Record<string, { minLat: number; maxLat: number; minLng: number; maxLng: number }> = {
    'TP. Hồ Chí Minh': { minLat: 10.37, maxLat: 11.16, minLng: 106.36, maxLng: 107.03 },
    'Hà Nội': { minLat: 20.58, maxLat: 21.36, minLng: 105.28, maxLng: 106.03 },
    'Đà Nẵng': { minLat: 15.91, maxLat: 16.23, minLng: 107.81, maxLng: 108.34 },
    'Bình Dương': { minLat: 10.89, maxLat: 11.51, minLng: 106.50, maxLng: 106.91 },
    'Đồng Nai': { minLat: 10.74, maxLat: 11.58, minLng: 106.75, maxLng: 107.58 },
    'Cần Thơ': { minLat: 9.91, maxLat: 10.32, minLng: 105.22, maxLng: 105.86 },
    'Hải Phòng': { minLat: 20.53, maxLat: 21.02, minLng: 106.40, maxLng: 107.13 },
    'Long An': { minLat: 10.20, maxLat: 11.02, minLng: 105.50, maxLng: 106.80 },
    'Quảng Ninh': { minLat: 20.67, maxLat: 21.68, minLng: 106.43, maxLng: 108.08 },
    'Khánh Hòa (Nha Trang)': { minLat: 11.72, maxLat: 12.87, minLng: 108.68, maxLng: 109.47 },
    'Lâm Đồng (Đà Lạt)': { minLat: 11.18, maxLat: 12.30, minLng: 107.27, maxLng: 108.78 },
  };

  let geocodeQuery = $state('');
  let isGeocoding = $state(false);

  // Deep Scan states
  let deepScan = $state(false);
  let selectedCity = $state<'hcm' | 'hanoi'>('hcm');
  let selectedDistricts = $state<string[]>([]);

  let selectedWards = $state<string[]>([]);

  // Derived state to get the list of wards under selected districts
  let availableWards = $derived.by(() => {
    let list: { name: string; district: string }[] = [];
    for (const district of selectedDistricts) {
      const wards = DISTRICT_WARDS[district] || [];
      for (const ward of wards) {
        list.push({ name: ward, district });
      }
    }
    return list;
  });

  function selectAllDistricts() {
    selectedDistricts = [...CITY_DISTRICTS[selectedCity]];
    selectedWards = [];
  }

  function deselectAllDistricts() {
    selectedDistricts = [];
    selectedWards = [];
  }

  function selectAllWards() {
    selectedWards = availableWards.map(w => `${w.name}, ${w.district}`);
  }

  function deselectAllWards() {
    selectedWards = [];
  }

  function toggleDistrict(district: string, checked: boolean) {
    if (checked) {
      selectedDistricts = [...selectedDistricts, district];
    } else {
      selectedDistricts = selectedDistricts.filter(d => d !== district);
      // Remove any wards associated with this deselected district
      selectedWards = selectedWards.filter(w => !w.endsWith(`, ${district}`));
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
      const response = await fetch(url, {
        headers: {
          'User-Agent': 'RustSale-CRM-v2-Client'
        }
      });
      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }
      const data = await response.json();
      if (data && data.length > 0) {
        const bbox = data[0].boundingbox; // [minlat, maxlat, minlng, maxlng]
        minLat = parseFloat(bbox[0]);
        maxLat = parseFloat(bbox[1]);
        minLng = parseFloat(bbox[2]);
        maxLng = parseFloat(bbox[3]);
        appendLog(`Successfully detected coordinates for "${data[0].display_name}":`);
        appendLog(`-> Latitude: ${minLat} to ${maxLat}`);
        appendLog(`-> Longitude: ${minLng} to ${maxLng}`);
      } else {
        appendLog(`Error: Could not find coordinates for "${geocodeQuery}". Try a broader name.`);
      }
    } catch (err) {
      appendLog(`Geocoding failed: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      isGeocoding = false;
    }
  }

  // Scrape running states
  let isScraping = $state(false);
  let progress = $state(0);
  let statusMessage = $state('Ready');
  let logs = $state<string[]>([]);
  let consoleElement: HTMLDivElement | null = null;

  let unlistenProgress: UnlistenFn | null = null;
  let unlistenLog: UnlistenFn | null = null;
  let unlistenComplete: UnlistenFn | null = null;

  function appendLog(msg: string) {
    const timestamp = new Date().toLocaleTimeString();
    logs = [...logs, `[${timestamp}] ${msg}`];
    
    // Auto-scroll to bottom of console
    setTimeout(() => {
      if (consoleElement) {
        consoleElement.scrollTop = consoleElement.scrollHeight;
      }
    }, 50);
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

    // If deepScan is true, validate and build targets list
    let districtsVal: string[] | null = null;
    let locationVal = mode === 'keyword' ? location : null;
    if (mode === 'keyword' && deepScan) {
      if (selectedDistricts.length === 0) {
        appendLog("Error: Please select at least one district for Deep Scan.");
        return;
      }
      locationVal = selectedCity === 'hcm' ? 'Hồ Chí Minh' : 'Hà Nội';
      if (selectedWards.length > 0) {
        districtsVal = selectedWards;
      } else {
        districtsVal = selectedDistricts;
      }
    }

    isScraping = true;
    progress = 0;
    statusMessage = 'Initializing scrape...';
    logs = [];
    appendLog(`Starting Google Maps Scraper in ${mode} mode...`);
    appendLog(`Keyword: "${keyword}"`);
    if (deepScan && districtsVal) {
      if (selectedWards.length > 0) {
        appendLog(`Deep Scan active for city: ${locationVal} (${districtsVal.length} wards selected)`);
      } else {
        appendLog(`Deep Scan active for city: ${locationVal} (${districtsVal.length} districts selected)`);
      }
    }

    try {
      const bbox = mode === 'grid' ? [Number(minLat), Number(maxLat), Number(minLng), Number(maxLng)] : null;
      const stepVal = mode === 'grid' ? Number(stepKm) : null;

      await invoke('start_google_maps_scrape', {
        keyword,
        mode,
        location: locationVal,
        bbox,
        stepKm: stepVal,
        districts: districtsVal
      });
      appendLog('Tauri scrape command invoked successfully. Awaiting browser startup...');
    } catch (err) {
      isScraping = false;
      statusMessage = 'Failed to start scrape';
      appendLog(`Error invoking scrape: ${err}`);
    }
  }

  onMount(async () => {
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
  });

  onDestroy(() => {
    if (unlistenProgress) unlistenProgress();
    if (unlistenLog) unlistenLog();
    if (unlistenComplete) unlistenComplete();
  });
</script>

<div class="container animate-fade-in">
  <header class="header">
    <div class="title-section">
      <span class="material-icons scraper-icon">map</span>
      <div>
        <h1>Google Maps Scraper</h1>
        <p class="subtitle">Extract local business leads dynamically with high precision</p>
      </div>
    </div>
  </header>

  <div class="layout-grid">
    <!-- Left Configuration Panel -->
    <div class="card config-card">
      <div class="card-header">
        <h2>Scraper Configuration</h2>
      </div>

      <div class="tabs">
        <button 
          type="button"
          class="tab-btn" 
          class:active={mode === 'keyword'} 
          onclick={() => mode = 'keyword'}
          disabled={isScraping}
        >
          <span class="material-icons">search</span>
          Keyword Search
        </button>
        <button 
          type="button"
          class="tab-btn" 
          class:active={mode === 'grid'} 
          onclick={() => mode = 'grid'}
          disabled={isScraping}
        >
          <span class="material-icons">grid_on</span>
          Grid Crawl
        </button>
      </div>

      <div class="form-content">
        <div class="input-group">
          <label for="keyword-input">Search Keyword <span class="required">*</span></label>
          <div class="input-wrapper">
            <span class="material-icons input-icon">storefront</span>
            <input 
              id="keyword-input"
              type="text" 
              bind:value={keyword} 
              placeholder="e.g. Cafe, Spa, Dental Clinic..." 
              disabled={isScraping} 
            />
          </div>
        </div>

        {#if mode === 'keyword'}
          {#if !deepScan}
            <div class="input-group animate-slide-down">
              <label for="location-input">Location / Area</label>
              <div class="input-wrapper">
                <span class="material-icons input-icon">place</span>
                <input 
                  id="location-input"
                  type="text" 
                  bind:value={location} 
                  placeholder="e.g. Quận 1, Ho Chi Minh City" 
                  disabled={isScraping} 
                />
              </div>
              <p class="input-desc">Focuses search on a specific neighborhood or town name.</p>
            </div>
          {:else}
            <!-- Deep Scan Configuration -->
            <div class="deep-scan-config animate-slide-down">
              <div class="input-group">
                <label for="deep-city-select">Target City</label>
                <select 
                  id="deep-city-select" 
                  bind:value={selectedCity} 
                  disabled={isScraping}
                  onchange={() => { selectedDistricts = []; selectedWards = []; }}
                >
                  <option value="hcm">TP. Hồ Chí Minh</option>
                  <option value="hanoi">Hà Nội</option>
                </select>
              </div>

              <div class="input-group" style="margin-top: var(--spacing-sm);">
                <div class="districts-header">
                  <span class="districts-label">Select Districts to Scan ({selectedDistricts.length} selected)</span>
                  <div class="districts-actions">
                    <button type="button" class="btn-text" onclick={selectAllDistricts} disabled={isScraping}>Select All</button>
                    <span class="divider">|</span>
                    <button type="button" class="btn-text" onclick={deselectAllDistricts} disabled={isScraping}>Clear</button>
                  </div>
                </div>
                
                <div class="districts-grid">
                  {#each CITY_DISTRICTS[selectedCity] as district}
                    <label class="district-checkbox-label">
                      <input 
                        type="checkbox" 
                        value={district} 
                        disabled={isScraping}
                        checked={selectedDistricts.includes(district)}
                        onchange={(e) => {
                          const checked = (e.target as HTMLInputElement).checked;
                          toggleDistrict(district, checked);
                        }}
                      />
                      <span>{district}</span>
                    </label>
                  {/each}
                </div>
              </div>

              {#if selectedDistricts.length > 0}
                <div class="input-group animate-slide-down" style="margin-top: var(--spacing-sm);">
                  <div class="districts-header">
                    <span class="districts-label">Select Wards to Scan ({selectedWards.length} selected / {availableWards.length} available)</span>
                    <div class="districts-actions">
                      <button type="button" class="btn-text" onclick={selectAllWards} disabled={isScraping}>Select All</button>
                      <span class="divider">|</span>
                      <button type="button" class="btn-text" onclick={deselectAllWards} disabled={isScraping}>Clear</button>
                    </div>
                  </div>
                  
                  <div class="districts-grid">
                    {#each availableWards as ward}
                      <label class="district-checkbox-label" title={`${ward.name} (${ward.district})`}>
                        <input 
                          type="checkbox" 
                          value={`${ward.name}, ${ward.district}`} 
                          disabled={isScraping}
                          checked={selectedWards.includes(`${ward.name}, ${ward.district}`)}
                          onchange={(e) => {
                            const checked = (e.target as HTMLInputElement).checked;
                            const val = `${ward.name}, ${ward.district}`;
                            if (checked) {
                              selectedWards = [...selectedWards, val];
                            } else {
                              selectedWards = selectedWards.filter(w => w !== val);
                            }
                          }}
                        />
                        <span class="ward-text">{ward.name} <small class="district-suffix">({ward.district.replace('Quận ', 'Q.').replace('Huyện ', 'H.')})</small></span>
                      </label>
                    {/each}
                  </div>
                </div>
              {/if}
            </div>
          {/if}

          <!-- Deep Scan Toggle -->
          <div class="input-group" style="margin-block: var(--spacing-md) var(--spacing-sm);">
            <label class="toggle-container" style="display: flex; align-items: center; gap: var(--spacing-xs); cursor: pointer; font-size: 13px; font-weight: 600; color: var(--color-on-surface);">
              <input 
                type="checkbox" 
                bind:checked={deepScan} 
                disabled={isScraping}
              />
              <span>Run Deep Scan (Sequential District Search)</span>
            </label>
            <p class="input-desc">Highly recommended for city-wide crawls. Sequentially searches district-by-district to bypass Google's 120-listing limit safely.</p>
          </div>
          {:else}
          <div class="grid-inputs animate-slide-down">
            <div class="helper-section">
              <div class="input-group">
                <label for="preset-select">Quick Select Province / City</label>
                <select 
                  id="preset-select" 
                  disabled={isScraping}
                  onchange={(e) => {
                    const val = (e.target as HTMLSelectElement).value;
                    if (val && PRESET_PROVINCES[val]) {
                      const coords = PRESET_PROVINCES[val];
                      minLat = coords.minLat;
                      maxLat = coords.maxLat;
                      minLng = coords.minLng;
                      maxLng = coords.maxLng;
                      appendLog(`Applied coordinates for preset: ${val}`);
                    }
                  }}
                >
                  <option value="">-- Select a Province / City --</option>
                  {#each Object.keys(PRESET_PROVINCES) as prov}
                    <option value={prov}>{prov}</option>
                  {/each}
                </select>
              </div>

              <div class="input-group">
                <label for="geocode-query">Search Administrative Area (District/Ward/Address)</label>
                <div style="display: flex; gap: var(--spacing-sm);">
                  <input 
                    id="geocode-query"
                    type="text" 
                    placeholder="e.g. Quận 1, TP. HCM" 
                    bind:value={geocodeQuery}
                    disabled={isScraping || isGeocoding}
                    onkeydown={(e) => {
                      if (e.key === 'Enter') {
                        e.preventDefault();
                        detectCoordinates();
                      }
                    }}
                  />
                  <button 
                    type="button" 
                    class="btn-secondary" 
                    onclick={detectCoordinates}
                    disabled={isScraping || isGeocoding}
                  >
                    {#if isGeocoding}Detecting...{:else}Detect{/if}
                  </button>
                </div>
              </div>
            </div>

            <div class="coordinates-title">Bounding Box Coordinates (Lat/Lng)</div>
            
            <div class="coord-grid">
              <div class="input-group">
                <label for="min-lat">Min Latitude</label>
                <input 
                  id="min-lat"
                  type="number" 
                  step="0.0001" 
                  bind:value={minLat} 
                  disabled={isScraping} 
                />
              </div>
              <div class="input-group">
                <label for="max-lat">Max Latitude</label>
                <input 
                  id="max-lat"
                  type="number" 
                  step="0.0001" 
                  bind:value={maxLat} 
                  disabled={isScraping} 
                />
              </div>
              <div class="input-group">
                <label for="min-lng">Min Longitude</label>
                <input 
                  id="min-lng"
                  type="number" 
                  step="0.0001" 
                  bind:value={minLng} 
                  disabled={isScraping} 
                />
              </div>
              <div class="input-group">
                <label for="max-lng">Max Longitude</label>
                <input 
                  id="max-lng"
                  type="number" 
                  step="0.0001" 
                  bind:value={maxLng} 
                  disabled={isScraping} 
                />
              </div>
            </div>

            <div class="input-group step-size-group">
              <div class="slider-header">
                <label for="step-km">Grid Step Size: <strong>{stepKm.toFixed(1)} km</strong></label>
              </div>
              <input 
                id="step-km"
                type="range" 
                min="0.5" 
                max="10.0" 
                step="0.5" 
                bind:value={stepKm} 
                disabled={isScraping} 
                class="slider"
              />
              <p class="input-desc">Smaller steps increase accuracy but require more search grid points.</p>
            </div>
          </div>
        {/if}

        <button 
          type="button"
          class="start-btn" 
          onclick={startScrape} 
          disabled={isScraping || !keyword.trim()}
        >
          {#if isScraping}
            <span class="spinner"></span>
            Scraping...
          {:else}
            <span class="material-icons">play_arrow</span>
            Start Scrape Campaign
          {/if}
        </button>
      </div>
    </div>

    <!-- Right Console & Progress Panel -->
    <div class="card status-card">
      <div class="card-header">
        <h2>Execution & Progress</h2>
        <span class="status-indicator" class:active={isScraping}>
          {isScraping ? 'Running' : 'Idle'}
        </span>
      </div>

      <div class="progress-section">
        <div class="progress-labels">
          <span class="status-msg">{statusMessage}</span>
          <span class="percentage">{Math.round(progress)}%</span>
        </div>
        <div class="progress-bar-container">
          <div class="progress-bar-fill" style="width: {progress}%" class:animated={isScraping}></div>
        </div>
      </div>

      <div class="console-section">
        <div class="console-header">
          <span class="material-icons console-icon">terminal</span>
          <h3>Console Output</h3>
          <button type="button" class="clear-btn" onclick={() => logs = []} disabled={logs.length === 0}>
            Clear
          </button>
        </div>
        <div class="console-body" bind:this={consoleElement}>
          {#if logs.length === 0}
            <div class="console-placeholder">
              <span class="material-icons">insights</span>
              <p>Logs will appear here once scraping starts.</p>
            </div>
          {:else}
            {#each logs as log}
              <div class="log-line">{log}</div>
            {/each}
          {/if}
        </div>
      </div>
    </div>
  </div>
</div>

<style>
  .container {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-lg);
    block-size: 100%;
  }

  .header {
    background: linear-gradient(135deg, var(--color-surface-container) 0%, var(--color-surface-container-low) 100%);
    padding: var(--spacing-md) var(--spacing-lg);
    border-radius: var(--rounded-lg);
    border: 1px solid var(--color-outline-variant);
    box-shadow: var(--shadow-soft);
  }

  .title-section {
    display: flex;
    align-items: center;
    gap: var(--spacing-md);
  }

  .scraper-icon {
    font-size: 36px;
    color: var(--color-success);
    background: rgba(16, 185, 129, 0.1);
    padding: var(--spacing-sm);
    border-radius: var(--rounded-md);
  }

  .subtitle {
    font-size: 13px;
    color: var(--color-on-surface-variant);
    margin-top: 2px;
  }

  .layout-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--spacing-lg);
    flex: 1;
    min-block-size: 0; /* Important for flex child scrolling */
  }

  @media (max-width: 1024px) {
    .layout-grid {
      grid-template-columns: 1fr;
      overflow-y: auto;
    }
  }

  .card {
    background: var(--color-surface-container-lowest);
    border: 1px solid var(--color-outline-variant);
    border-radius: var(--rounded-xl);
    box-shadow: var(--shadow-soft);
    display: flex;
    flex-direction: column;
    padding: var(--spacing-lg);
    min-block-size: 0;
  }

  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: var(--spacing-md);
    border-bottom: 1px solid var(--color-outline-variant);
    padding-bottom: var(--spacing-sm);
  }

  .card-header h2 {
    font-size: 16px;
    font-weight: 600;
  }

  .tabs {
    display: flex;
    gap: var(--spacing-xs);
    background: var(--color-surface-container-low);
    padding: 4px;
    border-radius: var(--rounded-md);
    margin-bottom: var(--spacing-md);
  }

  button:focus-visible {
    outline: 2px solid var(--color-success);
    outline-offset: 2px;
  }

  .tab-btn {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--spacing-sm);
    padding: var(--spacing-sm) var(--spacing-md);
    font-size: 13px;
    font-weight: 500;
    border-radius: var(--rounded-md);
    transition: var(--transition-fast);
    color: var(--color-on-surface-variant);
  }

  .tab-btn:hover:not(:disabled) {
    background: var(--color-surface-container);
    color: var(--color-primary);
  }

  .tab-btn.active {
    background: var(--color-surface-container-lowest);
    color: var(--color-primary);
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.05);
  }

  .tab-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .form-content {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-md);
    flex: 1;
  }

  .input-group {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-xs);
  }

  .input-group label {
    font-size: 12px;
    font-weight: 600;
    color: var(--color-on-surface-variant);
  }

  .required {
    color: var(--color-danger);
  }

  .input-wrapper {
    position: relative;
    display: flex;
    align-items: center;
  }

  .input-icon {
    position: absolute;
    left: 12px;
    color: var(--color-neutral-slate);
    font-size: 18px;
    pointer-events: none;
  }

  .input-wrapper input {
    width: 100%;
    padding: 10px 10px 10px 38px;
    border: 1px solid var(--color-outline);
    border-radius: var(--rounded-md);
    background: var(--color-surface-container-lowest);
    color: var(--color-on-surface);
    font-family: inherit;
    font-size: 14px;
    transition: var(--transition-fast);
  }

  .input-wrapper input:focus {
    border-color: var(--color-success);
    outline: none;
    box-shadow: 0 0 0 3px rgba(16, 185, 129, 0.15);
  }

  .input-wrapper input:disabled {
    background: var(--color-surface-container-low);
    color: var(--color-neutral-slate);
    cursor: not-allowed;
  }

  .input-desc {
    font-size: 11px;
    color: var(--color-neutral-slate);
    margin-top: 2px;
  }

  .grid-inputs {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-md);
    background: var(--color-surface-container-low);
    padding: var(--spacing-md);
    border-radius: var(--rounded-md);
    border: 1px solid var(--color-outline-variant);
  }

  .coordinates-title {
    font-size: 12px;
    font-weight: 700;
    color: var(--color-on-surface);
    margin-bottom: var(--spacing-xs);
  }

  .coord-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--spacing-sm);
  }

  .coord-grid input {
    width: 100%;
    padding: var(--spacing-sm);
    border: 1px solid var(--color-outline);
    border-radius: var(--rounded-sm);
    background: var(--color-surface-container-lowest);
    color: var(--color-on-surface);
    font-size: 13px;
  }

  .coord-grid input:focus {
    border-color: var(--color-success);
    outline: none;
  }

  .step-size-group {
    margin-top: var(--spacing-xs);
  }

  .slider-header {
    display: flex;
    justify-content: space-between;
    font-size: 12px;
  }

  .slider {
    -webkit-appearance: none;
    appearance: none;
    width: 100%;
    height: 6px;
    border-radius: var(--rounded-full);
    background: var(--color-outline-variant);
    outline: none;
    margin-block: var(--spacing-sm);
  }

  .slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--color-success);
    cursor: pointer;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
    transition: transform var(--transition-fast);
  }

  .slider::-webkit-slider-thumb:hover {
    transform: scale(1.15);
  }

  .slider:disabled::-webkit-slider-thumb {
    background: var(--color-neutral-slate);
    cursor: not-allowed;
  }

  .start-btn {
    margin-top: auto;
    width: 100%;
    background: var(--color-primary);
    color: var(--color-on-primary);
    padding: var(--spacing-md);
    border-radius: var(--rounded-md);
    font-weight: 600;
    font-size: 14px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--spacing-sm);
    transition: var(--transition-fast);
  }

  .start-btn:hover:not(:disabled) {
    background: var(--color-primary-container);
    box-shadow: 0 4px 6px rgba(15, 23, 42, 0.15);
  }

  .start-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  /* Status Card & Execution elements */
  .status-indicator {
    font-size: 12px;
    font-weight: 600;
    padding: var(--spacing-xs) var(--spacing-sm);
    border-radius: var(--rounded-full);
    background: var(--color-surface-container-low);
    color: var(--color-neutral-slate);
    transition: var(--transition-fast);
  }

  .status-indicator.active {
    background: rgba(16, 185, 129, 0.15);
    color: var(--color-success);
  }

  .progress-section {
    background: var(--color-surface-container-low);
    border: 1px solid var(--color-outline-variant);
    padding: var(--spacing-md);
    border-radius: var(--rounded-md);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-sm);
    margin-bottom: var(--spacing-lg);
  }

  .progress-labels {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .status-msg {
    font-size: 13px;
    font-weight: 500;
    color: var(--color-on-surface);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 80%;
  }

  .percentage {
    font-size: 14px;
    font-weight: 700;
    color: var(--color-success);
  }

  .progress-bar-container {
    height: 10px;
    background: var(--color-surface-container-highest);
    border-radius: var(--rounded-full);
    overflow: hidden;
    position: relative;
  }

  .progress-bar-fill {
    height: 100%;
    background: linear-gradient(90deg, var(--color-success) 0%, #34d399 100%);
    border-radius: var(--rounded-full);
    transition: width var(--transition-fast) ease-out;
  }

  .progress-bar-fill.animated {
    background-image: linear-gradient(
      45deg,
      rgba(255, 255, 255, 0.15) 25%,
      transparent 25%,
      transparent 50%,
      rgba(255, 255, 255, 0.15) 50%,
      rgba(255, 255, 255, 0.15) 75%,
      transparent 75%,
      transparent
    );
    background-size: 1rem 1rem;
    animation: progress-bar-stripes 1s linear infinite;
  }

  @keyframes progress-bar-stripes {
    from { background-position: 1rem 0; }
    to { background-position: 0 0; }
  }

  /* Console Styling */
  .console-section {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-block-size: 0;
    background: #0f172a; /* Slate 900 for dark retro console */
    border-radius: var(--rounded-md);
    overflow: hidden;
    border: 1px solid #1e293b;
    box-shadow: inset 0 2px 8px rgba(0, 0, 0, 0.3);
  }

  .console-header {
    background: #1e293b;
    padding: var(--spacing-sm) var(--spacing-md);
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
    border-bottom: 1px solid #334155;
  }

  .console-icon {
    font-size: 18px;
    color: #38bdf8;
  }

  .console-header h3 {
    font-size: 12px;
    font-weight: 700;
    color: #e2e8f0;
    margin: 0;
    flex: 1;
  }

  .clear-btn {
    font-size: 11px;
    font-weight: 600;
    color: #94a3b8;
    background: #334155;
    padding: var(--spacing-xs) var(--spacing-sm);
    border-radius: var(--rounded-sm);
    transition: var(--transition-fast);
  }

  .clear-btn:hover:not(:disabled) {
    color: #f1f5f9;
    background: #475569;
  }

  .clear-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .console-body {
    flex: 1;
    overflow-y: auto;
    padding: var(--spacing-md);
    font-family: Menlo, Monaco, Consolas, "Fira Code", monospace;
    font-size: 12px;
    line-height: 1.5;
    color: #34d399; /* Green text */
    display: flex;
    flex-direction: column;
    gap: var(--spacing-xs);
  }

  .console-placeholder {
    margin: auto;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--spacing-sm);
    color: #64748b;
    text-align: center;
  }

  .console-placeholder .material-icons {
    font-size: 32px;
  }

  .log-line {
    word-break: break-all;
    white-space: pre-wrap;
  }

  .spinner {
    width: 16px;
    height: 16px;
    border: 2px solid rgba(255, 255, 255, 0.3);
    border-top-color: var(--color-on-primary);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  /* Transition animations */
  .animate-fade-in {
    animation: fadeIn 0.3s ease-out forwards;
  }

  .animate-slide-down {
    animation: slideDown 0.25s ease-out forwards;
  }

  @keyframes fadeIn {
    from { opacity: 0; transform: translateY(4px); }
    to { opacity: 1; transform: translateY(0); }
  }

  @keyframes slideDown {
    from { opacity: 0; transform: translateY(-8px); }
    to { opacity: 1; transform: translateY(0); }
  }

  .helper-section {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--spacing-md);
    margin-bottom: var(--spacing-sm);
    padding-bottom: var(--spacing-md);
    border-bottom: 1px dashed var(--color-outline-variant);
  }
  .helper-section select,
  .helper-section input {
    width: 100%;
    padding: var(--spacing-sm) var(--spacing-md);
    border: 1px solid var(--color-outline);
    border-radius: var(--rounded-md);
    background: var(--color-surface-container-lowest);
    color: var(--color-on-surface);
    font-size: 13px;
    height: 38px;
    outline: none;
    transition: var(--transition-fast);
  }
  .helper-section select:focus,
  .helper-section input:focus {
    border-color: var(--color-success);
    box-shadow: 0 0 0 2px rgba(16, 185, 129, 0.1);
  }
  .btn-secondary {
    background: var(--color-surface-container-high);
    color: var(--color-on-surface);
    border: 1px solid var(--color-outline);
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    border-radius: var(--rounded-md);
    transition: var(--transition-fast);
    height: 38px;
    padding: 0 var(--spacing-md);
  }
  .btn-secondary:hover:not(:disabled) {
    background: var(--color-surface-container-highest);
  }
  .btn-secondary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .deep-scan-config {
    background: var(--color-surface-container-low);
    padding: var(--spacing-md);
    border-radius: var(--rounded-md);
    border: 1px solid var(--color-outline-variant);
    margin-bottom: var(--spacing-sm);
  }
  .deep-scan-config select {
    width: 100%;
    padding: var(--spacing-sm) var(--spacing-md);
    border: 1px solid var(--color-outline);
    border-radius: var(--rounded-md);
    background: var(--color-surface-container-lowest);
    color: var(--color-on-surface);
    font-size: 13px;
    height: 38px;
    outline: none;
    transition: var(--transition-fast);
  }
  .deep-scan-config select:focus {
    border-color: var(--color-success);
  }
  .districts-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: var(--spacing-xs);
  }
  .districts-label {
    font-size: 12px;
    font-weight: 700;
    color: var(--color-on-surface);
  }
  .districts-actions {
    display: flex;
    align-items: center;
    gap: var(--spacing-xs);
  }
  .btn-text {
    background: none;
    border: none;
    color: var(--color-success);
    font-size: 11px;
    font-weight: 700;
    cursor: pointer;
    padding: 0;
    transition: var(--transition-fast);
  }
  .btn-text:hover:not(:disabled) {
    text-decoration: underline;
  }
  .btn-text:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .divider {
    color: var(--color-outline);
    font-size: 11px;
  }
  .districts-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
    gap: var(--spacing-xs);
    max-height: 180px;
    overflow-y: auto;
    border: 1px solid var(--color-outline-variant);
    padding: var(--spacing-sm);
    border-radius: var(--rounded-sm);
    background: var(--color-surface-container-lowest);
  }
  .district-checkbox-label {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--color-on-surface-variant);
    cursor: pointer;
    user-select: none;
  }
  .district-checkbox-label input[type="checkbox"] {
    width: auto;
    height: auto;
    cursor: pointer;
  }
  .toggle-container input[type="checkbox"] {
    width: auto;
    height: auto;
    cursor: pointer;
  }
  .district-suffix {
    color: var(--color-on-surface-variant);
    font-size: 10px;
    opacity: 0.8;
  }
  .ward-text {
    display: inline-block;
    max-width: 100px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
