<script lang="ts">
  import { onMount } from 'svelte';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Badge } from '$lib/components/ui/badge';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import * as Table from '$lib/components/ui/table';
  import {
    Upload,
    FileSpreadsheet,
    FileText,
    CheckCircle2,
    AlertCircle,
    ArrowRight,
    ArrowLeft,
    Download,
    RefreshCw,
    X,
    FileUp,
    Check,
    AlertTriangle
  } from 'lucide-svelte';
  import { toast } from 'svelte-sonner';
  import * as crmService from '$lib/services/crmService';
  import {
    pickLeadImportFile,
    inspectLeadImportFile,
    executeLeadImport,
    downloadLeadImportTemplate,
    autoDetectColumnMapping,
    buildErrorRowsCsv,
    type ImportInspection,
    type ColumnMapping,
    type ConflictStrategy,
    type ImportProgress,
    type ImportSummaryResult
  } from '$lib/services/leadImportService';
  import { exportLeadsToCsv } from '$lib/utils/leadsExportCsv';
  import { onDragDropEvent } from '$lib/services/conversationPlatform';
  import { m } from '$lib/i18n';

  let {
    open = $bindable(false),
    onSuccess
  } = $props<{
    open: boolean;
    onSuccess?: () => void;
  }>();

  // Wizard steps: 1: Upload, 2: Mapping, 3: Config, 4: Execution/Results
  let currentStep = $state<1 | 2 | 3 | 4>(1);

  // File and inspection state
  let selectedFilePath = $state<string | null>(null);
  let fileName = $state<string>('');
  let inspecting = $state<boolean>(false);
  let inspection = $state<ImportInspection | null>(null);
  let selectedSheet = $state<string>('');
  let isDragOver = $state<boolean>(false);

  // Column mapping state
  let mapping = $state<ColumnMapping>({
    name_col: null,
    phone_col: null,
    email_col: null,
    company_col: null,
    value_col: null,
    address_col: null,
    about_col: null,
    status_col: null,
    owner_col: null
  });

  // Config state
  let conflictStrategy = $state<ConflictStrategy>('skip');
  let normalizePhone = $state<boolean>(true);
  let selectedCategoryId = $state<number | null>(null);
  let defaultStatus = $state<string>('New');
  let defaultOwner = $state<string>('Self');
  let defaultCurrency = $state<'VND' | 'USD'>('VND');
  let categories = $state<crmService.Category[]>([]);

  // Execution & Progress state
  let importing = $state<boolean>(false);
  let progress = $state<ImportProgress>({ processed_rows: 0, total_rows: 0, percentage: 0 });
  let summaryResult = $state<ImportSummaryResult | null>(null);
  let errorMessage = $state<string | null>(null);
  let inspectionGeneration = 0;

  onMount(async () => {
    try {
      categories = await crmService.getCategories();
    } catch (e) {
      console.error('Failed to load categories', e);
    }
  });

  // Reset modal state when closed
  function handleClose() {
    if (importing) return;
    inspectionGeneration++;
    inspecting = false;
    open = false;
    currentStep = 1;
    selectedFilePath = null;
    fileName = '';
    inspection = null;
    summaryResult = null;
    errorMessage = null;
    defaultCurrency = 'VND';
    progress = { processed_rows: 0, total_rows: 0, percentage: 0 };
  }

  // Handle native file picking
  async function handlePickFile() {
    try {
      errorMessage = null;
      const path = await pickLeadImportFile();
      if (!path) return;
      await processSelectedFilePath(path);
    } catch (err: any) {
      const msg = err?.message || String(err);
      errorMessage = msg;
      toast.error(msg);
    }
  }

  // Native Tauri drag-drop handler (R4)
  $effect(() => {
    if (!open || currentStep !== 1) return;

    const unlisten = onDragDropEvent(
      (paths) => {
        isDragOver = false;
        if (!paths || paths.length === 0) return;
        const valid = paths.find((p) => /\.(xlsx|xls|csv)$/i.test(p));
        if (valid) {
          void processSelectedFilePath(valid);
        } else {
          toast.error('Chỉ hỗ trợ tệp định dạng .xlsx, .xls, .csv');
        }
      },
      (active) => {
        if (currentStep === 1) {
          isDragOver = active;
        }
      }
    );

    return () => {
      unlisten();
    };
  });

  // Handle Drag and Drop (HTML5 DOM fallback)
  function handleDragOver(e: DragEvent) {
    e.preventDefault();
    isDragOver = true;
  }

  function handleDragLeave() {
    isDragOver = false;
  }

  async function handleDrop(e: DragEvent) {
    e.preventDefault();
    isDragOver = false;

    const files = e.dataTransfer?.files;
    if (!files || files.length === 0) return;

    const file = files[0];
    const path = (file as any).path || file.name;
    if (path) {
      await processSelectedFilePath(path);
    }
  }

  async function processSelectedFilePath(path: string) {
    const currentGen = ++inspectionGeneration;
    selectedFilePath = path;
    const parts = path.split(/[/\\]/);
    fileName = parts[parts.length - 1] || path;
    inspecting = true;
    errorMessage = null;

    try {
      const insp = await inspectLeadImportFile(path);
      if (currentGen !== inspectionGeneration) return;
      inspection = insp;
      selectedSheet = insp.selected_sheet || insp.sheets[0] || '';

      // Auto-detect mappings
      mapping = autoDetectColumnMapping(insp.headers);
    } catch (err: any) {
      if (currentGen !== inspectionGeneration) return;
      const msg = err?.message || String(err);
      errorMessage = msg;
      inspection = null;
      toast.error(msg);
    } finally {
      if (currentGen === inspectionGeneration) {
        inspecting = false;
      }
    }
  }

  async function handleSheetChange(newSheet: string) {
    if (!selectedFilePath) return;
    const currentGen = ++inspectionGeneration;
    selectedSheet = newSheet;
    inspecting = true;
    try {
      const insp = await inspectLeadImportFile(selectedFilePath, newSheet);
      if (currentGen !== inspectionGeneration) return;
      inspection = insp;
      mapping = autoDetectColumnMapping(insp.headers);
    } catch (err: any) {
      if (currentGen !== inspectionGeneration) return;
      const msg = err?.message || String(err);
      errorMessage = msg;
      inspection = null;
      mapping = {
        name_col: null,
        phone_col: null,
        email_col: null,
        company_col: null,
        value_col: null,
        address_col: null,
        about_col: null,
        status_col: null,
        owner_col: null
      };
      toast.error(msg);
    } finally {
      if (currentGen === inspectionGeneration) {
        inspecting = false;
      }
    }
  }

  async function handleDownloadTemplate(format: 'xlsx' | 'csv') {
    try {
      const savedPath = await downloadLeadImportTemplate(format);
      if (savedPath) {
        toast.success(`Đã lưu tệp mẫu tại: ${savedPath}`);
      }
    } catch (err: any) {
      toast.error(err?.message || 'Không thể tải tệp mẫu');
    }
  }

  // Step 2 validation: Inspection must be valid and Full name column is required
  let isMappingValid = $derived(inspection !== null && mapping.name_col !== null && mapping.name_col !== undefined);

  // Start execution
  async function handleStartImport() {
    if (!selectedFilePath || !inspection) return;

    currentStep = 4;
    importing = true;
    errorMessage = null;
    progress = { processed_rows: 0, total_rows: inspection.total_rows_estimate, percentage: 0 };

    try {
      const result = await executeLeadImport(
        {
          file_path: selectedFilePath,
          sheet_name: selectedSheet,
          has_header: true,
          header_row_index: 0,
          mapping,
          conflict_strategy: conflictStrategy,
          default_category_id: selectedCategoryId,
          default_status: defaultStatus,
          default_owner: defaultOwner,
          default_currency: defaultCurrency,
          normalize_phone: normalizePhone
        },
        (prog) => {
          progress = prog;
        }
      );

      summaryResult = result;
      toast.success(m.leads_import_success_title());
      crmService.notifyLeadsChanged();
      if (onSuccess) {
        onSuccess();
      }
    } catch (err: any) {
      errorMessage = err?.message || String(err);
      toast.error(`Lỗi nhập dữ liệu: ${errorMessage}`);
    } finally {
      importing = false;
    }
  }

  // Download error rows CSV
  async function handleDownloadErrorRows() {
    if (!summaryResult || !inspection || summaryResult.error_rows.length === 0) return;

    const csvContent = buildErrorRowsCsv(inspection.headers, summaryResult.error_rows);
    try {
      // Use browser blob download or Tauri native export
      const blob = new Blob([csvContent], { type: 'text/csv;charset=utf-8;' });
      const url = URL.createObjectURL(blob);
      const link = document.createElement('a');
      link.href = url;
      link.download = `RustSale_Import_Errors_${Date.now()}.csv`;
      link.click();
      URL.revokeObjectURL(url);
      toast.success('Đã tải xuống tệp danh sách lỗi');
    } catch (e: any) {
      toast.error('Lỗi khi tải tệp lỗi: ' + (e?.message || e));
    }
  }
</script>

{#if open}
  <div class="fixed inset-0 z-50 bg-background/80 backdrop-blur-sm flex items-center justify-center p-4">
    <div class="w-full max-w-3xl max-h-[90vh] rounded-xl border border-border bg-card p-6 shadow-2xl flex flex-col gap-4 overflow-hidden animate-in fade-in zoom-in-95 duration-200">
      
      <!-- Top Header -->
      <div class="flex items-center justify-between border-b border-border pb-3">
        <div class="flex items-center gap-2.5">
          <div class="p-2 rounded-lg bg-primary/10 text-primary">
            <FileSpreadsheet class="size-5" />
          </div>
          <div>
            <h3 class="text-base font-bold text-foreground">{m.leads_import_modal_title()}</h3>
            <p class="text-xs text-muted-foreground">
              {#if currentStep === 1}
                {m.leads_import_step_upload()}
              {:else if currentStep === 2}
                {m.leads_import_step_mapping()}
              {:else if currentStep === 3}
                {m.leads_import_step_config()}
              {:else}
                {m.leads_import_step_progress()}
              {/if}
            </p>
          </div>
        </div>
        <Button variant="ghost" size="sm" onclick={handleClose} disabled={importing} class="size-8 p-0">
          <X class="size-4" />
        </Button>
      </div>

      <!-- Step Indicator Tracker -->
      <div class="grid grid-cols-4 gap-2 text-center text-xs font-medium pb-1">
        <div class={`p-2 rounded-md transition-colors ${currentStep === 1 ? 'bg-primary text-primary-foreground font-semibold' : currentStep > 1 ? 'bg-primary/20 text-foreground' : 'bg-muted text-muted-foreground'}`}>
          {m.leads_import_step_upload()}
        </div>
        <div class={`p-2 rounded-md transition-colors ${currentStep === 2 ? 'bg-primary text-primary-foreground font-semibold' : currentStep > 2 ? 'bg-primary/20 text-foreground' : 'bg-muted text-muted-foreground'}`}>
          {m.leads_import_step_mapping()}
        </div>
        <div class={`p-2 rounded-md transition-colors ${currentStep === 3 ? 'bg-primary text-primary-foreground font-semibold' : currentStep > 3 ? 'bg-primary/20 text-foreground' : 'bg-muted text-muted-foreground'}`}>
          {m.leads_import_step_config()}
        </div>
        <div class={`p-2 rounded-md transition-colors ${currentStep === 4 ? 'bg-primary text-primary-foreground font-semibold' : 'bg-muted text-muted-foreground'}`}>
          {m.leads_import_step_progress()}
        </div>
      </div>

      <!-- Error Alert Message -->
      {#if errorMessage}
        <div class="p-3 rounded-lg bg-destructive/10 border border-destructive/30 text-destructive text-xs flex items-center gap-2">
          <AlertCircle class="size-4 shrink-0" />
          <span>{errorMessage}</span>
        </div>
      {/if}

      <!-- Modal Body Content -->
      <div class="flex-1 overflow-y-auto pr-1 flex flex-col gap-4 text-xs min-h-[300px]">
        
        <!-- STEP 1: Upload & Inspect -->
        {#if currentStep === 1}
          <!-- Drag & Drop Zone -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            ondragover={handleDragOver}
            ondragleave={handleDragLeave}
            ondrop={handleDrop}
            class={`border-2 border-dashed rounded-xl p-8 flex flex-col items-center justify-center text-center gap-3 transition-colors ${
              isDragOver
                ? 'border-primary bg-primary/10'
                : selectedFilePath
                ? 'border-emerald-500/50 bg-emerald-500/5'
                : 'border-border bg-muted/30 hover:bg-muted/50'
            }`}
          >
            <div class={`p-3 rounded-full ${selectedFilePath ? 'bg-emerald-500/20 text-emerald-600' : 'bg-primary/10 text-primary'}`}>
              <Upload class="size-6" />
            </div>
            <div>
              <p class="font-semibold text-foreground text-sm">
                {#if selectedFilePath}
                  {fileName}
                {:else}
                  {m.leads_import_drag_drop_hint()}
                {/if}
              </p>
              <p class="text-xs text-muted-foreground mt-1">Hỗ trợ các tệp: .xlsx, .xls, .csv</p>
            </div>
            <Button size="sm" variant="outline" onclick={handlePickFile} disabled={inspecting} class="gap-2 mt-2">
              <FileUp class="size-4" />
              {m.leads_import_choose_file_btn()}
            </Button>
          </div>

          <!-- Download Templates Banner -->
          <div class="p-3.5 rounded-lg border border-border bg-muted/40 flex flex-wrap items-center justify-between gap-3">
            <div class="flex items-center gap-2">
              <FileText class="size-4 text-muted-foreground" />
              <span class="text-muted-foreground font-medium">{m.leads_import_download_template_hint()}</span>
            </div>
            <div class="flex items-center gap-2">
              <Button variant="ghost" size="sm" onclick={() => handleDownloadTemplate('xlsx')} class="h-8 gap-1.5 text-xs text-primary hover:bg-primary/10">
                <Download class="size-3.5" />
                {m.leads_import_download_excel_template()}
              </Button>
              <span class="text-border">|</span>
              <Button variant="ghost" size="sm" onclick={() => handleDownloadTemplate('csv')} class="h-8 gap-1.5 text-xs text-primary hover:bg-primary/10">
                <Download class="size-3.5" />
                {m.leads_import_download_csv_template()}
              </Button>
            </div>
          </div>

          <!-- Sheet Selector (if multi-sheet Excel) & File Stats -->
          {#if inspection}
            <div class="p-4 rounded-lg border border-border bg-card flex flex-col gap-3">
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-2">
                  <Badge variant="outline" class="uppercase font-mono text-[10px]">
                    {inspection.file_type}
                  </Badge>
                  <span class="font-medium text-foreground">
                    Ước tính: {inspection.total_rows_estimate} dòng dữ liệu
                  </span>
                </div>

                {#if inspection.sheets.length > 1}
                  <div class="flex items-center gap-2">
                    <span class="text-muted-foreground">{m.leads_import_sheet_label()}:</span>
                    <select
                      bind:value={selectedSheet}
                      onchange={(e) => handleSheetChange((e.target as HTMLSelectElement).value)}
                      class="h-8 px-2 rounded-md border border-input bg-background text-xs text-foreground focus:outline-hidden"
                    >
                      {#each inspection.sheets as sheet (sheet)}
                        <option value={sheet}>{sheet}</option>
                      {/each}
                    </select>
                  </div>
                {/if}
              </div>

              <!-- Preview Table of Sample Rows -->
              <div class="flex flex-col gap-1.5">
                <span class="text-xs font-semibold text-muted-foreground">{m.leads_import_preview_sample_title()}</span>
                <div class="border border-border rounded-md overflow-x-auto max-h-48">
                  <table class="w-full text-left text-xs border-collapse">
                    <thead class="bg-muted/70 sticky top-0 border-b border-border">
                      <tr>
                        {#each inspection.headers as header, hIdx (hIdx)}
                          <th class="p-2 font-semibold text-foreground whitespace-nowrap">{header}</th>
                        {/each}
                      </tr>
                    </thead>
                    <tbody>
                      {#each inspection.sample_rows.slice(0, 5) as row, rIdx (rIdx)}
                        <tr class="border-b border-border/50 hover:bg-muted/30">
                          {#each row as cell, cIdx (cIdx)}
                            <td class="p-2 whitespace-nowrap text-muted-foreground">{cell || '—'}</td>
                          {/each}
                        </tr>
                      {/each}
                    </tbody>
                  </table>
                </div>
              </div>
            </div>
          {/if}

        <!-- STEP 2: Column Mapping -->
        {:else if currentStep === 2 && inspection}
          <div class="flex flex-col gap-3">
            <p class="text-muted-foreground">{m.leads_import_mapping_desc()}</p>

            <div class="border border-border rounded-lg overflow-hidden">
              <table class="w-full text-left text-xs border-collapse">
                <thead class="bg-muted/80 border-b border-border">
                  <tr>
                    <th class="p-2.5 font-semibold text-foreground w-1/3">Trường dữ liệu CRM</th>
                    <th class="p-2.5 font-semibold text-foreground w-1/3">Cột tương ứng trong file</th>
                    <th class="p-2.5 font-semibold text-foreground w-1/3">Dữ liệu mẫu (Dòng 1)</th>
                  </tr>
                </thead>
                <tbody class="divide-y divide-border/60">
                  <!-- Name (Required) -->
                  <tr class="hover:bg-muted/20">
                    <td class="p-2.5 font-medium text-foreground">
                      {m.leads_import_field_name()}
                    </td>
                    <td class="p-2.5">
                      <select
                        bind:value={mapping.name_col}
                        class="w-full h-8 px-2 rounded-md border border-input bg-background text-xs focus:outline-hidden"
                      >
                        <option value={null}>{m.leads_import_unmapped()}</option>
                        {#each inspection.headers as header, idx (idx)}
                          <option value={idx}>{header}</option>
                        {/each}
                      </select>
                    </td>
                    <td class="p-2.5 text-muted-foreground font-mono truncate max-w-[200px]">
                      {mapping.name_col !== null && mapping.name_col !== undefined ? inspection.sample_rows[0]?.[mapping.name_col] || '—' : '—'}
                    </td>
                  </tr>

                  <!-- Phone -->
                  <tr class="hover:bg-muted/20">
                    <td class="p-2.5 font-medium text-foreground">{m.leads_import_field_phone()}</td>
                    <td class="p-2.5">
                      <select
                        bind:value={mapping.phone_col}
                        class="w-full h-8 px-2 rounded-md border border-input bg-background text-xs focus:outline-hidden"
                      >
                        <option value={null}>{m.leads_import_unmapped()}</option>
                        {#each inspection.headers as header, idx (idx)}
                          <option value={idx}>{header}</option>
                        {/each}
                      </select>
                    </td>
                    <td class="p-2.5 text-muted-foreground font-mono truncate max-w-[200px]">
                      {mapping.phone_col !== null && mapping.phone_col !== undefined ? inspection.sample_rows[0]?.[mapping.phone_col] || '—' : '—'}
                    </td>
                  </tr>

                  <!-- Email -->
                  <tr class="hover:bg-muted/20">
                    <td class="p-2.5 font-medium text-foreground">{m.leads_import_field_email()}</td>
                    <td class="p-2.5">
                      <select
                        bind:value={mapping.email_col}
                        class="w-full h-8 px-2 rounded-md border border-input bg-background text-xs focus:outline-hidden"
                      >
                        <option value={null}>{m.leads_import_unmapped()}</option>
                        {#each inspection.headers as header, idx (idx)}
                          <option value={idx}>{header}</option>
                        {/each}
                      </select>
                    </td>
                    <td class="p-2.5 text-muted-foreground font-mono truncate max-w-[200px]">
                      {mapping.email_col !== null && mapping.email_col !== undefined ? inspection.sample_rows[0]?.[mapping.email_col] || '—' : '—'}
                    </td>
                  </tr>

                  <!-- Company -->
                  <tr class="hover:bg-muted/20">
                    <td class="p-2.5 font-medium text-foreground">{m.leads_import_field_company()}</td>
                    <td class="p-2.5">
                      <select
                        bind:value={mapping.company_col}
                        class="w-full h-8 px-2 rounded-md border border-input bg-background text-xs focus:outline-hidden"
                      >
                        <option value={null}>{m.leads_import_unmapped()}</option>
                        {#each inspection.headers as header, idx (idx)}
                          <option value={idx}>{header}</option>
                        {/each}
                      </select>
                    </td>
                    <td class="p-2.5 text-muted-foreground font-mono truncate max-w-[200px]">
                      {mapping.company_col !== null && mapping.company_col !== undefined ? inspection.sample_rows[0]?.[mapping.company_col] || '—' : '—'}
                    </td>
                  </tr>

                  <!-- Value -->
                  <tr class="hover:bg-muted/20">
                    <td class="p-2.5 font-medium text-foreground">
                      {m.leads_import_field_value()} ({defaultCurrency})
                    </td>
                    <td class="p-2.5">
                      <select
                        bind:value={mapping.value_col}
                        class="w-full h-8 px-2 rounded-md border border-input bg-background text-xs focus:outline-hidden"
                      >
                        <option value={null}>{m.leads_import_unmapped()}</option>
                        {#each inspection.headers as header, idx (idx)}
                          <option value={idx}>{header}</option>
                        {/each}
                      </select>
                    </td>
                    <td class="p-2.5 text-muted-foreground font-mono truncate max-w-[200px]">
                      {mapping.value_col !== null && mapping.value_col !== undefined ? inspection.sample_rows[0]?.[mapping.value_col] || '—' : '—'}
                    </td>
                  </tr>

                  <!-- Address -->
                  <tr class="hover:bg-muted/20">
                    <td class="p-2.5 font-medium text-foreground">{m.leads_import_field_address()}</td>
                    <td class="p-2.5">
                      <select
                        bind:value={mapping.address_col}
                        class="w-full h-8 px-2 rounded-md border border-input bg-background text-xs focus:outline-hidden"
                      >
                        <option value={null}>{m.leads_import_unmapped()}</option>
                        {#each inspection.headers as header, idx (idx)}
                          <option value={idx}>{header}</option>
                        {/each}
                      </select>
                    </td>
                    <td class="p-2.5 text-muted-foreground font-mono truncate max-w-[200px]">
                      {mapping.address_col !== null && mapping.address_col !== undefined ? inspection.sample_rows[0]?.[mapping.address_col] || '—' : '—'}
                    </td>
                  </tr>

                  <!-- About / Notes -->
                  <tr class="hover:bg-muted/20">
                    <td class="p-2.5 font-medium text-foreground">{m.leads_import_field_about()}</td>
                    <td class="p-2.5">
                      <select
                        bind:value={mapping.about_col}
                        class="w-full h-8 px-2 rounded-md border border-input bg-background text-xs focus:outline-hidden"
                      >
                        <option value={null}>{m.leads_import_unmapped()}</option>
                        {#each inspection.headers as header, idx (idx)}
                          <option value={idx}>{header}</option>
                        {/each}
                      </select>
                    </td>
                    <td class="p-2.5 text-muted-foreground font-mono truncate max-w-[200px]">
                      {mapping.about_col !== null && mapping.about_col !== undefined ? inspection.sample_rows[0]?.[mapping.about_col] || '—' : '—'}
                    </td>
                  </tr>

                  <!-- Status -->
                  <tr class="hover:bg-muted/20">
                    <td class="p-2.5 font-medium text-foreground">{m.leads_import_field_status()}</td>
                    <td class="p-2.5">
                      <select
                        bind:value={mapping.status_col}
                        class="w-full h-8 px-2 rounded-md border border-input bg-background text-xs focus:outline-hidden"
                      >
                        <option value={null}>{m.leads_import_unmapped()}</option>
                        {#each inspection.headers as header, idx (idx)}
                          <option value={idx}>{header}</option>
                        {/each}
                      </select>
                    </td>
                    <td class="p-2.5 text-muted-foreground font-mono truncate max-w-[200px]">
                      {mapping.status_col !== null && mapping.status_col !== undefined ? inspection.sample_rows[0]?.[mapping.status_col] || '—' : '—'}
                    </td>
                  </tr>

                  <!-- Owner -->
                  <tr class="hover:bg-muted/20">
                    <td class="p-2.5 font-medium text-foreground">{m.leads_import_field_owner()}</td>
                    <td class="p-2.5">
                      <select
                        bind:value={mapping.owner_col}
                        class="w-full h-8 px-2 rounded-md border border-input bg-background text-xs focus:outline-hidden"
                      >
                        <option value={null}>{m.leads_import_unmapped()}</option>
                        {#each inspection.headers as header, idx (idx)}
                          <option value={idx}>{header}</option>
                        {/each}
                      </select>
                    </td>
                    <td class="p-2.5 text-muted-foreground font-mono truncate max-w-[200px]">
                      {mapping.owner_col !== null && mapping.owner_col !== undefined ? inspection.sample_rows[0]?.[mapping.owner_col] || '—' : '—'}
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>

            {#if !isMappingValid}
              <p class="text-destructive font-medium text-xs flex items-center gap-1.5">
                <AlertCircle class="size-3.5" />
                Vui lòng chọn cột tương ứng với trường bắt buộc: Họ và tên (*)
              </p>
            {/if}
          </div>

        <!-- STEP 3: Config & Conflicts -->
        {:else if currentStep === 3}
          <div class="flex flex-col gap-4">
            <!-- Conflict Strategy Options -->
            <div class="p-4 rounded-lg border border-border bg-card flex flex-col gap-3">
              <span class="font-semibold text-foreground text-sm flex items-center gap-2">
                <AlertTriangle class="size-4 text-amber-500" />
                {m.leads_import_conflict_title()}
              </span>

              <div class="flex flex-col gap-2.5">
                <label class="flex items-start gap-3 p-2.5 rounded-md border border-border/70 hover:bg-muted/30 cursor-pointer">
                  <input
                    type="radio"
                    name="conflict"
                    value="skip"
                    bind:group={conflictStrategy}
                    class="mt-0.5 text-primary focus:ring-primary"
                  />
                  <div>
                    <p class="font-medium text-foreground">{m.leads_import_conflict_skip()}</p>
                    <p class="text-muted-foreground text-[11px]">Nếu SĐT hoặc Email đã có trong CRM, bỏ qua dòng đó và giữ nguyên thông tin cũ.</p>
                  </div>
                </label>

                <label class="flex items-start gap-3 p-2.5 rounded-md border border-border/70 hover:bg-muted/30 cursor-pointer">
                  <input
                    type="radio"
                    name="conflict"
                    value="overwrite"
                    bind:group={conflictStrategy}
                    class="mt-0.5 text-primary focus:ring-primary"
                  />
                  <div>
                    <p class="font-medium text-foreground">{m.leads_import_conflict_overwrite()}</p>
                    <p class="text-muted-foreground text-[11px]">Cập nhật các thông tin mới từ tệp vào khách hàng hiện có.</p>
                  </div>
                </label>

                <label class="flex items-start gap-3 p-2.5 rounded-md border border-border/70 hover:bg-muted/30 cursor-pointer">
                  <input
                    type="radio"
                    name="conflict"
                    value="allow"
                    bind:group={conflictStrategy}
                    class="mt-0.5 text-primary focus:ring-primary"
                  />
                  <div>
                    <p class="font-medium text-foreground">{m.leads_import_conflict_allow()}</p>
                    <p class="text-muted-foreground text-[11px]">Tạo bản ghi mới ngay cả khi số điện thoại hoặc email bị trùng.</p>
                  </div>
                </label>
              </div>
            </div>

            <!-- Phone Normalization Checkbox -->
            <label class="flex items-center gap-2.5 p-3.5 rounded-lg border border-border bg-card cursor-pointer">
              <input
                type="checkbox"
                bind:checked={normalizePhone}
                class="rounded border-input text-primary focus:ring-primary size-4"
              />
              <div>
                <span class="font-medium text-foreground">{m.leads_import_normalize_phone_label()}</span>
                <p class="text-muted-foreground text-[11px]">Tự động thêm số 0 bị mất trong Excel, chuyển đổi các đầu +84/84 về 0, kiểm tra đầu số hợp lệ (03, 05, 07, 08, 09).</p>
              </div>
            </label>

            <!-- Defaults Configuration -->
            <div class="p-4 rounded-lg border border-border bg-card grid grid-cols-1 sm:grid-cols-2 md:grid-cols-4 gap-3">
              <!-- Category -->
              <div class="flex flex-col gap-1.5">
                <label for="import-cat" class="font-medium text-foreground">{m.leads_import_default_category_label()}</label>
                <select
                  id="import-cat"
                  bind:value={selectedCategoryId}
                  class="h-9 px-2 rounded-md border border-input bg-background text-xs focus:outline-hidden"
                >
                  <option value={null}>-- Không gán nhóm --</option>
                  {#each categories as cat (cat.id)}
                    <option value={cat.id}>{cat.name}</option>
                  {/each}
                </select>
              </div>

              <!-- Default Status -->
              <div class="flex flex-col gap-1.5">
                <label for="import-status" class="font-medium text-foreground">{m.leads_import_default_status_label()}</label>
                <Input
                  id="import-status"
                  bind:value={defaultStatus}
                  placeholder="New"
                  class="h-9 text-xs"
                />
              </div>

              <!-- Default Owner -->
              <div class="flex flex-col gap-1.5">
                <label for="import-owner" class="font-medium text-foreground">{m.leads_import_default_owner_label()}</label>
                <Input
                  id="import-owner"
                  bind:value={defaultOwner}
                  placeholder="Self"
                  class="h-9 text-xs"
                />
              </div>

              <!-- Default Currency -->
              <div class="flex flex-col gap-1.5">
                <label for="import-currency" class="font-medium text-foreground">{m.leads_import_default_currency_label()}</label>
                <select
                  id="import-currency"
                  bind:value={defaultCurrency}
                  class="h-9 px-2 rounded-md border border-input bg-background text-xs focus:outline-hidden"
                >
                  <option value="VND">VND (₫)</option>
                  <option value="USD">USD ($)</option>
                </select>
              </div>
            </div>
          </div>

        <!-- STEP 4: Execution Progress & Summary Results -->
        {:else if currentStep === 4}
          <div class="flex flex-col gap-4 items-center justify-center py-4">
            {#if importing}
              <div class="w-full max-w-md flex flex-col items-center gap-4 text-center">
                <RefreshCw class="size-8 text-primary animate-spin" />
                <div>
                  <p class="font-bold text-sm text-foreground">{m.leads_import_processing()}</p>
                  <p class="text-xs text-muted-foreground mt-0.5">
                    Đã xử lý {progress.processed_rows} / {progress.total_rows} dòng
                  </p>
                </div>
                <!-- Progress Bar -->
                <div class="w-full bg-muted rounded-full h-3 overflow-hidden border border-border">
                  <div
                    class="bg-primary h-full transition-all duration-300 rounded-full"
                    style={`width: ${progress.percentage}%;`}
                  ></div>
                </div>
                <span class="font-mono text-xs font-semibold text-primary">{progress.percentage}%</span>
              </div>
            {:else if summaryResult}
              <div class="w-full flex flex-col gap-4 animate-in fade-in duration-300">
                <!-- Success Header -->
                <div class="flex items-center gap-3 p-3.5 rounded-lg bg-emerald-500/10 border border-emerald-500/30 text-emerald-700 dark:text-emerald-300">
                  <CheckCircle2 class="size-5 shrink-0" />
                  <div>
                    <h4 class="font-bold text-sm">{m.leads_import_success_title()}</h4>
                    <p class="text-xs opacity-90">Toàn bộ dữ liệu hợp lệ đã được ghi vào hệ thống CRM an toàn.</p>
                  </div>
                </div>

                <!-- Metrics Grid -->
                <div class="grid grid-cols-2 sm:grid-cols-4 gap-3 text-center">
                  <div class="p-3 rounded-lg border border-border bg-card">
                    <p class="text-muted-foreground text-[11px]">{m.leads_import_stat_total()}</p>
                    <p class="text-xl font-bold font-mono text-foreground mt-1">{summaryResult.total_rows}</p>
                  </div>
                  <div class="p-3 rounded-lg border border-emerald-500/30 bg-emerald-500/5">
                    <p class="text-emerald-600 dark:text-emerald-400 text-[11px] font-medium">{m.leads_import_stat_inserted()}</p>
                    <p class="text-xl font-bold font-mono text-emerald-600 dark:text-emerald-400 mt-1">{summaryResult.inserted}</p>
                  </div>
                  <div class="p-3 rounded-lg border border-blue-500/30 bg-blue-500/5">
                    <p class="text-blue-600 dark:text-blue-400 text-[11px] font-medium">{m.leads_import_stat_updated()}</p>
                    <p class="text-xl font-bold font-mono text-blue-600 dark:text-blue-400 mt-1">{summaryResult.updated}</p>
                  </div>
                  <div class="p-3 rounded-lg border border-amber-500/30 bg-amber-500/5">
                    <p class="text-amber-600 dark:text-amber-400 text-[11px] font-medium">{m.leads_import_stat_skipped()}</p>
                    <p class="text-xl font-bold font-mono text-amber-600 dark:text-amber-400 mt-1">{summaryResult.skipped_duplicates}</p>
                  </div>
                </div>

                <!-- Failed Rows Alert & Export -->
                {#if summaryResult.failed > 0}
                  <div class="p-4 rounded-lg border border-destructive/30 bg-destructive/5 flex flex-col gap-3">
                    <div class="flex items-center justify-between">
                      <div class="flex items-center gap-2 text-destructive font-medium">
                        <AlertCircle class="size-4" />
                        <span>{m.leads_import_stat_failed()} {summaryResult.failed} dòng</span>
                      </div>
                      <Button variant="outline" size="sm" onclick={handleDownloadErrorRows} class="gap-1.5 text-xs border-destructive/40 text-destructive hover:bg-destructive/10">
                        <Download class="size-3.5" />
                        {m.leads_import_download_errors_btn()}
                      </Button>
                    </div>

                    <!-- Error Rows Table Preview -->
                    <div class="border border-border/80 rounded-md overflow-x-auto max-h-36">
                      <table class="w-full text-left text-xs border-collapse">
                        <thead class="bg-muted/70 sticky top-0 border-b border-border">
                          <tr>
                            <th class="p-2 font-semibold text-foreground w-16">Dòng</th>
                            <th class="p-2 font-semibold text-foreground">Lý do từ chối</th>
                          </tr>
                        </thead>
                        <tbody>
                          {#each summaryResult.error_rows.slice(0, 10) as err (err.row_number)}
                            <tr class="border-b border-border/50 hover:bg-muted/30">
                              <td class="p-2 font-mono text-muted-foreground">{err.row_number}</td>
                              <td class="p-2 text-destructive">{err.reason}</td>
                            </tr>
                          {/each}
                        </tbody>
                      </table>
                    </div>
                  </div>
                {/if}
              </div>
            {/if}
          </div>
        {/if}

      </div>

      <!-- Modal Bottom Navigation Buttons -->
      <div class="flex items-center justify-between border-t border-border pt-3">
        <div>
          {#if currentStep > 1 && currentStep < 4}
            <Button variant="outline" size="sm" onclick={() => currentStep -= 1} class="gap-1.5 text-xs">
              <ArrowLeft class="size-3.5" />
              {m.leads_import_back_btn()}
            </Button>
          {/if}
        </div>

        <div class="flex items-center gap-2">
          {#if currentStep < 4}
            <Button variant="ghost" size="sm" onclick={handleClose} class="text-xs">
              {m.leads_import_cancel_btn()}
            </Button>
          {/if}

          {#if currentStep === 1}
            <Button
              size="sm"
              onclick={() => currentStep = 2}
              disabled={!inspection || inspecting}
              class="gap-1.5 text-xs"
            >
              {m.leads_import_next_btn()}
              <ArrowRight class="size-3.5" />
            </Button>
          {:else if currentStep === 2}
            <Button
              size="sm"
              onclick={() => currentStep = 3}
              disabled={!isMappingValid}
              class="gap-1.5 text-xs"
            >
              {m.leads_import_next_btn()}
              <ArrowRight class="size-3.5" />
            </Button>
          {:else if currentStep === 3}
            <Button
              size="sm"
              onclick={handleStartImport}
              class="gap-1.5 text-xs bg-primary text-primary-foreground hover:bg-primary/90"
            >
              <Upload class="size-3.5" />
              {m.leads_import_start_btn()}
            </Button>
          {:else if currentStep === 4 && !importing}
            <Button size="sm" onclick={handleClose} class="text-xs">
              {m.leads_import_finish_btn()}
            </Button>
          {/if}
        </div>
      </div>

    </div>
  </div>
{/if}
