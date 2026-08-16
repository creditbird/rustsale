<script lang="ts">
  import '../index.css';
  import { Toaster } from '$lib/components/ui/sonner';
  import UpdateModal from '$lib/components/UpdateModal.svelte';
  import { updaterService } from '$lib/services/updaterService';
  import { onMount } from 'svelte';

  let { children } = $props();

  onMount(() => {
    // Non-blocking silent background check after 3 seconds
    const timer = setTimeout(() => {
      updaterService.checkForUpdate({ silent: true }).catch((err) => {
        console.warn('[Updater] Background check error:', err);
      });
    }, 3000);

    return () => clearTimeout(timer);
  });
</script>

<Toaster richColors position="top-right" />
<UpdateModal />
{@render children()}
