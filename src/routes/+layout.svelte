<script lang="ts">
  import '../index.css';
  import { Toaster } from '$lib/components/ui/sonner';
  let { children } = $props();
</script>

<Toaster richColors position="top-right" />
{@render children()}
