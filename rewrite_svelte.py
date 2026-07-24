import re

with open('src/lib/components/ZaloOutreach.svelte', 'r') as f:
    content = f.read()

# Replace selectedImei with selectedImeis array
content = content.replace("let selectedImei = $state('');", "let selectedImeis = $state<string[]>([]);")

# Add category and segmentation states
state_addition = """
  // Category and Segmentation targeting
  import { getCategories, getCampaignLeads, type Category } from '$lib/services/crmService';
  let categories = $state<Category[]>([]);
  let selectedCategoryId = $state<number | null>(null);
  let segmentationFilter = $state('all');
  let filterTemplateId = $state<number | null>(null);
"""
content = content.replace("let selectedLeadIds = $state<number[]>([]);", "let selectedLeadIds = $state<number[]>([]);\n" + state_addition)

content = content.replace("import { getLeads, type Lead } from '$lib/services/crmService';", "import { getLeads, type Lead } from '$lib/services/crmService';") # already there, but we added getCategories above

# Fix loadSessions to not use selectedImei
content = content.replace("""      if (sessions.length > 0 && !selectedImei) {
        selectedImei = sessions[0].imei;
      }""", """      if (sessions.length > 0 && selectedImeis.length === 0) {
        selectedImeis = [sessions[0].imei];
      }""")

# Add loadCategories
load_cat = """
  async function loadCategories() {
    try {
      categories = await getCategories();
    } catch (e) {
      console.error('Error loading categories:', e);
    }
  }
"""
content = content.replace("onMount(() => {", load_cat + "\n  onMount(() => {\n    loadCategories();")

# Replace loadLeadsData effect
effect_block = """
  $effect(() => {
    if (selectedCategoryId !== null) {
      getCampaignLeads(selectedCategoryId, segmentationFilter, filterTemplateId)
        .then(res => {
          leads = res;
        })
        .catch(err => {
          console.error('Error fetching campaign leads', err);
          leads = [];
        });
    } else {
      leads = [];
    }
  });
"""
content = content.replace("onMount(() => {", effect_block + "\n  onMount(() => {")

# Update startCampaign
old_start_checks = """    if (!selectedImei) {
      alert('Please select an active Zalo session first.');
      return;
    }"""
new_start_checks = """    if (selectedImeis.length === 0) {
      alert('Please select at least one active Zalo session first.');
      return;
    }
    if (selectedCategoryId === null) {
      alert('Please select a target category first.');
      return;
    }"""
content = content.replace(old_start_checks, new_start_checks)

content = content.replace("'outreach-progress'", "'campaign-progress'")
content = content.replace("'outreach-log'", "'campaign-log'")
content = content.replace("'outreach-complete'", "'campaign-complete'")

old_invoke = """      await invoke('send_zalo_outreach', {
        imei: selectedImei,
        leadIds: selectedLeadIds,
        templateId: selectedTemplateId
      });"""
new_invoke = """      await invoke('send_zalo_campaign', {
        categoryId: selectedCategoryId,
        sessionImeis: selectedImeis,
        templateId: selectedTemplateId
      });"""
content = content.replace(old_invoke, new_invoke)

# Replace step 1 UI
old_step1 = """          <!-- Step 1: Select Session -->
          <div class="input-group">
            <label for="campaign-session">Step 1: Select Active Zalo Session</label>
            <div class="select-wrapper">
              <select id="campaign-session" bind:value={selectedImei} disabled={isCampaignRunning}>
                <option value="" disabled>-- Choose Session --</option>
                {#each sessions as s}
                  <option value={s.imei}>IMEI: {s.imei}</option>
                {/each}
              </select>
            </div>
          </div>"""
new_step1 = """          <!-- Step 1: Select Session(s) -->
          <div class="input-group">
            <label>Step 1: Select Active Zalo Session(s)</label>
            <div class="checkbox-group" style="display: flex; gap: 10px; flex-wrap: wrap;">
              {#each sessions as s}
                <label style="display: flex; align-items: center; gap: 5px;">
                  <input type="checkbox" value={s.imei} bind:group={selectedImeis} disabled={isCampaignRunning} />
                  IMEI: {s.imei}
                </label>
              {/each}
            </div>
          </div>
          
          <!-- Category Selection -->
          <div class="input-group">
            <label for="campaign-category">Target Category</label>
            <div class="select-wrapper">
              <select id="campaign-category" bind:value={selectedCategoryId} disabled={isCampaignRunning}>
                <option value={null} disabled>-- Choose Category --</option>
                {#each categories as cat}
                  <option value={cat.id}>{cat.name}</option>
                {/each}
              </select>
            </div>
          </div>
          
          <!-- Segmentation Filters -->
          <div class="input-group">
            <label>Campaign Segmentation Filter</label>
            <div class="radio-group" style="display: flex; flex-direction: column; gap: 5px;">
              <label><input type="radio" value="all" bind:group={segmentationFilter} disabled={isCampaignRunning} /> All Leads in Category</label>
              <label><input type="radio" value="never_sent" bind:group={segmentationFilter} disabled={isCampaignRunning} /> Never Sent Successfully</label>
              <label><input type="radio" value="last_failed" bind:group={segmentationFilter} disabled={isCampaignRunning} /> Last Attempt Failed</label>
              <label><input type="radio" value="template_success" bind:group={segmentationFilter} disabled={isCampaignRunning} /> Successful Template Match</label>
            </div>
          </div>
          
          {#if segmentationFilter === 'template_success'}
          <div class="input-group" style="margin-left: 20px;">
            <label for="filter-template">Filter by Success Template</label>
            <div class="select-wrapper">
              <select id="filter-template" bind:value={filterTemplateId} disabled={isCampaignRunning}>
                <option value={null} disabled>-- Choose Template --</option>
                {#each templates as t}
                  <option value={t.id}>{t.name}</option>
                {/each}
              </select>
            </div>
          </div>
          {/if}
"""
content = content.replace(old_step1, new_step1)

# fix selection in sessions list
content = content.replace("class:selected={selectedImei === s.imei}", "class:selected={selectedImeis.includes(s.imei)}")
content = content.replace("onclick={() => selectedImei = s.imei}", "onclick={() => { if (selectedImeis.includes(s.imei)) selectedImeis = selectedImeis.filter(i => i !== s.imei); else selectedImeis = [...selectedImeis, s.imei]; }}")
content = content.replace("""      if (selectedImei === imei) {
        selectedImei = '';
      }""", """      selectedImeis = selectedImeis.filter(i => i !== imei);""")

content = content.replace("disabled={isCampaignRunning || !selectedImei || selectedTemplateId === null || selectedLeadIds.length === 0}", "disabled={isCampaignRunning || selectedImeis.length === 0 || selectedTemplateId === null || selectedCategoryId === null || selectedLeadIds.length === 0}")


with open('src/lib/components/ZaloOutreach.svelte', 'w') as f:
    f.write(content)

