<script lang="ts">
  import { onMount } from 'svelte';
  import type { IndexedImage, Palette } from '../../../lib/asset-formats';
  import type { MapAnimationDefinition, MapCell, MapLayout } from '../../../lib/map-formats';

  let {
    layout,
    selected,
    layers,
    terrainImages,
    animations,
    palette,
    onselect
  }: {
    layout: MapLayout;
    selected?: MapCell;
    terrainImages: Map<string, IndexedImage>;
    palette: Palette;
    animations: MapAnimationDefinition[];
    layers: {
      objects: boolean;
      prices: boolean;
      events: boolean;
      starts: boolean;
      shops: boolean;
    };
    onselect: (cell: MapCell) => void;
  } = $props();

  // Group-000 terrain artwork is authored as 80×60 sprites over an 80×40
  // isometric footprint. At 100% zoom this preserves the original pixels.
  const tileWidth = 80;
  const tileHeight = 40;
  let canvas: HTMLCanvasElement;
  let viewportWidth = $state(900);
  let viewportHeight = $state(620);
  let zoom = $state(1);
  let panX = $state(0);
  let panY = $state(0);
  let hovered = $state<MapCell>();
  let dragging = false;
  let moved = false;
  let lastX = 0;
  let lastY = 0;
  let imageCache = new Map<string, HTMLCanvasElement>();
  let cachedImages: Map<string, IndexedImage> | undefined;
  let cachedPalette: Palette | undefined;

  function color(index: number, saturation = 58, light = 55) {
    return `hsl(${(index * 53) % 360} ${saturation}% ${light}%)`;
  }

  function project(x: number, y: number) {
    return { x: ((x - y) * tileWidth) / 2, y: ((x + y) * tileHeight) / 2 };
  }

  function diamond(context: CanvasRenderingContext2D, x: number, y: number) {
    context.beginPath();
    context.moveTo(x, y - tileHeight / 2);
    context.lineTo(x + tileWidth / 2, y);
    context.lineTo(x, y + tileHeight / 2);
    context.lineTo(x - tileWidth / 2, y);
    context.closePath();
  }

  function imageCanvas(image: IndexedImage) {
    if (cachedImages !== terrainImages || cachedPalette !== palette) {
      imageCache = new Map();
      cachedImages = terrainImages;
      cachedPalette = palette;
    }
    const cached = imageCache.get(image.id);
    if (cached) return cached;
    const output = document.createElement('canvas');
    output.width = image.width;
    output.height = image.height;
    const context = output.getContext('2d');
    if (!context) return;
    const rgba = context.createImageData(image.width, image.height);
    for (let index = 0; index < image.pixels.length; index += 1) {
      const source = image.pixels[index] * 3;
      const target = index * 4;
      rgba.data[target] = palette[source];
      rgba.data[target + 1] = palette[source + 1];
      rgba.data[target + 2] = palette[source + 2];
      rgba.data[target + 3] = image.alpha?.[index] ?? 255;
    }
    context.putImageData(rgba, 0, 0);
    imageCache.set(image.id, output);
    return output;
  }

  function mapElementImage(group: number, id: number) {
    return terrainImages.get(`${String(group).padStart(3, '0')}/${String(id).padStart(3, '0')}`);
  }

  function animationFor(cell: MapCell) {
    if (cell.regionId !== 15 || cell.objectId === undefined) return;
    return animations.find((animation) => Number(animation.id.split('/')[1]) === cell.objectId);
  }

  function drawPlacedImage(
    context: CanvasRenderingContext2D,
    image: IndexedImage,
    point: { x: number; y: number }
  ) {
    // Placed map-element canvases are X-centred and bottom-anchored. Their
    // transparent bottom padding is authored relative to the diamond's bottom
    // edge, not 20 pixels below it.
    const source = imageCanvas(image);
    if (source) context.drawImage(source, point.x - image.width / 2, point.y + tileHeight / 2 - image.height);
  }

  function eventSymbol(context: CanvasRenderingContext2D, cell: MapCell, x: number, y: number) {
    if (!layers.events || !cell.regionId || cell.regionId === 1) return;
    context.save();
    context.textAlign = 'center';
    context.textBaseline = 'middle';
    context.fillStyle = '#17212b';
    context.strokeStyle = '#17212b';
    context.lineWidth = 1.4;
    context.font = 'bold 8px sans-serif';
    if (cell.regionId === 4) context.fillText('M', x, y);
    else if (cell.regionId === 5) context.fillText('S', x, y);
    else if (cell.regionId === 6) context.fillText('B', x, y);
    else if (cell.regionId === 7) context.fillText('↑', x, y);
    else if (cell.regionId === 8) {
      context.beginPath();
      context.moveTo(x, y + 3);
      context.lineTo(x, y - 4);
      context.moveTo(x, y - 4);
      context.lineTo(x - 3, y - 1);
      context.moveTo(x, y - 4);
      context.lineTo(x + 3, y - 1);
      context.stroke();
    } else if (cell.regionId === 9) {
      context.beginPath();
      context.ellipse(x, y, 6, 3, 0, 0, Math.PI * 2);
      context.stroke();
    } else if (cell.regionId === 10) context.fillText('+', x, y);
    else if (cell.regionId === 11) context.fillText('!', x, y);
    else if (cell.regionId === 14) {
      context.strokeRect(x - 3.5, y - 3.5, 7, 7);
    } else if (cell.regionId === 15) context.fillText('▶', x, y);
    else context.fillText(String(cell.regionId), x, y);
    context.restore();
  }

  function curvedArrow(
    context: CanvasRenderingContext2D,
    source: { x: number; y: number },
    target: { x: number; y: number },
    alpha: number
  ) {
    const lift = Math.max(35, Math.abs(target.x - source.x) * 0.18 + Math.abs(target.y - source.y) * 0.22);
    const control = { x: (source.x + target.x) / 2, y: Math.min(source.y, target.y) - lift };
    context.save();
    context.globalAlpha = alpha;
    context.strokeStyle = '#5b2bbf';
    context.fillStyle = '#5b2bbf';
    context.lineWidth = 2.4;
    context.beginPath();
    context.moveTo(source.x, source.y);
    context.quadraticCurveTo(control.x, control.y, target.x, target.y);
    context.stroke();
    const angle = Math.atan2(target.y - control.y, target.x - control.x);
    context.beginPath();
    context.moveTo(target.x, target.y);
    context.lineTo(target.x - Math.cos(angle - 0.55) * 8, target.y - Math.sin(angle - 0.55) * 8);
    context.lineTo(target.x - Math.cos(angle + 0.55) * 8, target.y - Math.sin(angle + 0.55) * 8);
    context.closePath();
    context.fill();
    context.restore();
  }

  function draw() {
    if (!canvas) return;
    const context = canvas.getContext('2d');
    if (!context) return;
    context.setTransform(1, 0, 0, 1, 0, 0);
    context.clearRect(0, 0, viewportWidth, viewportHeight);
    context.fillStyle = '#e8edf2';
    context.fillRect(0, 0, viewportWidth, viewportHeight);
    context.setTransform(zoom, 0, 0, zoom, panX, panY);

    // Terrain is always the first map layer. Draw in isometric depth order so
    // the lower 20 pixels of raised terrain overlap the next row correctly.
    const depthOrderedCells = [...layout.cells].sort(
      (left, right) => left.x + left.y - (right.x + right.y) || left.x - right.x
    );
    context.imageSmoothingEnabled = false;
    for (const cell of depthOrderedCells) {
      if (cell.terrainId === undefined) continue;
      const point = project(cell.x, cell.y);
      const terrain = mapElementImage(0, cell.terrainId);
      const source = terrain ? imageCanvas(terrain) : undefined;
      if (source) context.drawImage(source, point.x - tileWidth / 2, point.y - tileHeight / 2);
      else {
        diamond(context, point.x, point.y);
        context.fillStyle = color(cell.terrainId, 28, 76);
        context.fill();
        context.strokeStyle = 'rgb(74 94 112 / 0.24)';
        context.lineWidth = 0.7;
        context.stroke();
      }
    }

    if (layers.events) {
      // Event artwork (002/228–259) is authored as a floor layer, so it sits
      // above terrain but below shared interactions and map scenery.
      for (const cell of depthOrderedCells) {
        if (cell.objectId === undefined || cell.objectId < 228 || cell.objectId >= 260) continue;
        const point = project(cell.x, cell.y);
        const artwork = mapElementImage(2, cell.objectId);
        if (artwork) drawPlacedImage(context, artwork, point);
      }

      for (const cell of layout.cells) {
        const point = project(cell.x, cell.y);
        eventSymbol(context, cell, point.x, point.y);
      }
    }

    if (layers.objects) {
      for (const cell of depthOrderedCells) {
        const point = project(cell.x, cell.y);
        const animation = animationFor(cell);
        if (animation?.frameIds[0]) {
          const firstFrame = terrainImages.get(animation.frameIds[0]);
          if (firstFrame) drawPlacedImage(context, firstFrame, point);
          continue;
        }
        // 000–227 is the runtime purchasable-building catalogue and 228–259
        // is event floor artwork. Neither belongs to the initial object layer.
        if (cell.objectId === undefined || cell.objectId < 260) continue;
        const artwork = mapElementImage(2, cell.objectId);
        if (artwork) drawPlacedImage(context, artwork, point);
      }
    }

    if (layers.prices)
      for (const cell of layout.cells) {
        if (cell.regionId !== 1) continue;
        const point = project(cell.x, cell.y);
        context.fillStyle = '#17212b';
        context.font = 'bold 7px sans-serif';
        context.textAlign = 'center';
        context.textBaseline = 'middle';
        context.fillText(`$${cell.routeValue}`, point.x, point.y);
      }

    if (layers.starts)
      for (const start of layout.starts) {
        const point = project(start.x, start.y);
        context.fillStyle = '#1671c5';
        context.fillRect(point.x - 5, point.y - 5, 10, 10);
        context.fillStyle = '#fff';
        context.font = 'bold 7px sans-serif';
        context.textAlign = 'center';
        context.textBaseline = 'middle';
        context.fillText(String(start.player + 1), point.x, point.y);
      }
    if (layers.shops)
      for (const location of layout.specialLocations) {
        const point = project(location.x, location.y);
        context.strokeStyle = '#7a2bc2';
        context.lineWidth = 2;
        diamond(context, point.x, point.y);
        context.stroke();
      }

    // Connection lines are interaction effects, not map artwork. Keep them as
    // the final map layer so objects, prices, starts, and shop markers cannot
    // hide the hole-to-jail relationship.
    if (layers.events) {
      const jailPoint = project(layout.jail.x, layout.jail.y);
      for (const cell of layout.cells.filter((candidate) => candidate.regionId === 9)) {
        const point = project(cell.x, cell.y);
        curvedArrow(context, point, jailPoint, hovered?.index === cell.index ? 0.9 : 0.08);
      }
    }

    for (const [cell, stroke, width] of [
      [hovered, '#4a6b88', 1.6],
      [selected, '#ff8a00', 2.5]
    ] as const) {
      if (!cell) continue;
      const point = project(cell.x, cell.y);
      diamond(context, point.x, point.y);
      context.strokeStyle = stroke;
      context.lineWidth = width;
      context.stroke();
    }
  }

  function cellAt(clientX: number, clientY: number) {
    const bounds = canvas.getBoundingClientRect();
    const screenX = ((clientX - bounds.left) / bounds.width) * viewportWidth;
    const screenY = ((clientY - bounds.top) / bounds.height) * viewportHeight;
    const worldX = (screenX - panX) / zoom;
    const worldY = (screenY - panY) / zoom;
    const x = Math.round(worldX / tileWidth + worldY / tileHeight);
    const y = Math.round(worldY / tileHeight - worldX / tileWidth);
    if (x < 0 || y < 0 || x >= layout.width || y >= layout.height) return;
    return layout.cells[y * layout.width + x];
  }

  function fitView() {
    const contentWidth = ((layout.width + layout.height) * tileWidth) / 2 + tileWidth;
    const contentHeight = ((layout.width + layout.height) * tileHeight) / 2 + tileHeight;
    zoom = Math.min(
      2,
      Math.max(0.18, Math.min((viewportWidth - 50) / contentWidth, (viewportHeight - 50) / contentHeight))
    );
    const center = project((layout.width - 1) / 2, (layout.height - 1) / 2);
    panX = viewportWidth / 2 - center.x * zoom;
    panY = viewportHeight / 2 - center.y * zoom;
  }

  function changeZoom(factor: number, anchorX = viewportWidth / 2, anchorY = viewportHeight / 2) {
    const next = Math.min(5, Math.max(0.15, zoom * factor));
    const worldX = (anchorX - panX) / zoom;
    const worldY = (anchorY - panY) / zoom;
    panX = anchorX - worldX * next;
    panY = anchorY - worldY * next;
    zoom = next;
  }

  function pointerDown(event: PointerEvent) {
    dragging = true;
    moved = false;
    lastX = event.clientX;
    lastY = event.clientY;
    canvas.setPointerCapture(event.pointerId);
  }

  function pointerMove(event: PointerEvent) {
    if (dragging) {
      const dx = event.clientX - lastX;
      const dy = event.clientY - lastY;
      if (Math.abs(dx) + Math.abs(dy) > 1) moved = true;
      panX += dx;
      panY += dy;
      lastX = event.clientX;
      lastY = event.clientY;
    } else hovered = cellAt(event.clientX, event.clientY);
  }

  function pointerUp(event: PointerEvent) {
    if (!moved) {
      const cell = cellAt(event.clientX, event.clientY);
      if (cell) onselect(cell);
    }
    dragging = false;
    canvas.releasePointerCapture(event.pointerId);
  }

  function wheel(event: WheelEvent) {
    event.preventDefault();
    const bounds = canvas.getBoundingClientRect();
    changeZoom(event.deltaY < 0 ? 1.16 : 1 / 1.16, event.clientX - bounds.left, event.clientY - bounds.top);
  }

  $effect(() => {
    draw();
  });

  onMount(() => {
    const observer = new ResizeObserver(([entry]) => {
      viewportWidth = Math.max(320, Math.round(entry.contentRect.width));
    });
    observer.observe(canvas);
    requestAnimationFrame(fitView);
    return () => observer.disconnect();
  });
</script>

<div class="map-canvas-wrap">
  <div class="map-canvas-controls">
    <button aria-label="Zoom out" onclick={() => changeZoom(1 / 1.25)}>−</button>
    <span>{Math.round(zoom * 100)}%</span>
    <button aria-label="Zoom in" onclick={() => changeZoom(1.25)}>+</button>
    <button onclick={fitView}>Fit</button>
    <span class="map-canvas-hint"
      >Drag to pan · wheel to zoom{hovered ? ` · ${hovered.x},${hovered.y}` : ''}</span
    >
  </div>
  <canvas
    class="logical-map"
    bind:this={canvas}
    width={viewportWidth}
    height={viewportHeight}
    onpointerdown={pointerDown}
    onpointermove={pointerMove}
    onpointerup={pointerUp}
    onpointercancel={() => (dragging = false)}
    onpointerleave={() => {
      if (!dragging) hovered = undefined;
    }}
    onwheel={wheel}
    ondblclick={fitView}
    aria-label={`Isometric logical ${layout.width} by ${layout.height} map grid`}
  ></canvas>
</div>
