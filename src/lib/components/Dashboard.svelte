<script lang="ts">
  import * as crmService from '$lib/services/crmService';
  import type { Lead, Deal } from '$lib/services/crmService';
  import * as Card from '$lib/components/ui/card';
  import { Badge } from '$lib/components/ui/badge';
  import { DollarSign, Trophy, Users, CheckCircle2, TrendingUp, TrendingDown, AlertCircle } from 'lucide-svelte';

  let leads = $state<Lead[]>([]);
  let deals = $state<Deal[]>([]);
  let errorMsg = $state<string | null>(null);

  $effect(() => {
    Promise.all([crmService.getLeads(), crmService.getDeals()])
      .then(([leadsData, dealsData]) => {
        leads = leadsData;
        deals = dealsData;
        errorMsg = null;
      })
      .catch((err) => {
        errorMsg = err instanceof Error ? err.message : String(err);
      });
  });

  const totalRevenue = $derived(
    deals.filter(d => d.stage === 'Won').reduce((sum, d) => sum + d.value, 0)
  );

  const dealsWon = $derived(
    deals.filter(d => d.stage === 'Won').length
  );

  const activeLeadsCount = $derived(
    leads.filter(l => l.status !== 'Converted' && l.status !== 'Lost').length
  );

  function formatCurrency(val: number) {
    return new Intl.NumberFormat('en-US', {
      style: 'currency',
      currency: 'USD',
      minimumFractionDigits: 2,
      maximumFractionDigits: 2
    }).format(val);
  }

  const stats = $derived([
    { label: 'Total Revenue Gen', value: formatCurrency(totalRevenue), change: '+12.4%', up: true, icon: DollarSign },
    { label: 'Deals Won', value: String(dealsWon), change: '+2', up: true, icon: Trophy },
    { label: 'Active Leads', value: String(activeLeadsCount), change: '-4%', up: false, icon: Users }
  ]);

  const schedule = [
    { time: '09:00 - 09:45', title: 'Team Sync & Status Update' },
    { time: '10:30 - 11:15', title: 'Product Demo with Acme Corp' },
    { time: '14:00 - 15:00', title: 'Quarterly Review with CloudSync' }
  ];
</script>

<div class="space-y-6">
  {#if errorMsg}
    <div class="p-4 rounded-lg border border-destructive/30 bg-destructive/10 text-destructive text-sm flex items-center gap-3">
      <AlertCircle class="w-5 h-5 shrink-0" />
      <span>Failed to load CRM data: {errorMsg}</span>
    </div>
  {/if}

  <!-- Header -->
  <div class="flex items-center justify-between">
    <div>
      <h2 class="text-xl font-bold text-foreground tracking-tight">Sales Performance</h2>
      <p class="text-xs text-muted-foreground mt-0.5">Real-time overview of CRM metrics, deals, and daily schedule</p>
    </div>
  </div>

  <!-- KPI Metrics Cards -->
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

  <!-- Secondary Grid (Revenue Target & Schedule) -->
  <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
    <!-- Revenue Target Chart Card -->
    <Card.Root class="lg:col-span-2">
      <Card.Header class="flex flex-row items-center justify-between pb-4">
        <div>
          <Card.Title class="text-base font-semibold">Revenue Target vs. Actual</Card.Title>
          <p class="text-xs text-muted-foreground">Q3 Performance progress</p>
        </div>
        <Badge variant="outline" class="border-emerald-500/30 text-emerald-600 bg-emerald-500/10 uppercase text-[10px] tracking-wider">
          Active Quarter
        </Badge>
      </Card.Header>
      <Card.Content>
        <div class="space-y-4">
          <svg viewBox="0 0 400 130" class="w-full h-auto">
            <!-- Background Bar -->
            <rect x="10" y="30" width="380" height="24" rx="6" fill="var(--color-muted, #f1f5f9)"></rect>
            <!-- Progress Bar -->
            <rect x="10" y="30" width="{Math.min(380, (totalRevenue / 150000) * 380)}" height="24" rx="6" fill="#10b981"></rect>
            <!-- Progress Label -->
            <text x="20" y="46" fill="#ffffff" font-size="11" font-weight="700">
              Actual: {formatCurrency(totalRevenue)} ({Math.min(100, Math.round((totalRevenue / 150000) * 100))}%)
            </text>
            <!-- Target Marker -->
            <line x1="314" y1="20" x2="314" y2="60" stroke="#f43f5e" stroke-width="2" stroke-dasharray="3 3"></line>
            <text x="314" y="14" text-anchor="middle" fill="#f43f5e" font-size="10" font-weight="700">Target: $120k</text>
            <!-- Stretch Goal -->
            <line x1="380" y1="20" x2="380" y2="60" stroke="#6366f1" stroke-width="2" stroke-dasharray="3 3"></line>
            <text x="380" y="78" text-anchor="end" fill="#6366f1" font-size="10" font-weight="700">Stretch: $150k</text>
          </svg>
        </div>
      </Card.Content>
    </Card.Root>

    <!-- Schedule Card -->
    <Card.Root>
      <Card.Header class="pb-3">
        <Card.Title class="text-base font-semibold">Today's Schedule</Card.Title>
        <p class="text-xs text-muted-foreground">Upcoming events and meetings</p>
      </Card.Header>
      <Card.Content class="space-y-3">
        {#each schedule as item}
          <div class="flex items-start gap-3 p-3 rounded-lg border border-border bg-muted/30 hover:bg-muted/60 transition-colors">
            <CheckCircle2 class="w-4 h-4 text-emerald-500 mt-0.5 shrink-0" />
            <div class="space-y-0.5">
              <p class="text-xs font-semibold text-foreground">{item.title}</p>
              <span class="text-[11px] text-muted-foreground">{item.time}</span>
            </div>
          </div>
        {/each}
      </Card.Content>
    </Card.Root>
  </div>
</div>
