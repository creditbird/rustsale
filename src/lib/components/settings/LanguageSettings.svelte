<script lang="ts">
  import * as Card from '$lib/components/ui/card';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Languages, Check, Globe, DollarSign, Coins, RotateCcw } from 'lucide-svelte';
  import {
    m,
    localeState,
    setLocale,
    type Locale,
    type Currency,
    currencyState,
    setBaseCurrency,
    setExchangeRate,
    DEFAULT_USD_VND_RATE,
    formatCurrency,
    formatDate
  } from '$lib/i18n';
  import { toast } from 'svelte-sonner';

  const languages = [
    {
      id: 'vi' as const,
      label: 'Vietnamese',
      native: 'Tiếng Việt',
      flag: '🇻🇳',
      get desc() { return m.settings_lang_vi_desc(); }
    },
    {
      id: 'en' as const,
      label: 'English',
      native: 'English (US)',
      flag: '🇺🇸',
      get desc() { return m.settings_lang_en_desc(); }
    }
  ];

  const currencyOptions = [
    {
      id: 'VND' as const,
      symbol: '₫',
      label: 'VND (₫)',
      get title() { return m.settings_currency_vnd(); },
      get desc() { return m.settings_currency_vnd_desc(); }
    },
    {
      id: 'USD' as const,
      symbol: '$',
      label: 'USD ($)',
      get title() { return m.settings_currency_usd(); },
      get desc() { return m.settings_currency_usd_desc(); }
    }
  ];

  let editRateInput = $state(String(currencyState.exchangeRate));
  let isEditingRate = $state(false);
  let isSavingCurrency = $state(false);
  let isSavingRate = $state(false);
  let isSaving = $derived(isSavingCurrency || isSavingRate);

  $effect(() => {
    const currentRate = currencyState.exchangeRate;
    if (!isEditingRate) {
      editRateInput = String(currentRate);
    }
  });

  function handleSelectLanguage(locale: Locale) {
    if (localeState.current === locale) return;
    setLocale(locale);
    toast.success(m.settings_language_changed_toast());
  }

  async function handleSelectCurrency(curr: Currency) {
    if (currencyState.baseCurrency === curr || isSaving) return;
    isSavingCurrency = true;
    try {
      await setBaseCurrency(curr);
      toast.success(m.settings_currency_changed_toast());
    } catch (err: any) {
      console.error('Failed to save currency setting:', err);
      toast.error(err?.message || 'Failed to save currency setting');
    } finally {
      isSavingCurrency = false;
    }
  }

  async function handleUpdateRate() {
    if (isSaving) return;
    const parsed = parseFloat(editRateInput.replace(/,/g, '').trim());
    if (isNaN(parsed) || parsed <= 0) {
      editRateInput = String(currencyState.exchangeRate);
      return;
    }
    if (parsed === currencyState.exchangeRate) return;
    isSavingRate = true;
    try {
      await setExchangeRate(parsed);
      editRateInput = String(parsed);
      toast.success(m.settings_rate_changed_toast());
    } catch (err: any) {
      console.error('Failed to save exchange rate:', err);
      editRateInput = String(currencyState.exchangeRate);
      toast.error(err?.message || 'Failed to save exchange rate');
    } finally {
      isSavingRate = false;
    }
  }

  async function handleResetRate() {
    if (currencyState.exchangeRate === DEFAULT_USD_VND_RATE || isSaving) return;
    isSavingRate = true;
    try {
      await currencyState.resetExchangeRate();
      editRateInput = String(DEFAULT_USD_VND_RATE);
      toast.success(m.settings_rate_changed_toast());
    } catch (err: any) {
      console.error('Failed to reset exchange rate:', err);
      editRateInput = String(currencyState.exchangeRate);
      toast.error(err?.message || 'Failed to reset exchange rate');
    } finally {
      isSavingRate = false;
    }
  }
</script>

<Card.Root class="overflow-hidden border-border/80">
  <Card.Header class="bg-muted/10 border-b border-border/60 pb-4">
    <div class="flex items-start gap-3">
      <div class="rounded-xl bg-primary/10 p-2.5 text-primary shrink-0">
        <Languages class="size-5" />
      </div>
      <div>
        <Card.Title class="text-base font-bold text-foreground">
          {m.settings_language_title()}
        </Card.Title>
        <Card.Description class="text-xs text-muted-foreground mt-0.5">
          {m.settings_language_desc()}
        </Card.Description>
      </div>
    </div>
  </Card.Header>

  <Card.Content class="p-5 flex flex-col gap-6">
    <!-- Language Selection -->
    <div>
      <h4 class="text-xs font-bold text-foreground uppercase tracking-wider mb-3">
        {m.settings_language_label()}
      </h4>
      <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
        {#each languages as lang (lang.id)}
          {@const isSelected = localeState.current === lang.id}
          <button
            type="button"
            class="flex flex-col text-left p-4 rounded-xl border transition-all relative cursor-pointer {isSelected
              ? 'border-primary bg-primary/5 ring-2 ring-primary/20 shadow-sm'
              : 'border-border/70 hover:border-border hover:bg-muted/30'}"
            onclick={() => handleSelectLanguage(lang.id)}
          >
            <div class="flex items-center justify-between mb-2">
              <div class="flex items-center gap-2.5">
                <span class="text-xl leading-none">{lang.flag}</span>
                <div>
                  <span class="text-sm font-semibold text-foreground">{lang.native}</span>
                  <span class="text-xs text-muted-foreground ml-1.5">({lang.label})</span>
                </div>
              </div>
              {#if isSelected}
                <div class="rounded-full bg-primary text-primary-foreground p-0.5">
                  <Check class="size-3.5" />
                </div>
              {/if}
            </div>
            <p class="text-xs text-muted-foreground leading-relaxed">{lang.desc}</p>
          </button>
        {/each}
      </div>
    </div>

    <!-- Base Currency Selection -->
    <div class="pt-2 border-t border-border/60">
      <div class="flex items-center justify-between mb-3">
        <div>
          <h4 class="text-xs font-bold text-foreground uppercase tracking-wider">
            {m.settings_currency_base_label()}
          </h4>
          <p class="text-xs text-muted-foreground mt-0.5">
            {m.settings_currency_desc()}
          </p>
        </div>
      </div>

      <div class="grid grid-cols-1 sm:grid-cols-2 gap-3 mb-4">
        {#each currencyOptions as opt (opt.id)}
          {@const isCurrSelected = currencyState.baseCurrency === opt.id}
          <button
            type="button"
            disabled={!currencyState.isHydrated || isSaving}
            class="flex flex-col text-left p-4 rounded-xl border transition-all relative cursor-pointer disabled:opacity-60 disabled:cursor-not-allowed {isCurrSelected
              ? 'border-emerald-500 bg-emerald-500/5 ring-2 ring-emerald-500/20 shadow-sm'
              : 'border-border/70 hover:border-border hover:bg-muted/30'}"
            onclick={() => handleSelectCurrency(opt.id)}
          >
            <div class="flex items-center justify-between mb-2">
              <div class="flex items-center gap-2.5">
                <div class="w-7 h-7 rounded-lg bg-muted/60 flex items-center justify-center font-bold text-sm {isCurrSelected ? 'text-emerald-600 dark:text-emerald-400 bg-emerald-500/10' : 'text-foreground'}">
                  {opt.symbol}
                </div>
                <div>
                  <span class="text-sm font-semibold text-foreground">{opt.title}</span>
                </div>
              </div>
              {#if isCurrSelected}
                <div class="rounded-full bg-emerald-500 text-white p-0.5">
                  <Check class="size-3.5" />
                </div>
              {/if}
            </div>
            <p class="text-xs text-muted-foreground leading-relaxed">{opt.desc}</p>
          </button>
        {/each}
      </div>

      <!-- USD / VND Exchange Rate Input -->
      <div class="p-3.5 rounded-xl bg-muted/20 border border-border/60 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
        <div class="flex items-center gap-2.5">
          <div class="p-2 rounded-lg bg-primary/10 text-primary">
            <Coins class="size-4" />
          </div>
          <div>
            <span class="text-xs font-semibold text-foreground block">{m.settings_currency_rate_label()}</span>
            <span class="text-[11px] text-muted-foreground">{m.settings_currency_rate_help({ rate: Number(currencyState.exchangeRate).toLocaleString() })}</span>
          </div>
        </div>
        <div class="flex items-center gap-2">
          <div class="relative w-32">
            <Input
              type="number"
              bind:value={editRateInput}
              min="1"
              step="100"
              disabled={!currencyState.isHydrated || isSaving}
              class="h-8 text-xs font-mono font-medium rounded-lg text-right pr-2"
              onfocus={() => isEditingRate = true}
              onblur={() => { isEditingRate = false; handleUpdateRate(); }}
              onkeydown={(e) => { if (e.key === 'Enter') handleUpdateRate(); }}
            />
          </div>
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={!currencyState.isHydrated || isSaving}
            class="h-8 px-2.5 text-xs rounded-lg gap-1.5"
            onclick={handleResetRate}
            title={m.settings_currency_rate_reset()}
          >
            <RotateCcw class="size-3 text-muted-foreground" />
            <span class="text-[11px]">{m.common_clear_filters ? m.common_clear_filters() : 'Reset'}</span>
          </Button>
        </div>
      </div>
    </div>

    <!-- Live Regional Preview -->
    <div class="rounded-xl bg-muted/30 border border-border/50 p-4 flex flex-wrap items-center justify-between gap-3 text-xs text-muted-foreground">
      <div class="flex items-center gap-2">
        <Globe class="size-4 text-primary shrink-0" />
        <span>{m.settings_preview({ locale: `${localeState.current.toUpperCase()} · ${currencyState.baseCurrency}` })}</span>
      </div>
      <div class="flex items-center gap-4 font-mono font-medium text-foreground">
        <span>{m.settings_currency()} <strong class="text-emerald-600 dark:text-emerald-400">{formatCurrency(currencyState.baseCurrency === 'VND' ? 15000000 : 1500, currencyState.baseCurrency)}</strong></span>
        <span>{m.settings_date()} <strong>{formatDate(new Date())}</strong></span>
      </div>
    </div>
  </Card.Content>
</Card.Root>
