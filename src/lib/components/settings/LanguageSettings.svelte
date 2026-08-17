<script lang="ts">
  import * as Card from '$lib/components/ui/card';
  import { Languages, Check, Globe } from 'lucide-svelte';
  import { m, localeState, setLocale, type Locale, formatCurrency, formatDate } from '$lib/i18n';
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

  function handleSelectLanguage(locale: Locale) {
    if (localeState.current === locale) return;
    setLocale(locale);
    toast.success(m.settings_language_changed_toast());
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

  <Card.Content class="p-5 flex flex-col gap-4">
    <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
      {#each languages as lang}
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

    <!-- Live Regional Preview -->
    <div class="rounded-lg bg-muted/30 border border-border/50 p-3.5 flex flex-wrap items-center justify-between gap-3 text-xs text-muted-foreground">
      <div class="flex items-center gap-2">
        <Globe class="size-4 text-primary shrink-0" />
        <span>{m.settings_preview({ locale: localeState.current.toUpperCase() })}</span>
      </div>
      <div class="flex items-center gap-4 font-mono font-medium text-foreground">
        <span>{m.settings_currency()} <strong class="text-emerald-600 dark:text-emerald-400">{formatCurrency(15000000, localeState.current === 'vi' ? 'VND' : 'USD')}</strong></span>
        <span>{m.settings_date()} <strong>{formatDate(new Date())}</strong></span>
      </div>
    </div>
  </Card.Content>
</Card.Root>
