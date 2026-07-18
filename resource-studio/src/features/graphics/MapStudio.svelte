<script lang="ts">
  import { onMount } from 'svelte';
  import { diagnosticPalette, type IndexedImage } from '../../lib/asset-formats';
  import {
    eventClassName,
    type MapAnimationDefinition,
    type MapCell,
    type MapLayout,
    type MapObject
  } from '../../lib/map-formats';
  import { downloadBlob } from '../../lib/browser-download';
  import { encodeIndexedPng, transparencyIndex } from '../../lib/indexed-png';
  import { storedZip } from '../../lib/stored-zip';
  import IndexedCanvas from './components/IndexedCanvas.svelte';
  import AssetTile from './components/AssetTile.svelte';
  import MapGrid from './components/MapGrid.svelte';

  const pageSize = 96;
  type PreparedImage = Omit<IndexedImage, 'pixels' | 'alpha' | 'palette' | 'pcxHeader'> & {
    pixels: string;
    alpha?: string;
    palette?: string;
    pcxHeader?: string;
  };
  type Entry = Pick<IndexedImage, 'id' | 'width' | 'height' | 'kind'> & { page: number };
  type MapInfo = {
    id: number;
    name: string;
    width: number;
    height: number;
    layoutUrl: string;
    elements: { name: string; count: number; pages: number; entries: Entry[] };
    palette?: string;
    preview?: PreparedImage;
    objectCount: number;
    animationCount: number;
  };
  type Category = 'terrain' | 'raw' | 'properties' | 'events' | 'shared' | 'map' | 'frames' | 'animations';

  let maps = $state<MapInfo[]>([]);
  let mapId = $state(0);
  let layout = $state<MapLayout>();
  let animations = $state<MapAnimationDefinition[]>([]);
  let images = $state(new Map<string, IndexedImage>());
  let loadedPages = $state(new Set<number>());
  let page = $state(0);
  let query = $state('');
  let category = $state<Category>('properties');
  let placement = $state('all');
  let familyFilter = $state('all');
  let regionFilter = $state('all');
  let selected = $state<IndexedImage>();
  let selectedCell = $state<MapCell>();
  let selectedIds = $state(new Set<string>());
  let fitPreviews = $state(true);
  let busy = $state(false);
  let status = $state('Loading prepared map catalogues…');
  let error = $state('');
  let layers = $state({
    objects: true,
    prices: true,
    events: true,
    starts: true,
    shops: true
  });

  const current = $derived(maps.find((map) => map.id === mapId));
  const palette = $derived(fromBase64(current?.palette) ?? diagnosticPalette());
  const preview = $derived(current?.preview ? readPreparedImage(current.preview) : undefined);
  const objects = $derived(new Map((layout?.objects ?? []).map((object) => [object.assetId, object])));
  const selectedObject = $derived(selected ? objects.get(selected.id) : undefined);
  const selectedCellObject = $derived(
    selectedCell?.objectId === undefined
      ? undefined
      : objects.get(`002/${String(selectedCell.objectId).padStart(3, '0')}`)
  );
  const selectedAnimation = $derived(
    selectedCell?.regionId === 15 && selectedCell.objectId !== undefined
      ? animations.find((animation) => Number(animation.id.split('/')[1]) === selectedCell!.objectId)
      : undefined
  );
  const families = $derived([...new Set((layout?.objects ?? []).map((object) => object.family))].sort());
  const regions = $derived(
    [...new Set((layout?.cells ?? []).map((cell) => cell.regionId).filter(Boolean))].sort((a, b) => a - b)
  );

  function fromBase64(value: string | undefined) {
    if (!value) return undefined;
    const raw = atob(value);
    return Uint8Array.from(raw, (character) => character.charCodeAt(0));
  }
  function cellRawBytes(cell: MapCell) {
    return cell.rawWords
      .flatMap((word) => [word & 0xff, (word >>> 8) & 0xff, (word >>> 16) & 0xff, word >>> 24])
      .map((byte) => byte.toString(16).padStart(2, '0'))
      .join(' ');
  }
  function cellRawWords(cell: MapCell) {
    return cell.rawWords
      .map(
        (word, index) =>
          `+0x${(index * 4).toString(16).padStart(2, '0')}: 0x${word.toString(16).padStart(8, '0')}`
      )
      .join(' · ');
  }
  function cellWordSplit(cell: MapCell) {
    const objectWord = cell.rawWords[0];
    const terrainWord = cell.rawWords[1];
    const objectId = objectWord >>> 16;
    const terrainId = terrainWord >>> 16;
    return `+0x00 object ID ${objectId === 0xffff ? 'none (0xffff)' : objectId}, flags 0x${(objectWord & 0xffff).toString(16).padStart(4, '0')} · +0x04 terrain ID ${terrainId === 0xffff ? 'none (0xffff)' : terrainId}, flags 0x${(terrainWord & 0xffff).toString(16).padStart(4, '0')} · +0x08 value ${cell.rawWords[2]} · +0x0c event class ${cell.rawWords[3]}`;
  }
  function terrainFlagNote(flags: number) {
    if (flags === 0x80) return 'ordinary class-0 path candidate';
    if (flags === 0x85) return 'event-path candidate';
    if (flags === 0xc0) return 'mixed property/scenery flag; not simply unwalkable';
    if (flags === 0xc5) return 'jail exit/boundary candidate';
    return 'unverified';
  }
  function readPreparedImage(image: PreparedImage): IndexedImage {
    return {
      ...image,
      pixels: fromBase64(image.pixels)!,
      alpha: fromBase64(image.alpha),
      palette: fromBase64(image.palette),
      pcxHeader: fromBase64(image.pcxHeader)
    };
  }
  function thumbnailScale(image: IndexedImage) {
    const largestSide = Math.max(image.width, image.height);
    return largestSide < 48 ? Math.min(3, 48 / largestSide) : 1;
  }
  function entryCategory(entry: Entry): Exclude<Category, 'animations'> {
    const [group, child = '0'] = entry.id.split('/').map(Number);
    if (group === 0) return 'terrain';
    if (group === 1) return 'raw';
    if (group === 3) return 'frames';
    if (Number(child) < 228) return 'properties';
    if (Number(child) < 260) return 'events';
    if (Number(child) < 298) return 'shared';
    return 'map';
  }
  function objectCategory(id: number): Category {
    if (id < 228) return 'properties';
    if (id < 260) return 'events';
    if (id < 298) return 'shared';
    return 'map';
  }
  function objectFor(entry: Entry) {
    return objects.get(entry.id);
  }
  function objectRegions(object: MapObject | undefined) {
    if (!object || !layout) return [];
    return [...new Set(object.placements.map((item) => layout!.cells[item.cellIndex].regionId))];
  }
  function placedLayoutAssetIds(mapLayout: MapLayout, definitions: MapAnimationDefinition[]) {
    const ids = new Set<string>();
    for (const cell of mapLayout.cells) {
      if (cell.objectId === undefined) continue;
      if (cell.regionId === 15) {
        const definition = definitions.find(
          (animation) => Number(animation.id.split('/')[1]) === cell.objectId
        );
        if (definition?.frameIds[0]) ids.add(definition.frameIds[0]);
      } else if (cell.objectId >= 228) ids.add(`002/${String(cell.objectId).padStart(3, '0')}`);
    }
    return [...ids];
  }
  const filtered = $derived(
    (current?.elements.entries ?? []).filter((entry) => {
      if (category === 'animations' || entryCategory(entry) !== category) return false;
      const object = objectFor(entry);
      if (placement === 'placed' && !object?.placements.length) return false;
      if (placement === 'unplaced' && object?.placements.length) return false;
      if (familyFilter !== 'all' && object?.family !== familyFilter) return false;
      if (regionFilter !== 'all' && !objectRegions(object).includes(Number(regionFilter))) return false;
      const term = query.trim().toLowerCase();
      return (
        !term ||
        [
          entry.id,
          `${entry.width}x${entry.height}`,
          object?.name,
          object?.family,
          object?.sourcePath,
          object?.placements.map((item) => `${item.x},${item.y}`).join(' ')
        ].some((value) => value?.toLowerCase().includes(term))
      );
    })
  );
  const pages = $derived(Math.max(1, Math.ceil(filtered.length / pageSize)));
  const visibleEntries = $derived(filtered.slice(page * pageSize, (page + 1) * pageSize));
  const visible = $derived(
    visibleEntries.map((entry) => images.get(entry.id)).filter(Boolean) as IndexedImage[]
  );

  async function inflateJson<T>(url: string): Promise<T> {
    const response = await fetch(url);
    if (!response.ok) throw new Error(`${url} returned HTTP ${response.status}`);
    if (!response.body) throw new Error(`${url} has no response body`);
    const buffer = await new Response(
      response.body.pipeThrough(new DecompressionStream('gzip'))
    ).arrayBuffer();
    return JSON.parse(new TextDecoder().decode(buffer)) as T;
  }
  async function loadPage(nextPage: number) {
    if (!current || loadedPages.has(nextPage)) return;
    const payload = await inflateJson<{ version: number; images: PreparedImage[] }>(
      `/game/prepared/${current.elements.name}/page-${nextPage}.prepared`
    );
    if (payload.version !== 1) throw new Error('Prepared map page is invalid.');
    images = new Map([
      ...images,
      ...payload.images.map((image) => {
        const decoded = readPreparedImage(image);
        return [decoded.id, decoded] as [string, IndexedImage];
      })
    ]);
    loadedPages = new Set([...loadedPages, nextPage]);
  }
  async function loadAssets(ids: string[]) {
    if (!current) return;
    const wanted = new Set(ids);
    const pagesToLoad = new Set(
      current.elements.entries.filter((entry) => wanted.has(entry.id)).map((entry) => entry.page)
    );
    await Promise.all([...pagesToLoad].map(loadPage));
  }
  async function loadVisible() {
    await Promise.all([...new Set(visibleEntries.map((entry) => entry.page))].map(loadPage));
  }
  async function chooseMap(next: number) {
    mapId = next;
    layout = undefined;
    animations = [];
    images = new Map();
    loadedPages = new Set();
    selectedIds = new Set();
    selected = undefined;
    selectedCell = undefined;
    page = 0;
    query = '';
    familyFilter = 'all';
    regionFilter = 'all';
    placement = 'all';
    error = '';
    status = `Loading map ${next}…`;
    const info = maps.find((map) => map.id === next);
    if (!info) return;
    try {
      const payload = await inflateJson<{
        version: number;
        layout: MapLayout;
        animations: MapAnimationDefinition[];
      }>(info.layoutUrl);
      if (payload.version !== 1) throw new Error('Prepared map layout is invalid.');
      if (mapId !== next) return;
      layout = payload.layout;
      animations = payload.animations;
      status = `Decoded ${layout.width}×${layout.height} cells, ${layout.objects.length} named objects, ${layout.starts.length} starts, and ${layout.specialLocations.length} special-location records.`;
      const terrainIds = info.elements.entries
        .filter((entry) => entry.id.startsWith('000/'))
        .map((entry) => entry.id);
      await Promise.all([
        loadVisible(),
        loadAssets([...terrainIds, ...placedLayoutAssetIds(payload.layout, payload.animations)])
      ]);
    } catch (cause) {
      if (mapId !== next) return;
      error = cause instanceof Error ? cause.message : String(cause);
    }
  }
  async function selectCell(cell: MapCell) {
    selectedCell = cell;
    if (cell.objectId !== undefined) await loadAssets([`002/${String(cell.objectId).padStart(3, '0')}`]);
  }
  function resetFilters(next: Category) {
    category = next;
    page = 0;
    query = '';
    familyFilter = 'all';
    regionFilter = 'all';
    placement = 'all';
  }
  function toggle(id: string) {
    const next = new Set(selectedIds);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    selectedIds = next;
  }
  function selectFiltered() {
    selectedIds = new Set(filtered.map((entry) => entry.id));
  }
  async function selectAnimationFrames(animation: MapAnimationDefinition) {
    await loadAssets(animation.frameIds);
    selectedIds = new Set(animation.frameIds);
  }
  async function exportSelected() {
    if (!selectedIds.size) return;
    busy = true;
    error = '';
    try {
      await loadAssets([...selectedIds]);
      const picks = [...selectedIds].map((id) => images.get(id)).filter(Boolean) as IndexedImage[];
      const entries: { name: string; bytes: Uint8Array }[] = [];
      for (const image of picks) {
        const alpha = image.alpha ?? new Uint8Array(image.pixels.length).fill(255);
        entries.push({
          name: `${image.id.replace('/', '-')}.png`,
          bytes: await encodeIndexedPng(
            { width: image.width, height: image.height, pixels: image.pixels, alpha, palette },
            transparencyIndex(image.pixels, alpha)
          )
        });
      }
      downloadBlob(storedZip(entries), `map${String(mapId).padStart(4, '0')}-${category}.zip`);
      status = `Exported ${entries.length} indexed map PNGs.`;
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      busy = false;
    }
  }
  onMount(async () => {
    try {
      const response = await fetch('/game/prepared/maps.json');
      if (!response.ok)
        throw new Error('Map data is not staged. Run the language dev command after adding a map pair.');
      const payload = (await response.json()) as { version: number; maps: MapInfo[] };
      maps = payload.maps;
      if (maps.length) await chooseMap(maps[0].id);
      else status = 'No paired mapNNNN.dat and mapElemNNNN.dat files were staged.';
    } catch (cause) {
      error = cause instanceof Error ? cause.message : String(cause);
      status = 'Map catalogue loading failed.';
    }
  });
  $effect(() => {
    if (current && visibleEntries.length && category !== 'animations') void loadVisible();
  });
  $effect(() => {
    if (category === 'animations') void loadAssets(animations.flatMap((animation) => animation.frameIds));
  });
</script>

<section class="map-studio">
  <div class="map-header">
    <label
      >Map <select
        value={mapId}
        onchange={(event) => void chooseMap(Number((event.currentTarget as HTMLSelectElement).value))}
        >{#each maps as map (map.id)}<option value={map.id}>#{map.id} · {map.name}</option>{/each}</select
      ></label
    >
    {#if current}<span
        >{current.width}×{current.height} cells · {current.objectCount} named objects · {current.animationCount}
        animation definitions</span
      >{/if}
  </div>
  {#if error}<p class="error">{error}</p>{/if}
  <p class="status">{status}</p>
  {#if current && layout}
    <section class="map-overview">
      <article>
        {#if preview}<span class="map-preview"
            ><IndexedCanvas image={preview} palette={preview.palette ?? palette} /></span
          >{/if}
        <div>
          <strong>{current.name}</strong><span
            >{layout.previewSourcePath ?? 'Embedded map preview'} · group-005 VGA palette</span
          >
        </div>
      </article>
      <article>
        <strong>Evidence policy</strong><span
          >Coordinates, references, dimensions, filenames, starts, land prices, shop records, and animation
          links are confirmed by file correlation and gameplay observations. Flag bits, unknown event classes,
          two non-land raw values, and animation control words remain unverified.</span
        >
      </article>
    </section>

    <section class="layout-panel">
      <div class="layout-column">
        <div class="layout-map">
          <div class="layout-title">
            <strong>Layout</strong><span>{layout.width} × {layout.height} cells</span>
          </div>
          <MapGrid
            {layout}
            selected={selectedCell}
            {layers}
            terrainImages={images}
            {animations}
            {palette}
            onselect={(cell) => void selectCell(cell)}
          />
        </div>
        <div class="layout-layers">
          <strong>Layers</strong>
          <div class="layout-layer-options">
            {#each [['objects', 'Objects'], ['prices', 'Prices'], ['events', 'Events'], ['starts', 'Starts'], ['shops', 'Shop']] as option (option[0])}<label
                ><input
                  type="checkbox"
                  checked={layers[option[0] as keyof typeof layers]}
                  onchange={(event) =>
                    (layers = {
                      ...layers,
                      [option[0]]: (event.currentTarget as HTMLInputElement).checked
                    })}
                />{option[1]}</label
              >{/each}
          </div>
        </div>
      </div>
      <div class="layout-inspector">
        <strong>{selectedCell ? `Cell ${selectedCell.x}, ${selectedCell.y}` : 'Select a map cell'}</strong>
        {#if selectedCell}<dl>
            <dt>Object</dt>
            <dd>{selectedCellObject ? `#${selectedCellObject.id} · ${selectedCellObject.name}` : 'none'}</dd>
            {#if selectedCell.objectId === 0}<dt>MB1 role</dt>
              <dd>
                Generic map-block artwork; frequently layered under terrain/scenery <em>candidate role</em>
              </dd>{/if}
            <dt>Terrain</dt>
            <dd>{selectedCell.terrainId ?? 'none'}</dd>
            <dt>Terrain flags</dt>
            <dd>
              0x{selectedCell.terrainFlags.toString(16)} <em>{terrainFlagNote(selectedCell.terrainFlags)}</em>
            </dd>
            <dt>Object flags</dt>
            <dd>0x{selectedCell.objectFlags.toString(16)} <em>unverified</em></dd>
            {#if selectedCell.regionId === 1}<dt>Land price</dt>
              <dd>{selectedCell.routeValue}</dd>{:else}<dt>Raw value</dt>
              <dd>
                {selectedCell.routeValue}
                {#if selectedCell.routeValue}<em>unverified</em>{/if}
              </dd>{/if}
            <dt>Event class</dt>
            <dd>{selectedCell.regionId} · {eventClassName(selectedCell.regionId)}</dd>
            <dt>Source offset</dt>
            <dd>0x{selectedCell.offset.toString(16)}</dd>
            <dt>Raw 16 bytes</dt>
            <dd><code>{cellRawBytes(selectedCell)}</code></dd>
            <dt>Raw words</dt>
            <dd><code>{cellRawWords(selectedCell)}</code></dd>
            <dt>Word split</dt>
            <dd><code>{cellWordSplit(selectedCell)}</code></dd>
          </dl>
          {#if selectedAnimation}<p class="animation-link">
              Marker #{selectedCellObject?.id} selects {selectedAnimation.id}: {selectedAnimation
                .frameIds[0]}–{selectedAnimation.frameIds.at(-1)}
            </p>
            <button onclick={() => (category = 'animations')}>Open animation definition</button>{/if}
          {#if selectedCellObject}<button
              onclick={() => {
                category = objectCategory(selectedCellObject.id);
                query = selectedCellObject.name;
                page = 0;
              }}>Find object in catalogue</button
            >{/if}{/if}
      </div>
    </section>

    <section class="map-points">
      <article>
        <strong>Confirmed player starts</strong>{#each layout.starts as start (start.player)}<button
            onclick={() => void selectCell(layout!.cells[start.y * layout!.width + start.x])}
            >Player {start.player + 1}: ({start.x}, {start.y}, {start.z}) · direction code {start.directionCode}</button
          >{/each}
        <button
          onclick={() => void selectCell(layout!.cells[layout!.jail.y * layout!.width + layout!.jail.x])}
          >Jail entry: ({layout.jail.x}, {layout.jail.y}) · direction code {layout.jail.directionCode}</button
        ><span
          >Approach: bomb ({layout.jail.bombX}, {layout.jail.bombY}) → {layout.jail.approachCells
            .map((cell) => `(${cell.x}, ${cell.y})`)
            .join(' → ')}</span
        >
      </article>
      <article>
        <strong>Confirmed shop records</strong
        >{#each layout.specialLocations as location (location.index)}<details>
            <summary>Shop #{location.index}: ({location.x}, {location.y}, {location.z})</summary><code
              >{location.parameterWords
                .map((word, index) => `+0x${(12 + index * 4).toString(16)} ${word}`)
                .join('\n')}</code
            >
          </details>{/each}
      </article>
    </section>

    <nav class="map-categories" aria-label="Map asset category">
      {#each [['terrain', 'Terrain'], ['raw', 'Raw tiles'], ['properties', 'Runtime properties (002/000–227)'], ['events', 'Event artwork (002/228–259)'], ['shared', 'Shared interactions (002/260–297)'], ['map', 'Map scenery (002/298+)'], ['frames', 'Animation frames'], ['animations', 'Frame groups']] as item (item[0])}<button
          class:active={category === item[0]}
          onclick={() => resetFilters(item[0] as Category)}>{item[1]}</button
        >{/each}
    </nav>

    {#if category === 'animations'}
      <section class="animation-list">
        {#each animations as animation (animation.id)}<article>
            <header>
              <div>
                <strong>Definition #{animation.id}</strong><span
                  >{animation.width}×{animation.height} · origin {animation.originX},{animation.originY} · {animation
                    .frameIds.length} referenced frames</span
                >
              </div>
              <button onclick={() => void selectAnimationFrames(animation)}>Select frames</button>
            </header>
            <div class="frame-strip">
              {#each animation.frameIds as id, index (`${animation.id}-${index}`)}{#if images.get(id)}<button
                    onclick={() => (selected = images.get(id))}
                    ><IndexedCanvas image={images.get(id)!} {palette} fitVisible /><small>{id}</small></button
                  >{/if}{/each}
            </div>
            <details>
              <summary>Raw control and per-frame fields · unverified</summary><code
                >Control: {animation.controlWords.join(', ')}\n{animation.frames
                  .map(
                    (frame) =>
                      `${frame.assetId} @0x${frame.sourceOffset.toString(16)}: ${frame.rawWords.join(', ')}`
                  )
                  .join('\n')}\nFooter bytes: {animation.footerBytes.length}</code
              >
            </details>
          </article>{/each}
      </section>
    {:else}
      <section class="map-toolbar">
        <label
          >Search<input
            type="search"
            placeholder="ID, BMP name, family, size, or x,y"
            bind:value={query}
            oninput={() => (page = 0)}
          /></label
        >{#if ['properties', 'events', 'shared', 'map'].includes(category)}<label
            >Placement<select bind:value={placement} onchange={() => (page = 0)}
              ><option value="all">All</option><option value="placed">Placed</option><option value="unplaced"
                >Unplaced</option
              ></select
            ></label
          ><label
            >Family<select bind:value={familyFilter} onchange={() => (page = 0)}
              ><option value="all">All families</option>{#each families as family (family)}<option
                  value={family}>{family}</option
                >{/each}</select
            ></label
          ><label
            >Event class <select bind:value={regionFilter} onchange={() => (page = 0)}
              ><option value="all">All</option>{#each regions as region (region)}<option
                  value={String(region)}>{region} · {eventClassName(region)}</option
                >{/each}</select
            ></label
          >{/if}<label class="fit-previews"
          ><span>Fit artwork</span><input type="checkbox" bind:checked={fitPreviews} /></label
        ><button onclick={selectFiltered}>Select filtered</button><button
          disabled={busy || !selectedIds.size}
          onclick={exportSelected}>Export {selectedIds.size} PNGs</button
        >
      </section>
      <p class="count">Showing {visible.length} of {filtered.length} {category} entries.</p>
      <section class="grid">
        {#each visible as image (image.id)}<AssetTile
            {image}
            {palette}
            fitVisible={fitPreviews}
            scale={thumbnailScale(image)}
            modified={selectedIds.has(image.id)}
            checked={selectedIds.has(image.id)}
            onopen={() => (selected = image)}
            oncheck={() => toggle(image.id)}
          />{/each}
      </section>
      <nav class="bottom-nav">
        <span>Page <b>{page + 1}</b> / {pages}</span><button disabled={page === 0} onclick={() => (page -= 1)}
          >←</button
        ><button disabled={page + 1 >= pages} onclick={() => (page += 1)}>→</button>
      </nav>
    {/if}
  {/if}
</section>

{#if selected}<div class="modal">
    <div class="modal-panel map-object-modal" role="dialog" aria-modal="true">
      <header>
        <div>
          <strong>#{selected.id}{selectedObject ? ` · ${selectedObject.name}` : ''}</strong><span
            >{selected.width} × {selected.height} · format 0x{selected.magic?.toString(16) ?? 'raw'}</span
          >
        </div>
        <button onclick={() => (selected = undefined)}>Close</button>
      </header>
      <div class="large-preview"><IndexedCanvas image={selected} {palette} /></div>
      {#if selectedObject}<dl>
          <dt>Source</dt>
          <dd>{selectedObject.sourcePath}</dd>
          <dt>Family</dt>
          <dd>{selectedObject.family}</dd>
          <dt>Catalogue</dt>
          <dd>{objectCategory(selectedObject.id)}</dd>
          <dt>Placements</dt>
          <dd>
            {selectedObject.placements.length
              ? selectedObject.placements.map((item) => `(${item.x}, ${item.y})`).join(', ')
              : 'not directly placed in this map grid'}
          </dd>
        </dl>{/if}
    </div>
  </div>{/if}
