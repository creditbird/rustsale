<script lang="ts">
  import * as crmService from '$lib/services/crmService';
  import type { Lead, Deal } from '$lib/services/crmService';

  let leads = $state<Lead[]>([]);
  let deals = $state<Deal[]>([]);

  $effect(() => {
    crmService.getLeads().then(data => leads = data).catch(console.error);
    crmService.getDeals().then(data => deals = data).catch(console.error);
  });

  const totalRevenue = $derived(
    deals.filter(d => d.stage === 'Won').reduce((sum, d) => sum + d.value, 0) || 45000
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
    { label: 'Total Revenue Gen', value: formatCurrency(totalRevenue), change: '+12.4%', up: true, icon: 'payments' },
    { label: 'Deals Won', value: String(dealsWon), change: '+2', up: true, icon: 'emoji_events' },
    { label: 'Active Leads', value: String(activeLeadsCount), change: '-4%', up: false, icon: 'people' }
  ]);

  const schedule = [
    { time: '09:00 - 09:45', title: 'Team Sync & Status Update' },
    { time: '10:30 - 11:15', title: 'Product Demo with Acme Corp' },
    { time: '14:00 - 15:00', title: 'Quarterly Review with CloudSync' }
  ];
</script>

<div class="dashboard-container">
  <header class="dashboard-header">
    <div class="header-title-area">
      <h1>Sales Performance</h1>
      <p class="subtitle">Real-time overview of your CRM metrics, deals, and daily schedule</p>
    </div>
  </header>

  <div class="stats-grid">
    {#each stats as stat}
      <div class="stat-card">
        <div class="stat-header">
          <span class="stat-label">{stat.label}</span>
          <span class="material-icons stat-icon">{stat.icon}</span>
        </div>
        <div class="stat-value-container">
          <span class="stat-value">{stat.value}</span>
          <span class="stat-change" class:up={stat.up}>{stat.change}</span>
        </div>
      </div>
    {/each}
  </div>

  <div class="dashboard-grid">
    <div class="dashboard-card">
      <div class="card-header">
        <h3>Revenue Target vs. Actual</h3>
        <span class="badge-success">Active Quarter</span>
      </div>
      <div class="funnel-container">
        <!-- SVG Chart mock for target vs actual -->
        <svg viewBox="0 0 400 150" class="chart-svg">
          <rect x="20" y="30" width="360" height="20" rx="10" fill="var(--color-surface-container-high)"></rect>
          <rect x="20" y="30" width="{Math.min(360, (totalRevenue / 150000) * 360)}" height="20" rx="10" fill="var(--color-success)"></rect>
          <text x="30" y="44" fill="var(--color-on-primary)" font-size="10" font-weight="700">
            Actual: {formatCurrency(totalRevenue)} ({Math.min(100, Math.round((totalRevenue / 150000) * 100))}%)
          </text>
          <line x1="288" y1="15" x2="288" y2="65" stroke="var(--color-danger)" stroke-width="2" stroke-dasharray="3 3"></line>
          <text x="294" y="22" fill="var(--color-danger)" font-size="9" font-weight="700">Target: $120k (80%)</text>
          
          <line x1="360" y1="15" x2="360" y2="65" stroke="var(--color-primary)" stroke-width="2" stroke-dasharray="3 3"></line>
          <text x="320" y="80" fill="var(--color-primary)" font-size="9" font-weight="700">Stretch: $150k</text>
        </svg>
      </div>
    </div>

    <div class="dashboard-card">
      <h3>Today's Schedule</h3>
      <ul class="schedule-list">
        {#each schedule as item}
          <li class="schedule-item">
            <span class="material-icons schedule-icon">check_circle</span>
            <div class="schedule-details">
              <strong>{item.title}</strong>
              <p>{item.time}</p>
            </div>
          </li>
        {/each}
      </ul>
    </div>
  </div>
</div>

<style>
  .dashboard-container {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-lg);
  }
  .dashboard-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .dashboard-header h1 {
    font-size: 24px;
    font-weight: 700;
    color: var(--color-on-surface);
  }
  .subtitle {
    font-size: 14px;
    color: var(--color-on-surface-variant);
    margin-top: var(--spacing-xs);
  }
  .stats-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: var(--spacing-md);
  }
  @media (max-width: 768px) {
    .stats-grid {
      grid-template-columns: 1fr;
    }
  }
  .stat-card {
    background: var(--color-surface-container-lowest);
    border: 1px solid var(--color-outline-variant);
    border-radius: var(--rounded-md);
    padding: var(--spacing-md);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-sm);
    box-shadow: var(--shadow-soft);
    transition: var(--transition-fast);
  }
  .stat-card:hover {
    transform: translateY(-2px);
    border-color: var(--color-outline);
  }
  .stat-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .stat-label {
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--color-on-surface-variant);
    font-weight: 700;
  }
  .stat-icon {
    color: var(--color-on-surface-variant);
    font-size: 20px;
    opacity: 0.7;
  }
  .stat-value-container {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }
  .stat-value {
    font-size: 22px;
    font-weight: 700;
    color: var(--color-on-surface);
  }
  .stat-change {
    font-size: 11px;
    font-weight: bold;
    color: var(--color-danger);
    padding: 2px 6px;
    background: rgba(244, 63, 94, 0.1);
    border-radius: var(--rounded-sm);
  }
  .stat-change.up {
    color: var(--color-success);
    background: rgba(16, 185, 129, 0.1);
  }
  .dashboard-grid {
    display: grid;
    grid-template-columns: 2fr 1fr;
    gap: var(--spacing-md);
  }
  @media (max-width: 900px) {
    .dashboard-grid {
      grid-template-columns: 1fr;
    }
  }
  .dashboard-card {
    background: var(--color-surface-container-lowest);
    border: 1px solid var(--color-outline-variant);
    border-radius: var(--rounded-md);
    padding: var(--spacing-md);
    box-shadow: var(--shadow-soft);
  }
  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: var(--spacing-md);
  }
  .dashboard-card h3 {
    margin: 0;
    margin-bottom: var(--spacing-md);
    font-size: 16px;
    font-weight: 600;
    color: var(--color-on-surface);
  }
  .card-header h3 {
    margin-bottom: 0;
  }
  .badge-success {
    font-size: 10px;
    font-weight: 700;
    padding: 2px 8px;
    background: rgba(16, 185, 129, 0.1);
    color: var(--color-success);
    border-radius: var(--rounded-full);
    text-transform: uppercase;
  }
  .chart-svg {
    width: 100%;
    height: auto;
  }
  .schedule-list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: var(--spacing-sm);
  }
  .schedule-item {
    display: flex;
    gap: var(--spacing-sm);
    padding: var(--spacing-sm);
    background: var(--color-surface-container-low);
    border-radius: var(--rounded-md);
    transition: var(--transition-fast);
  }
  .schedule-item:hover {
    background: var(--color-surface-container);
  }
  .schedule-icon {
    font-size: 20px;
    color: var(--color-success);
  }
  .schedule-details strong {
    font-size: 13px;
    font-weight: 600;
    color: var(--color-on-surface);
    display: block;
  }
  .schedule-details p {
    font-size: 11px;
    color: var(--color-on-surface-variant);
    margin: 0;
    margin-top: 2px;
  }
</style>
