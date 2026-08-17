<script lang="ts">
  import { onMount } from 'svelte';
  import * as crmService from '$lib/services/crmService';
  import type { Lead, Deal, Activity as ActivityType } from '$lib/services/crmService';
  import * as Card from '$lib/components/ui/card';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { 
    DollarSign, Trophy, Users, CheckCircle2, TrendingUp, TrendingDown, AlertCircle,
    Layers, Activity, RefreshCw, MessageCircle, Settings, X, Edit3, Save, Target
  } from 'lucide-svelte';
  import {
    m,
    formatCurrency,
    formatNumber,
    currencyState,
    convertCurrency,
    type Currency
  } from '$lib/i18n';

  let { activeTab = 'dashboard' }: { activeTab?: string } = $props();

  let leads = $state<Lead[]>([]);
  let deals = $state<Deal[]>([]);
  let recentActivities = $state<ActivityType[]>([]);
  let errorMsg = $state<string | null>(null);
  let loading = $state(false);
  let selectedPeriod = $state('Q3 2026');
  let isStale = $state(false);
  let hasLoaded = $state(false);

  function getInitialTarget(): number {
    if (typeof localStorage === 'undefined') return 120000000;
    try {
      const saved = localStorage.getItem('rustsale_target_revenue_v2');
      if (saved) return Math.max(1000000, Number(saved) || 120000000);
      const legacy = localStorage.getItem('rustsale_target_revenue');
      if (legacy) {
        const num = Number(legacy);
        return num >= 1000000 ? num : 120000000;
      }
    } catch {}
    return 120000000;
  }

  function getInitialStretch(target: number): number {
    if (typeof localStorage === 'undefined') return 150000000;
    try {
      const saved = localStorage.getItem('rustsale_stretch_revenue_v2');
      if (saved) return Math.max(target, Number(saved) || 150000000);
      const legacy = localStorage.getItem('rustsale_stretch_revenue');
      if (legacy) {
        const num = Number(legacy);
        return num >= target ? num : Math.max(target, 150000000);
      }
    } catch {}
    return 150000000;
  }

  // Configurable Base Target & Stretch Goals (Quarterly baseline in VND)
  let targetRevenue = $state(getInitialTarget());
  let stretchRevenue = $state(getInitialStretch(targetRevenue));
  let showTargetModal = $state(false);
  let editTargetInput = $state(targetRevenue);
  let editStretchInput = $state(stretchRevenue);

  let reloadTimer: ReturnType<typeof setTimeout> | null = null;
  function debouncedReload() {
    if (reloadTimer) clearTimeout(reloadTimer);
    reloadTimer = setTimeout(() => {
      reloadTimer = null;
      if (activeTab === 'dashboard') {
        reloadData();
      } else {
        isStale = true;
      }
    }, 300);
  }

  $effect(() => {
    if (activeTab === 'dashboard') {
      if (isStale || !hasLoaded) {
        isStale = false;
        reloadData();
      }
    } else {
      if (reloadTimer) {
        clearTimeout(reloadTimer);
        reloadTimer = null;
        isStale = true;
      }
    }
  });

  onMount(() => {
    const unsubBridge = crmService.initScraperEventBridge();

    const unsubLeads = crmService.onLeadsChanged(() => {
      if (activeTab === 'dashboard') {
        debouncedReload();
      } else {
        isStale = true;
      }
    });

    return () => {
      unsubLeads();
      unsubBridge();
      if (reloadTimer) clearTimeout(reloadTimer);
    };
  });

  function openTargetModal() {
    editTargetInput = targetRevenue;
    editStretchInput = stretchRevenue;
    showTargetModal = true;
  }

  function saveTargets(e: Event) {
    e.preventDefault();
    targetRevenue = Math.max(1000000, Number(editTargetInput) || 120000000);
    stretchRevenue = Math.max(targetRevenue, Number(editStretchInput) || 150000000);
    try {
      localStorage.setItem('rustsale_target_revenue_v2', targetRevenue.toString());
      localStorage.setItem('rustsale_stretch_revenue_v2', stretchRevenue.toString());
    } catch (e) {
      console.error('Failed to save target to localStorage', e);
    }
    showTargetModal = false;
  }

  function reloadData() {
    loading = true;
    Promise.all([crmService.getLeads(), crmService.getDeals(), crmService.getRecentActivities(5)])
      .then(([leadsData, dealsData, activitiesData]) => {
        leads = leadsData;
        deals = dealsData;
        recentActivities = activitiesData;
        errorMsg = null;
        hasLoaded = true;
        isStale = false;
      })
      .catch((err) => {
        errorMsg = err instanceof Error ? err.message : String(err);
      })
      .finally(() => {
        loading = false;
      });
  }

  // Active period target scaling
  const periodMultiplier = $derived(
    selectedPeriod === 'This Month' ? 0.333 : selectedPeriod === 'YTD 2026' ? 4 : 1
  );

  const activePeriodTarget = $derived(Math.round(targetRevenue * periodMultiplier));
  const activePeriodStretch = $derived(Math.round(stretchRevenue * periodMultiplier));

  const convertedActivePeriodTarget = $derived(
    convertCurrency(activePeriodTarget, 'VND', currencyState.baseCurrency, currencyState.exchangeRate)
  );
  const convertedActivePeriodStretch = $derived(
    convertCurrency(activePeriodStretch, 'VND', currencyState.baseCurrency, currencyState.exchangeRate)
  );

  // Filter deals based on selectedPeriod
  const activeDeals = $derived.by(() => {
    return deals.filter(d => {
      if (selectedPeriod === 'All' || !selectedPeriod) return true;
      if (!d.created_at) return false;
      const dDate = new Date(d.created_at);
      if (isNaN(dDate.getTime())) return false;
      const year = dDate.getFullYear();
      const month = dDate.getMonth();

      if (selectedPeriod === 'This Month') {
        const now = new Date();
        return year === now.getFullYear() && month === now.getMonth();
      } else if (selectedPeriod === 'Q3 2026') {
        return year === 2026 && month >= 6 && month <= 8;
      } else if (selectedPeriod === 'YTD 2026') {
        return year === 2026;
      }
      return true;
    });
  });

  const totalRevenue = $derived(
    activeDeals
      .filter(d => d.stage === 'Won')
      .reduce((sum, d) => {
        const val = convertCurrency(
          d.value || 0,
          (d.currency as Currency) || 'VND',
          currencyState.baseCurrency,
          currencyState.exchangeRate
        );
        return sum + val;
      }, 0)
  );

  const dealsWon = $derived(
    activeDeals.filter(d => d.stage === 'Won').length
  );

  const activeLeadsCount = $derived(
    leads.filter(l => l.status !== 'Converted' && l.status !== 'Lost').length
  );

  const totalPipelineValue = $derived(
    activeDeals
      .filter(d => d.stage !== 'Won' && d.stage !== 'Lost')
      .reduce((sum, d) => {
        const val = convertCurrency(
          d.value || 0,
          (d.currency as Currency) || 'VND',
          currencyState.baseCurrency,
          currencyState.exchangeRate
        );
        return sum + val;
      }, 0)
  );

  const totalDealsCount = $derived(activeDeals.length);

  const winRate = $derived(
    totalDealsCount > 0 ? Math.round((dealsWon / totalDealsCount) * 100) : 0
  );

  const targetProgressPct = $derived(
    convertedActivePeriodTarget > 0 ? Math.min(100, Math.round((totalRevenue / convertedActivePeriodTarget) * 100)) : 0
  );

  const targetPosX = $derived(
    convertedActivePeriodStretch > 0 ? Math.min(380, Math.max(20, Math.round(10 + (convertedActivePeriodTarget / convertedActivePeriodStretch) * 380))) : 314
  );

  function formatCompactCurrency(val: number) {
    return formatCurrency(val, currencyState.baseCurrency);
  }

  // Calculation of Previous Period Data for 100% Real Comparative Trends
  const previousPeriodDeals = $derived.by(() => {
    return deals.filter(d => {
      if (!d.created_at) return false;
      const dDate = new Date(d.created_at);
      if (isNaN(dDate.getTime())) return false;
      const year = dDate.getFullYear();
      const month = dDate.getMonth();

      if (selectedPeriod === 'This Month') {
        const now = new Date();
        const prevMonth = now.getMonth() === 0 ? 11 : now.getMonth() - 1;
        const prevYear = now.getMonth() === 0 ? now.getFullYear() - 1 : now.getFullYear();
        return year === prevYear && month === prevMonth;
      } else if (selectedPeriod === 'Q3 2026') {
        return year === 2026 && month >= 3 && month <= 5;
      } else if (selectedPeriod === 'YTD 2026') {
        return year === 2025;
      }
      return false;
    });
  });

  const previousRevenue = $derived(
    previousPeriodDeals
      .filter(d => d.stage === 'Won')
      .reduce((sum, d) => {
        const val = convertCurrency(
          d.value || 0,
          (d.currency as Currency) || 'VND',
          currencyState.baseCurrency,
          currencyState.exchangeRate
        );
        return sum + val;
      }, 0)
  );

  const previousWonCount = $derived(
    previousPeriodDeals.filter(d => d.stage === 'Won').length
  );

  const revenueChangeStr = $derived.by(() => {
    if (previousRevenue === 0) {
      return totalRevenue > 0 ? '+100%' : '+0%';
    }
    const pct = ((totalRevenue - previousRevenue) / previousRevenue) * 100;
    return (pct >= 0 ? '+' : '') + pct.toFixed(1) + '%';
  });

  const dealsWonChangeStr = $derived.by(() => {
    const diff = dealsWon - previousWonCount;
    return diff >= 0 ? `+${diff}` : `${diff}`;
  });

  const previousPeriodLeads = $derived.by(() => {
    return leads.filter(l => {
      if (!l.last_activity) return false;
      const lDate = new Date(l.last_activity);
      if (isNaN(lDate.getTime())) return false;
      const year = lDate.getFullYear();
      const month = lDate.getMonth();

      if (selectedPeriod === 'This Month') {
        const now = new Date();
        const prevMonth = now.getMonth() === 0 ? 11 : now.getMonth() - 1;
        const prevYear = now.getMonth() === 0 ? now.getFullYear() - 1 : now.getFullYear();
        return year === prevYear && month === prevMonth;
      } else if (selectedPeriod === 'Q3 2026') {
        return year === 2026 && month >= 3 && month <= 5;
      } else if (selectedPeriod === 'YTD 2026') {
        return year === 2025;
      }
      return false;
    });
  });

  const currentPeriodLeads = $derived.by(() => {
    return leads.filter(l => {
      if (!l.last_activity) return true;
      const lDate = new Date(l.last_activity);
      if (isNaN(lDate.getTime())) return true;
      const year = lDate.getFullYear();
      const month = lDate.getMonth();

      if (selectedPeriod === 'This Month') {
        const now = new Date();
        return year === now.getFullYear() && month === now.getMonth();
      } else if (selectedPeriod === 'Q3 2026') {
        return year === 2026 && month >= 6 && month <= 8;
      } else if (selectedPeriod === 'YTD 2026') {
        return year === 2026;
      }
      return true;
    });
  });

  const leadsChangeStr = $derived.by(() => {
    const prevCount = previousPeriodLeads.length;
    const currCount = currentPeriodLeads.length;
    if (prevCount === 0) {
      return currCount > 0 ? `+${currCount}` : '+0%';
    }
    const pct = ((currCount - prevCount) / prevCount) * 100;
    return (pct >= 0 ? '+' : '') + pct.toFixed(1) + '%';
  });

  const stats = $derived([
    { label: m.dashboard_total_revenue(), value: formatCurrency(totalRevenue, currencyState.baseCurrency), change: revenueChangeStr, up: totalRevenue >= previousRevenue, icon: DollarSign },
    { label: m.dashboard_deals_won(), value: String(dealsWon), change: dealsWonChangeStr, up: dealsWon >= previousWonCount, icon: Trophy },
    { label: m.dashboard_active_leads(), value: String(activeLeadsCount), change: leadsChangeStr, up: currentPeriodLeads.length >= previousPeriodLeads.length, icon: Users }
  ]);

  // Pipeline Stage Distribution
  const stageConfig: Record<string, { label: string; color: string; bg: string; border: string; prob: number }> = {
    get 'Prospecting'() { return { label: m.pipeline_stage_prospecting(), color: 'text-blue-500', bg: 'bg-blue-500', border: 'border-blue-500/30', prob: 20 }; },
    get 'Demo'() { return { label: m.pipeline_stage_demo(), color: 'text-purple-500', bg: 'bg-purple-500', border: 'border-purple-500/30', prob: 40 }; },
    get 'Proposal'() { return { label: m.pipeline_stage_proposal(), color: 'text-amber-500', bg: 'bg-amber-500', border: 'border-amber-500/30', prob: 60 }; },
    get 'Negotiation'() { return { label: m.pipeline_stage_negotiation(), color: 'text-indigo-500', bg: 'bg-indigo-500', border: 'border-indigo-500/30', prob: 80 }; },
    get 'Won'() { return { label: m.pipeline_stage_won(), color: 'text-emerald-500', bg: 'bg-emerald-500', border: 'border-emerald-500/30', prob: 100 }; }
  };

  const stageBreakdown = $derived(
    Object.keys(stageConfig).map(stage => {
      const stageDeals = activeDeals.filter(d => d.stage === stage);
      const value = stageDeals.reduce((sum, d) => {
        return sum + convertCurrency(
          d.value || 0,
          (d.currency as Currency) || 'VND',
          currencyState.baseCurrency,
          currencyState.exchangeRate
        );
      }, 0);
      const count = stageDeals.length;
      return {
        stage,
        ...stageConfig[stage],
        value,
        count,
        pctOfTotal: totalPipelineValue + totalRevenue > 0 ? Math.round((value / (totalPipelineValue + totalRevenue)) * 100) : 0
      };
    })
  );

  // Top Active Deals sorted by value converted to common base currency
  const topActiveDeals = $derived(
    activeDeals
      .filter(d => d.stage !== 'Won')
      .sort((a, b) => {
        const valA = convertCurrency(
          a.value || 0,
          (a.currency as Currency) || 'VND',
          currencyState.baseCurrency,
          currencyState.exchangeRate
        );
        const valB = convertCurrency(
          b.value || 0,
          (b.currency as Currency) || 'VND',
          currencyState.baseCurrency,
          currencyState.exchangeRate
        );
        return valB - valA;
      })
      .slice(0, 4)
  );

  // Lead Status Stats
  const leadStatusStats = $derived([
    { label: 'New', count: leads.filter(l => l.status === 'New').length, color: 'bg-blue-500' },
    { label: 'Contacted', count: leads.filter(l => l.status === 'Contacted').length, color: 'bg-amber-500' },
    { label: 'Qualified', count: leads.filter(l => l.status === 'Qualified').length, color: 'bg-indigo-500' },
    { label: 'Converted', count: leads.filter(l => l.status === 'Converted').length, color: 'bg-emerald-500' },
    { label: 'Lost', count: leads.filter(l => l.status === 'Lost').length, color: 'bg-rose-500' }
  ]);

  const zaloVerifiedCount = $derived(leads.filter(l => l.has_zalo === 1).length);
  const zaloPct = $derived(leads.length > 0 ? Math.round((zaloVerifiedCount / leads.length) * 100) : 0);

  const schedule = $derived.by(() => {
    return recentActivities.map(act => {
      const timeStr = act.created_at ? new Date(act.created_at).toLocaleTimeString('en-US', { hour: '2-digit', minute: '2-digit' }) : 'Just now';
      return {
        time: timeStr,
        title: `[${act.activity_type}] ${act.content}`
      };
    });
  });

  // Dynamic Monthly / Period Breakdown Trend Data derived 100% from actual Won Deals
  const monthlyTrendData = $derived.by(() => {
    const wonDeals = activeDeals.filter(d => d.stage === 'Won');

    if (selectedPeriod === 'This Month') {
      const weekBuckets = [0, 0, 0, 0];
      wonDeals.forEach(d => {
        if (!d.created_at) return;
        const dt = new Date(d.created_at);
        if (isNaN(dt.getTime())) return;
        const day = dt.getDate();
        const weekIdx = Math.min(3, Math.floor((day - 1) / 7));
        const val = convertCurrency(
          d.value || 0,
          (d.currency as Currency) || 'VND',
          currencyState.baseCurrency,
          currencyState.exchangeRate
        );
        weekBuckets[weekIdx] += val;
      });
      const targetPerWeek = Math.round(convertedActivePeriodTarget / 4);
      return [
        { month: 'Week 1', value: weekBuckets[0], target: targetPerWeek },
        { month: 'Week 2', value: weekBuckets[1], target: targetPerWeek },
        { month: 'Week 3', value: weekBuckets[2], target: targetPerWeek },
        { month: 'Week 4', value: weekBuckets[3], target: targetPerWeek }
      ];
    } else if (selectedPeriod === 'YTD 2026') {
      const qBuckets = [0, 0, 0, 0];
      wonDeals.forEach(d => {
        if (!d.created_at) return;
        const dt = new Date(d.created_at);
        if (isNaN(dt.getTime())) return;
        const m = dt.getMonth();
        const qIdx = Math.floor(m / 3);
        const val = convertCurrency(
          d.value || 0,
          (d.currency as Currency) || 'VND',
          currencyState.baseCurrency,
          currencyState.exchangeRate
        );
        qBuckets[qIdx] += val;
      });
      const targetPerQ = Math.round(convertedActivePeriodTarget / 4);
      return [
        { month: 'Q1 2026', value: qBuckets[0], target: targetPerQ },
        { month: 'Q2 2026', value: qBuckets[1], target: targetPerQ },
        { month: 'Q3 2026', value: qBuckets[2], target: targetPerQ },
        { month: 'Q4 2026', value: qBuckets[3], target: targetPerQ }
      ];
    }

    // Default Q3 2026 (July, August, September)
    const monthBuckets: Record<number, number> = { 6: 0, 7: 0, 8: 0 };
    wonDeals.forEach(d => {
      if (!d.created_at) return;
      const dt = new Date(d.created_at);
      if (isNaN(dt.getTime())) return;
      const m = dt.getMonth();
      if (m in monthBuckets) {
        const val = convertCurrency(
          d.value || 0,
          (d.currency as Currency) || 'VND',
          currencyState.baseCurrency,
          currencyState.exchangeRate
        );
        monthBuckets[m] += val;
      }
    });
    const targetPerMonth = Math.round(convertedActivePeriodTarget / 3);
    return [
      { month: 'Jul 2026', value: monthBuckets[6], target: targetPerMonth },
      { month: 'Aug 2026', value: monthBuckets[7], target: targetPerMonth },
      { month: 'Sep 2026', value: monthBuckets[8], target: targetPerMonth }
    ];
  });
</script>

<div class="space-y-6">
  {#if errorMsg}
    <div class="p-4 rounded-lg border border-destructive/30 bg-destructive/10 text-destructive text-sm flex items-center gap-3">
      <AlertCircle class="w-5 h-5 shrink-0" />
      <span>Failed to load CRM data: {errorMsg}</span>
    </div>
  {/if}

  <!-- Executive Dashboard Header -->
  <div class="flex flex-col sm:flex-row sm:items-center justify-between gap-4 bg-card/60 p-4 rounded-2xl border border-border/80 shadow-2xs backdrop-blur-xs">
    <div>
      <div class="flex items-center gap-2">
        <h2 class="text-xl font-bold text-foreground tracking-tight">Sales Performance</h2>
        <Badge variant="outline" class="border-emerald-500/30 text-emerald-600 bg-emerald-500/10 font-mono text-[10px] uppercase">
          Live System
        </Badge>
      </div>
      <p class="text-xs text-muted-foreground mt-0.5">Real-time overview of CRM metrics, deals, and daily schedule</p>
    </div>

    <!-- Controls & Period Filter -->
    <div class="flex items-center gap-2 self-start sm:self-auto flex-wrap">
      <div class="flex items-center gap-1 bg-muted/50 p-1 rounded-xl border border-border text-xs">
        {#each ['This Month', 'Q3 2026', 'YTD 2026'] as period}
          <button
            type="button"
            onclick={() => selectedPeriod = period}
            class="px-3 py-1 rounded-lg font-semibold transition-all text-xs {selectedPeriod === period ? 'bg-primary text-primary-foreground shadow-2xs font-bold' : 'text-muted-foreground hover:text-foreground'}"
          >
            {period}
          </button>
        {/each}
      </div>

      <Button variant="outline" size="sm" onclick={reloadData} class="h-8 gap-1.5 text-xs font-semibold rounded-xl" disabled={loading}>
        <RefreshCw class="w-3.5 h-3.5 {loading ? 'animate-spin' : ''}" />
        <span>Refresh</span>
      </Button>
    </div>
  </div>

  <!-- Primary Executive KPI Cards Grid (3 Columns) -->
  <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
    {#each stats as stat}
      {@const Icon = stat.icon}
      <Card.Root class="transition-all duration-150 hover:border-foreground/20">
        <Card.Header class="flex flex-row items-center justify-between space-y-0 pb-2">
          <Card.Title class="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
            {stat.label}
          </Card.Title>
          <div class="w-8 h-8 rounded-md bg-muted flex items-center justify-center text-muted-foreground">
            <Icon class="w-4 h-4" />
          </div>
        </Card.Header>
        <Card.Content>
          <div class="flex items-baseline justify-between">
            <div class="text-2xl font-bold tracking-tight text-foreground">{stat.value}</div>
            <Badge variant={stat.up ? "default" : "destructive"} class="gap-1 text-[11px] px-2 py-0.5">
              {#if stat.up}
                <TrendingUp class="w-3 h-3" />
              {:else}
                <TrendingDown class="w-3 h-3" />
              {/if}
              {stat.change}
            </Badge>
          </div>
        </Card.Content>
      </Card.Root>
    {/each}
  </div>

  <!-- Row 1: Charts Section (Revenue Target vs. Actual & Stage Funnel) -->
  <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
    
    <!-- Revenue Target vs. Actual Modern Interactive Chart Card -->
    <Card.Root class="lg:col-span-2 shadow-2xs border-border/80">
      <Card.Header class="flex flex-row items-center justify-between pb-4 border-b border-border/60">
        <div>
          <Card.Title class="text-base font-semibold">Revenue Target vs. Actual</Card.Title>
          <p class="text-xs text-muted-foreground font-medium">Tracking Period: <span class="font-bold text-foreground">{selectedPeriod}</span></p>
        </div>
        <div class="flex items-center gap-2">
          <Button 
            variant="outline" 
            size="sm" 
            onclick={openTargetModal}
            class="h-7 text-xs gap-1.5 font-semibold text-primary border-primary/30 hover:bg-primary/10 rounded-lg"
          >
            <Settings class="w-3.5 h-3.5" />
            <span>Edit Target</span>
          </Button>
          <Badge variant="outline" class="border-emerald-500/30 text-emerald-600 bg-emerald-500/10 uppercase text-[10px] tracking-wider font-bold">
            {selectedPeriod}
          </Badge>
        </div>
      </Card.Header>

      <Card.Content class="pt-6 space-y-6">
        <!-- Target Progress Summary Banner -->
        <div class="p-4 rounded-xl bg-muted/40 border border-border/70 flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4">
          <div class="space-y-1">
            <span class="text-xs text-muted-foreground font-medium">Actual Progress ({selectedPeriod})</span>
            <div class="flex items-baseline gap-2">
              <span class="text-2xl font-extrabold text-emerald-600 tracking-tight">Revenue: {formatCompactCurrency(totalRevenue)}</span>
              <span class="text-xs font-bold text-muted-foreground">/ {formatCompactCurrency(convertedActivePeriodTarget)} Target</span>
            </div>
          </div>
          <div class="flex items-center gap-4">
            <div class="text-right">
              <span class="text-[11px] text-muted-foreground block font-medium">Completion Rate</span>
              <span class="text-lg font-black text-foreground">{targetProgressPct}%</span>
            </div>
            <div class="w-12 h-12 rounded-full border-4 border-emerald-500/20 border-t-emerald-500 flex items-center justify-center font-bold text-xs text-emerald-600 bg-emerald-500/5">
              {targetProgressPct}%
            </div>
          </div>
        </div>

        <!-- SVG Target vs Actual Bar Chart Visualizer -->
        <div class="space-y-3 pt-2">
          <div class="flex items-center justify-between text-xs font-semibold text-muted-foreground">
            <span>Progress Visualizer ({selectedPeriod})</span>
            <div class="flex items-center gap-3 text-[11px]">
              <span class="flex items-center gap-1.5"><span class="w-2.5 h-2.5 rounded-full bg-emerald-500"></span> Actual</span>
              <span class="flex items-center gap-1.5"><span class="w-2.5 h-2.5 rounded-full bg-rose-500"></span> Target ({formatCompactCurrency(convertedActivePeriodTarget)})</span>
              <span class="flex items-center gap-1.5"><span class="w-2.5 h-2.5 rounded-full bg-indigo-500"></span> Stretch ({formatCompactCurrency(convertedActivePeriodStretch)})</span>
            </div>
          </div>

          <div class="relative pt-6 pb-4 bg-card rounded-xl border border-border/60 p-4 shadow-2xs">
            <svg viewBox="0 0 400 130" class="w-full h-auto">
              <!-- Background Bar -->
              <rect x="10" y="30" width="380" height="24" rx="6" fill="var(--color-muted, #f1f5f9)"></rect>
              <!-- Progress Bar -->
              <rect x="10" y="30" width="{convertedActivePeriodStretch > 0 ? Math.min(380, (totalRevenue / convertedActivePeriodStretch) * 380) : 0}" height="24" rx="6" fill="#10b981"></rect>
              <!-- Target Marker line -->
              <line x1="{targetPosX}" y1="20" x2="{targetPosX}" y2="60" stroke="#f43f5e" stroke-width="2" stroke-dasharray="3 3"></line>
              <text x="{targetPosX}" y="14" text-anchor="middle" fill="#f43f5e" font-size="10" font-weight="700">Target: {formatCompactCurrency(convertedActivePeriodTarget)}</text>
              <!-- Stretch Goal Line -->
              <line x1="380" y1="20" x2="380" y2="60" stroke="#6366f1" stroke-width="2" stroke-dasharray="3 3"></line>
              <text x="380" y="78" text-anchor="end" fill="#6366f1" font-size="10" font-weight="700">Stretch: {formatCompactCurrency(convertedActivePeriodStretch)}</text>
            </svg>
          </div>
        </div>

        <!-- Monthly Forecast Sparkline / Trend -->
        <div class="pt-2">
          <span class="text-xs font-bold text-foreground block mb-3">Revenue Trends & Forecast ({selectedPeriod})</span>
          <div class="grid grid-cols-3 sm:grid-cols-6 gap-2">
            {#each monthlyTrendData as item}
              <div class="p-2.5 rounded-xl border border-border/70 bg-muted/20 text-center space-y-1">
                <span class="text-[10px] font-semibold text-muted-foreground block truncate">{item.month}</span>
                <span class="text-xs font-extrabold text-foreground block">{formatCompactCurrency(item.value)}</span>
                <span class="text-[9px] text-emerald-600 font-bold block">Target: {formatCompactCurrency(item.target)}</span>
              </div>
            {/each}
          </div>
        </div>
      </Card.Content>
    </Card.Root>

    <!-- Pipeline Stage Breakdown Card -->
    <Card.Root class="shadow-2xs border-border/80 flex flex-col">
      <Card.Header class="pb-3 border-b border-border/60">
        <div class="flex items-center justify-between">
          <Card.Title class="text-base font-bold text-foreground">Pipeline Stages</Card.Title>
          <Badge variant="outline" class="text-[10px]">{activeDeals.length} deals</Badge>
        </div>
        <p class="text-xs text-muted-foreground">Opportunity Distribution ({selectedPeriod})</p>
      </Card.Header>

      <Card.Content class="pt-4 flex-1 flex flex-col justify-between space-y-4">
        <div class="space-y-3.5">
          {#each stageBreakdown as item}
            <div class="space-y-1">
              <div class="flex items-center justify-between text-xs">
                <div class="flex items-center gap-1.5">
                  <span class="w-2.5 h-2.5 rounded-full {item.bg}"></span>
                  <span class="font-bold text-foreground">{item.label}</span>
                  <span class="text-[10px] text-muted-foreground">({item.prob}%)</span>
                </div>
                <div class="flex items-center gap-2">
                  <span class="text-[11px] font-semibold text-muted-foreground">{item.count} deals</span>
                  <span class="font-extrabold text-foreground">{formatCompactCurrency(item.value)}</span>
                </div>
              </div>

              <!-- Bar Progress -->
              <div class="w-full bg-muted rounded-full h-2 overflow-hidden flex">
                <div 
                  class="{item.bg} h-full rounded-full transition-all duration-500" 
                  style="width: {Math.max(4, item.pctOfTotal)}%"
                ></div>
              </div>
            </div>
          {/each}
        </div>

        <div class="pt-3 border-t border-border/60 bg-muted/20 p-3 rounded-xl flex items-center justify-between text-xs">
          <span class="text-muted-foreground font-medium">Total pipeline value:</span>
          <span class="font-black text-emerald-600 text-sm">{formatCompactCurrency(totalPipelineValue + totalRevenue)}</span>
        </div>
      </Card.Content>
    </Card.Root>
  </div>

  <!-- Row 2: Operational Intelligence (3 Columns) -->
  <div class="grid grid-cols-1 md:grid-cols-3 gap-6">

    <!-- Card 1: Lead Distribution & Zalo Status -->
    <Card.Root class="shadow-2xs border-border/80">
      <Card.Header class="pb-3 border-b border-border/60">
        <div class="flex items-center justify-between">
          <Card.Title class="text-sm font-bold text-foreground">Lead Breakdown</Card.Title>
          <Badge variant="outline" class="text-[10px] font-mono">{leads.length} Leads</Badge>
        </div>
        <p class="text-xs text-muted-foreground">Status & Zalo Verification</p>
      </Card.Header>
      <Card.Content class="pt-4 space-y-4">
        <div class="space-y-2.5">
          {#each leadStatusStats as status}
            {@const pct = leads.length > 0 ? Math.round((status.count / leads.length) * 100) : 0}
            <div class="space-y-1">
              <div class="flex items-center justify-between text-xs">
                <span class="font-semibold text-foreground">{status.label}</span>
                <span class="text-muted-foreground font-mono">{status.count} ({pct}%)</span>
              </div>
              <div class="w-full bg-muted rounded-full h-1.5 overflow-hidden">
                <div class="{status.color} h-full transition-all duration-300" style="width: {pct}%"></div>
              </div>
            </div>
          {/each}
        </div>

        <div class="pt-3 border-t border-border/60 flex items-center justify-between p-3 rounded-xl bg-sky-500/5 border border-sky-500/20">
          <div class="flex items-center gap-2">
            <MessageCircle class="w-4 h-4 text-sky-500" />
            <div>
              <p class="text-xs font-bold text-foreground">Zalo Reachability</p>
              <p class="text-[10px] text-muted-foreground">{zaloVerifiedCount} actual leads</p>
            </div>
          </div>
          <Badge variant="outline" class="border-sky-500/30 text-sky-600 bg-sky-500/10 font-bold">
            {zaloPct}%
          </Badge>
        </div>
      </Card.Content>
    </Card.Root>

    <!-- Card 2: Top Active Deals List -->
    <Card.Root class="shadow-2xs border-border/80">
      <Card.Header class="pb-3 border-b border-border/60">
        <div class="flex items-center justify-between">
          <Card.Title class="text-sm font-bold text-foreground">{m.dashboard_deals_overview()}</Card.Title>
          <Badge variant="secondary" class="text-[10px]">{topActiveDeals.length} active</Badge>
        </div>
        <p class="text-xs text-muted-foreground">Highest value opportunities</p>
      </Card.Header>
      <Card.Content class="pt-4 space-y-3">
        {#if topActiveDeals.length === 0}
          <div class="p-4 rounded-xl border border-dashed border-border text-center text-xs text-muted-foreground py-8">
            {m.dashboard_no_deals()}
          </div>
        {:else}
          {#each topActiveDeals as deal}
            {@const lead = leads.find(l => l.id === deal.lead_id)}
            {@const cfg = stageConfig[deal.stage] || { label: deal.stage, color: 'text-foreground', bg: 'bg-muted' }}
            <div class="p-3 rounded-xl border border-border/80 bg-card hover:border-primary/40 transition-colors space-y-1.5 shadow-2xs">
              <div class="flex items-start justify-between gap-2">
                <h4 class="text-xs font-bold text-foreground truncate">{deal.title}</h4>
                <span class="text-xs font-black text-emerald-600 shrink-0">{formatCurrency(deal.value, (deal.currency as Currency) || 'VND')}</span>
              </div>
              <div class="flex items-center justify-between text-[11px]">
                <span class="text-muted-foreground truncate">{lead ? lead.name : m.kanban_unassigned()}</span>
                <Badge variant="outline" class="text-[9px] py-0 px-1.5 font-semibold border-border">
                  {cfg.label}
                </Badge>
              </div>
            </div>
          {/each}
        {/if}
      </Card.Content>
    </Card.Root>

    <!-- Card 3: Today's Schedule Timeline / Recent System Activities -->
    <Card.Root class="shadow-2xs border-border/80">
      <Card.Header class="pb-3 border-b border-border/60">
        <div class="flex items-center justify-between">
          <Card.Title class="text-sm font-bold text-foreground">{m.dashboard_recent_activity()}</Card.Title>
          <Badge variant="outline" class="text-[10px] font-mono">{schedule.length} logs</Badge>
        </div>
        <p class="text-xs text-muted-foreground">Latest tasks & interactions log</p>
      </Card.Header>
      <Card.Content class="pt-4 space-y-3">
        {#if schedule.length === 0}
          <div class="p-4 rounded-xl border border-dashed border-border text-center text-xs text-muted-foreground py-8">
            {m.dashboard_no_activities()}
          </div>
        {:else}
          {#each schedule as item}
            <div class="flex items-start gap-3 p-3 rounded-xl border border-border/80 bg-card hover:bg-muted/40 transition-colors shadow-2xs">
              <CheckCircle2 class="w-4 h-4 text-emerald-500 mt-0.5 shrink-0" />
              <div class="space-y-0.5 min-w-0 flex-1">
                <p class="text-xs font-semibold text-foreground line-clamp-2">{item.title}</p>
                <span class="text-[10px] text-muted-foreground font-mono">{item.time}</span>
              </div>
            </div>
          {/each}
        {/if}
      </Card.Content>
    </Card.Root>

  </div>
</div>

<!-- Modal Edit Targets & Goals -->
{#if showTargetModal}
  <div class="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4">
    <div class="bg-card border border-border rounded-2xl shadow-2xl w-full max-w-md overflow-hidden animate-in fade-in zoom-in-95 duration-150">
      <div class="px-6 py-4 border-b border-border flex items-center justify-between bg-muted/40">
        <div class="flex items-center gap-2.5">
          <div class="w-8 h-8 rounded-xl bg-primary/10 text-primary flex items-center justify-center shadow-2xs">
            <Target class="w-4 h-4" />
          </div>
          <div>
            <h3 class="font-bold text-base text-foreground">{m.dashboard_target_config_title()}</h3>
            <p class="text-xs text-muted-foreground">{m.dashboard_target_config_subtitle()}</p>
          </div>
        </div>
        <button 
          type="button" 
          onclick={() => showTargetModal = false}
          class="text-muted-foreground hover:text-foreground p-1.5 rounded-lg hover:bg-accent transition-colors"
          aria-label={m.common_close()}
        >
          <X class="w-4 h-4" />
        </button>
      </div>

      <form onsubmit={saveTargets} class="p-6 space-y-4 text-xs">
        <div class="space-y-1.5">
          <label for="target-revenue-input" class="font-bold text-foreground flex items-center gap-1.5">
            <DollarSign class="w-3.5 h-3.5 text-rose-500" />
            {m.dashboard_target_revenue_label()} (₫)
          </label>
          <Input 
            id="target-revenue-input"
            type="number"
            bind:value={editTargetInput}
            step="1000000"
            min="1000000"
            class="h-10 text-xs rounded-xl font-bold text-foreground"
            required
          />
          <p class="text-[11px] text-muted-foreground">{m.dashboard_target_example()}</p>
        </div>

        <div class="space-y-1.5">
          <label for="stretch-revenue-input" class="font-bold text-foreground flex items-center gap-1.5">
            <TrendingUp class="w-3.5 h-3.5 text-indigo-500" />
            {m.dashboard_stretch_revenue_label()} (₫)
          </label>
          <Input 
            id="stretch-revenue-input"
            type="number"
            bind:value={editStretchInput}
            step="1000000"
            min="1000000"
            class="h-10 text-xs rounded-xl font-bold text-indigo-600"
            required
          />
          <p class="text-[11px] text-muted-foreground">{m.dashboard_stretch_example()}</p>
        </div>

        <div class="pt-4 border-t border-border flex items-center justify-end gap-3">
          <Button 
            type="button" 
            variant="outline" 
            onclick={() => showTargetModal = false}
            class="h-9 px-4 text-xs rounded-xl font-semibold"
          >
            {m.common_cancel()}
          </Button>
          <Button 
            type="submit" 
            class="h-9 px-5 text-xs rounded-xl font-bold bg-primary text-primary-foreground hover:bg-primary/90 shadow-md gap-1.5"
          >
            <Save class="w-3.5 h-3.5" />
            <span>{m.dashboard_save_target_btn()}</span>
          </Button>
        </div>
      </form>
    </div>
  </div>
{/if}
