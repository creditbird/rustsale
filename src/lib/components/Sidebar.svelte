<script lang="ts">
  import {
    LayoutDashboard,
    MapPin,
    Users,
    Tags,
    MessageSquare,
    Mail,
    Kanban,
    UserCheck,
    Flame
  } from 'lucide-svelte';

  let { activeTab = $bindable() } = $props();

  const menuItems = [
    { id: 'dashboard', label: 'Dashboard', icon: LayoutDashboard },
    { id: 'scraper', label: 'Google Maps Scraper', icon: MapPin },
    { id: 'leads', label: 'Leads', icon: Users },
    { id: 'categories', label: 'Category Manager', icon: Tags },
    { id: 'outreach', label: 'Zalo Outreach', icon: MessageSquare },
    { id: 'email_outreach', label: 'Email Outreach', icon: Mail },
    { id: 'pipeline', label: 'Deal Pipeline', icon: Kanban },
    { id: 'contacts', label: 'Contacts', icon: UserCheck }
  ];
</script>

<aside class="w-64 h-screen bg-sidebar border-r border-sidebar-border flex flex-col text-sidebar-foreground select-none shrink-0">
  <!-- Brand Header -->
  <div class="h-16 px-5 border-b border-sidebar-border flex items-center gap-3">
    <div class="size-9 rounded-lg bg-primary/10 text-primary flex items-center justify-center font-bold">
      <Flame class="size-5 text-emerald-600" />
    </div>
    <div class="flex flex-col">
      <span class="font-bold text-base tracking-tight text-foreground">RustSale</span>
      <span class="text-[11px] text-muted-foreground font-medium uppercase tracking-wider">CRM & Outreach Pro</span>
    </div>
  </div>

  <!-- Navigation Links -->
  <nav class="flex-1 px-3 py-4 flex flex-col gap-1 overflow-y-auto">
    {#each menuItems as item (item.id)}
      {@const Icon = item.icon}
      {@const isActive = activeTab === item.id}
      <button 
        type="button"
        class="w-full h-10 px-3 rounded-md flex items-center gap-3 text-sm font-medium transition-all duration-150 relative group {isActive ? 'bg-sidebar-accent text-sidebar-accent-foreground font-semibold shadow-xs' : 'text-muted-foreground hover:bg-sidebar-accent/50 hover:text-foreground'}" 
        onclick={() => activeTab = item.id}
      >
        <Icon class="size-4 transition-colors {isActive ? 'text-primary' : 'text-muted-foreground group-hover:text-foreground'}" />
        <span class="truncate">{item.label}</span>
        {#if isActive}
          <div class="absolute left-0 top-2 bottom-2 w-1 bg-primary rounded-r-full"></div>
        {/if}
      </button>
    {/each}
  </nav>

  <!-- User / Status Footer -->
  <div class="p-4 border-t border-sidebar-border flex items-center gap-3 bg-sidebar-accent/20">
    <div class="size-8 rounded-full bg-emerald-500/10 text-emerald-600 border border-emerald-500/20 flex items-center justify-center text-xs font-bold">
      RS
    </div>
    <div class="flex flex-col overflow-hidden">
      <span class="text-xs font-semibold truncate text-foreground">Admin Workspace</span>
      <span class="text-[10px] text-emerald-600 font-medium flex items-center gap-1">
        <span class="size-1.5 rounded-full bg-emerald-500 animate-pulse"></span> System Active
      </span>
    </div>
  </div>
</aside>
