<script lang="ts">
  import App from './App.svelte';
  import AssetViewer from './AssetViewer.svelte';

  let pathname = $state(window.location.pathname.replace(/\/+$/, '') || '/');

  function updateRoute() {
    pathname = window.location.pathname.replace(/\/+$/, '') || '/';
    window.scrollTo({ top: 0 });
  }

  function followInternalLink(event: MouseEvent) {
    if (event.defaultPrevented || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
    const target = event.target as HTMLElement;
    const anchor = target.closest<HTMLAnchorElement>('a[data-route]');
    if (!anchor || anchor.origin !== window.location.origin) return;
    event.preventDefault();
    history.pushState({}, '', anchor.href);
    updateRoute();
  }
</script>

<svelte:window onpopstate={updateRoute} onclick={followInternalLink} />

{#if pathname === '/assets'}
  <AssetViewer />
{:else}
  <App />
{/if}
