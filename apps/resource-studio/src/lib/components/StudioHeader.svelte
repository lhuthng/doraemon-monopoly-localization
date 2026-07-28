<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    title,
    description,
    active,
    className = '',
    children
  }: {
    title: string;
    description: string;
    active: 'translation' | 'graphics' | 'fonts';
    className?: string;
    children?: Snippet;
  } = $props();
</script>

<header class={`studio-header studio-hero ${className}`}>
  <div class="studio-header-copy">
    <p class="eyebrow">Doraemon Monopoly · resource studio</p>
    <h1>{title}</h1>
    <p class="subtle">{description}</p>
  </div>
  <nav class="studio-switcher" aria-label="Resource studios">
    <a class:active={active === 'translation'} href="/" data-route>Translation</a>
    <a class:active={active === 'graphics'} href="/assets" data-route>Graphics</a>
    <a class:active={active === 'fonts'} href="/fonts" data-route>Fonts</a>
  </nav>
  {#if children}
    <div class="studio-header-actions">{@render children()}</div>
  {/if}
</header>
