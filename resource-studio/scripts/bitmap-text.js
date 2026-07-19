import fs from 'node:fs/promises';
import path from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import sharp from 'sharp';

const execFileAsync = promisify(execFile);

const [, , atlasImagePath, atlasJsonPath, textArgument, ...optionArguments] =
  process.argv;

// Accept shell-style options after the text as a convenience. For example:
//   bun run bitmap-text font.png font.json "TEXT" OUTLINES=0,0,0,2,r
// Environment variables before the command remain supported as well.
for (const argument of optionArguments) {
  if (argument === '--clipboard' || argument === '--clipboard-only') {
    process.env.CLIPBOARD = '1';
    continue;
  }

  const separator = argument.indexOf('=');

  if (separator <= 0) {
    continue;
  }

  const name = argument.slice(0, separator);
  const value = argument.slice(separator + 1);

  if (
    ['GLYPH_SCALE', 'OUTLINES', 'OUTPUT_DIR', 'OUTPUT_NAME', 'CLIPBOARD', 'CLIPBOARD_ONLY'].includes(
      name,
    )
  ) {
    process.env[name] = value;
  }
}

if (!atlasImagePath || !atlasJsonPath) {
  console.error(`
Usage:
  bun run bitmap-text <atlas.png> <atlas.json> "TEXT"

Example:
  bun run bitmap-text assets/font.png assets/font.json "HELLO-123"

Environment variables:
  TEXT="HELLO-123"
  GLYPH_SCALE="90%"
  OUTLINES="0,0,0,2,r-255,255,255,1,s"
  OUTPUT_DIR="./tmp"
  OUTPUT_NAME="output.png"
  CLIPBOARD="1"

Options may also be written after the text:
  bun run bitmap-text assets/font.png assets/font.json "HELLO" OUTLINES=0,0,0,2,r
  bun run bitmap-text assets/font.png assets/font.json "HELLO" --clipboard
`);
  process.exit(1);
}

const text = textArgument ?? process.env.TEXT;

if (!text) {
  console.error(
    'Missing text. Pass it as the third argument or set the TEXT environment variable.',
  );
  process.exit(1);
}

/**
 * Atlas JSON format:
 *
 * {
 *   "spaceWidth": 24,
 *   "gapWidth": 5,
 *   "lineGap": 8,
 *   "lineHeight": 56,
 *   "glyphs": {
 *     "A": { "x": 0, "y": 0, "w": 40, "h": 56 },
 *     "B": { "x": 40, "y": 0, "w": 42, "h": 56, "advance": 44 }
 *   }
 * }
 */

function parseGlyphScale(value) {
  if (!value) {
    return 1;
  }

  const trimmed = value.trim();
  let scale;

  if (trimmed.endsWith('%')) {
    scale = Number.parseFloat(trimmed.slice(0, -1)) / 100;
  } else {
    const parsed = Number.parseFloat(trimmed);

    // "90" means 90%, while "0.9" means a scale factor of 0.9.
    scale = parsed > 10 ? parsed / 100 : parsed;
  }

  if (!Number.isFinite(scale) || scale <= 0) {
    throw new Error(
      `Invalid GLYPH_SCALE "${value}". Examples: "90%", "90", or "0.9".`,
    );
  }

  return scale;
}

function parseOutlines(value) {
  if (!value?.trim()) {
    return [];
  }

  return value.split('-').map((outlineText, index) => {
    const parts = outlineText.split(',').map((part) => part.trim());

    if (parts.length !== 5) {
      throw new Error(
        `Invalid outline #${index + 1}: "${outlineText}". ` +
          'Expected red,green,blue,width,style.',
      );
    }

    const [redText, greenText, blueText, widthText, styleText] = parts;

    const red = Number.parseInt(redText, 10);
    const green = Number.parseInt(greenText, 10);
    const blue = Number.parseInt(blueText, 10);
    const width = Number.parseInt(widthText, 10);
    const style = styleText.toLowerCase();

    for (const [name, channelValue] of [
      ['red', red],
      ['green', green],
      ['blue', blue],
    ]) {
      if (
        !Number.isInteger(channelValue) ||
        channelValue < 0 ||
        channelValue > 255
      ) {
        throw new Error(
          `Invalid ${name} value in outline #${index + 1}: ${channelValue}`,
        );
      }
    }

    if (!Number.isInteger(width) || width < 0) {
      throw new Error(
        `Invalid width in outline #${index + 1}: ${widthText}`,
      );
    }

    if (style !== 'r' && style !== 's') {
      throw new Error(
        `Invalid style in outline #${index + 1}: "${styleText}". ` +
          'Use "r" for rounded or "s" for square.',
      );
    }

    return {
      red,
      green,
      blue,
      width,
      style,
    };
  });
}

function validateAtlas(atlas) {
  if (!atlas || typeof atlas !== 'object') {
    throw new Error('Atlas JSON must contain an object.');
  }

  if (!atlas.glyphs || typeof atlas.glyphs !== 'object') {
    throw new Error('Atlas JSON must contain a "glyphs" object.');
  }

  for (const [character, glyph] of Object.entries(atlas.glyphs)) {
    if (!glyph || typeof glyph !== 'object') {
      throw new Error(`Glyph "${character}" must be an object.`);
    }

    for (const field of ['x', 'y', 'w', 'h']) {
      if (!Number.isFinite(glyph[field])) {
        throw new Error(
          `Glyph "${character}" is missing numeric field "${field}".`,
        );
      }
    }

    if (glyph.w <= 0 || glyph.h <= 0) {
      throw new Error(`Glyph "${character}" must have positive w and h.`);
    }

    if (
      glyph.advance !== undefined &&
      (!Number.isFinite(glyph.advance) || glyph.advance <= 0)
    ) {
      throw new Error(
        `Glyph "${character}" has an invalid "advance" value.`,
      );
    }
  }
}

function assertSupportedText(input, glyphs) {
  const allowedPattern = /^[A-Z0-9 \-\r\n]+$/;

  if (!allowedPattern.test(input)) {
    const unsupported = [
      ...new Set(
        [...input].filter(
          (character) => !/[A-Z0-9 \-\r\n]/.test(character),
        ),
      ),
    ];

    throw new Error(
      `Unsupported character(s): ${unsupported
        .map((character) => JSON.stringify(character))
        .join(', ')}. Only A-Z, 0-9, "-", spaces, and newlines are allowed.`,
    );
  }

  const missing = [
    ...new Set(
      [...input].filter(
        (character) =>
          character !== ' ' &&
          character !== '\n' &&
          character !== '\r' &&
          !glyphs[character],
      ),
    ),
  ];

  if (missing.length > 0) {
    throw new Error(
      `The atlas JSON does not define: ${missing
        .map((character) => JSON.stringify(character))
        .join(', ')}`,
    );
  }
}

function createKernelOffsets(radius, style) {
  const offsets = [];

  for (let y = -radius; y <= radius; y += 1) {
    for (let x = -radius; x <= radius; x += 1) {
      if (style === 'r' && x * x + y * y > radius * radius) {
        continue;
      }

      offsets.push({ x, y });
    }
  }

  return offsets;
}

/**
 * Expands an alpha mask.
 *
 * "r" uses a circular/rounded kernel.
 * "s" uses a square kernel.
 */
function dilateAlpha(sourceAlpha, width, height, radius, style) {
  if (radius <= 0) {
    return Buffer.from(sourceAlpha);
  }

  const result = Buffer.alloc(width * height);
  const offsets = createKernelOffsets(radius, style);

  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      let maximumAlpha = 0;

      for (const offset of offsets) {
        const sourceX = x + offset.x;
        const sourceY = y + offset.y;

        if (
          sourceX < 0 ||
          sourceX >= width ||
          sourceY < 0 ||
          sourceY >= height
        ) {
          continue;
        }

        const alpha = sourceAlpha[sourceY * width + sourceX];

        if (alpha > maximumAlpha) {
          maximumAlpha = alpha;

          if (maximumAlpha === 255) {
            break;
          }
        }
      }

      result[y * width + x] = maximumAlpha;
    }
  }

  return result;
}

function makeColoredMask(alpha, width, height, color) {
  const rgba = Buffer.alloc(width * height * 4);

  for (let index = 0; index < alpha.length; index += 1) {
    const rgbaIndex = index * 4;

    rgba[rgbaIndex] = color.red;
    rgba[rgbaIndex + 1] = color.green;
    rgba[rgbaIndex + 2] = color.blue;
    rgba[rgbaIndex + 3] = alpha[index];
  }

  return rgba;
}

async function writeResult(buffer, outputPath) {
  const clipboardOnly = /^(1|true|yes)$/i.test(
    process.env.CLIPBOARD?.trim() || process.env.CLIPBOARD_ONLY?.trim() || '',
  );

  await fs.writeFile(outputPath, buffer);

  if (!clipboardOnly) {
    return;
  }

  if (process.platform !== 'darwin') {
    throw new Error('CLIPBOARD=1 currently requires macOS.');
  }

  const appleScriptPath = path
    .resolve(outputPath)
    .replaceAll('\\', '\\\\')
    .replaceAll('"', '\\"');

  try {
    await execFileAsync('osascript', [
      '-e',
      `set the clipboard to (read (POSIX file "${appleScriptPath}") as «class PNGf»)`,
    ]);
  } finally {
    await fs.rm(outputPath, { force: true });
  }
}

function subtractAlpha(outer, inner) {
  const result = Buffer.alloc(outer.length);

  for (let index = 0; index < result.length; index += 1) {
    result[index] = Math.max(0, outer[index] - inner[index]);
  }

  return result;
}

async function main() {
  const glyphScale = parseGlyphScale(process.env.GLYPH_SCALE);
  const outlines = parseOutlines(process.env.OUTLINES);

  const outputDirectory = process.env.OUTPUT_DIR?.trim() || 'tmp';
  const outputName = process.env.OUTPUT_NAME?.trim() || 'output.png';
  const outputPath = path.join(outputDirectory, outputName);

  const atlasJsonContent = await fs.readFile(atlasJsonPath, 'utf8');
  const atlas = JSON.parse(atlasJsonContent);

  validateAtlas(atlas);
  assertSupportedText(text, atlas.glyphs);

  const gapWidth = Number.isFinite(atlas.gapWidth) ? atlas.gapWidth : 0;
  const spaceWidth = Number.isFinite(atlas.spaceWidth)
    ? atlas.spaceWidth
    : 20;
  const lineGap = Number.isFinite(atlas.lineGap) ? atlas.lineGap : 0;

  const glyphEntries = Object.values(atlas.glyphs);

  // Compose at native atlas size first. The completed text layer is then
  // nearest-neighbour scaled as one image, before fixed-pixel outlines are
  // applied. This preserves hard pixel edges and scales all layout metrics.
  const largestGlyphHeight = Math.max(
    ...glyphEntries.map((glyph) => Math.max(1, Math.round(glyph.h))),
  );
  const configuredLineHeight = Number.isFinite(atlas.lineHeight)
    ? Math.max(1, Math.round(atlas.lineHeight))
    : largestGlyphHeight;
  const lineHeight = Math.max(largestGlyphHeight, configuredLineHeight);

  const normalizedText = text.replace(/\r\n/g, '\n').replace(/\r/g, '\n');
  const lines = normalizedText.split('\n');

  const placements = [];
  const lineWidths = [];

  for (let lineIndex = 0; lineIndex < lines.length; lineIndex += 1) {
    const line = lines[lineIndex];
    let cursorX = 0;

    for (
      let characterIndex = 0;
      characterIndex < line.length;
      characterIndex += 1
    ) {
      const character = line[characterIndex];

      if (character === ' ') {
        cursorX += spaceWidth;

        if (characterIndex < line.length - 1) {
          cursorX += gapWidth;
        }

        continue;
      }

      const glyph = atlas.glyphs[character];

      const renderedWidth = Math.max(1, Math.round(glyph.w));
      const renderedHeight = Math.max(1, Math.round(glyph.h));

      placements.push({
        glyph,
        x: cursorX,
        y: lineIndex * (lineHeight + lineGap),
        width: renderedWidth,
        height: renderedHeight,
      });

      const unscaledAdvance = Number.isFinite(glyph.advance)
        ? glyph.advance
        : glyph.w;

      const renderedAdvance = Math.max(1, Math.round(unscaledAdvance));

      cursorX += renderedAdvance;

      if (characterIndex < line.length - 1) {
        cursorX += gapWidth;
      }
    }

    lineWidths.push(cursorX);
  }

  // Advances determine the next glyph position, but a glyph rectangle can
  // extend beyond its advance. Use actual rendered extents for the canvas so
  // outlines and wide glyphs are not clipped or shifted into the next line.
  const glyphRight = placements.reduce(
    (right, placement) => Math.max(right, placement.x + placement.width),
    0,
  );
  const glyphBottom = placements.reduce(
    (bottom, placement) => Math.max(bottom, placement.y + placement.height),
    0,
  );

  const contentWidth = Math.max(1, ...lineWidths, glyphRight);

  const contentHeight = Math.max(
    1,
    lines.length * lineHeight +
      Math.max(0, lines.length - 1) * lineGap,
    glyphBottom,
  );

  const outerOutlineRadius = outlines.reduce(
    (total, outline) => total + outline.width,
    0,
  );

  const scaledContentWidth = Math.max(1, Math.round(contentWidth * glyphScale));
  const scaledContentHeight = Math.max(1, Math.round(contentHeight * glyphScale));
  const canvasWidth = scaledContentWidth + outerOutlineRadius * 2;
  const canvasHeight = scaledContentHeight + outerOutlineRadius * 2;

  const composites = [];

  for (const placement of placements) {
    const extractedGlyph = await sharp(atlasImagePath)
      .extract({
        left: Math.round(placement.glyph.x),
        top: Math.round(placement.glyph.y),
        width: Math.round(placement.glyph.w),
        height: Math.round(placement.glyph.h),
      })
      .png()
      .toBuffer();

    composites.push({
      input: extractedGlyph,
      left: Math.round(placement.x),
      top: Math.round(placement.y),
    });
  }

  const nativeGlyphLayer = await sharp({
    create: {
      width: contentWidth,
      height: contentHeight,
      channels: 4,
      background: '#00000000',
    },
  })
    .composite(composites)
    .png()
    .toBuffer();

  const scaledGlyphLayer = await sharp(nativeGlyphLayer)
    .resize(scaledContentWidth, scaledContentHeight, {
      fit: 'fill',
      kernel: sharp.kernel.nearest,
    })
    .png()
    .toBuffer();

  const glyphLayer = await sharp({
    create: {
      width: canvasWidth,
      height: canvasHeight,
      channels: 4,
      background: '#00000000',
    },
  })
    .composite([
      {
        input: scaledGlyphLayer,
        left: outerOutlineRadius,
        top: outerOutlineRadius,
      },
    ])
    .png()
    .toBuffer();

  await fs.mkdir(outputDirectory, { recursive: true });

  if (outlines.length === 0) {
    const result = await sharp(glyphLayer).png().toBuffer();
    await writeResult(result, outputPath);

    console.log(
      /^(1|true|yes)$/i.test(process.env.CLIPBOARD?.trim() || process.env.CLIPBOARD_ONLY?.trim() || '')
        ? 'Copied PNG to the clipboard.'
        : `Wrote ${outputPath}`,
    );
    return;
  }

  const { data: glyphPixels, info: glyphInfo } = await sharp(glyphLayer)
    .ensureAlpha()
    .raw()
    .toBuffer({ resolveWithObject: true });

  const glyphAlpha = Buffer.alloc(canvasWidth * canvasHeight);

  for (let index = 0; index < glyphAlpha.length; index += 1) {
    glyphAlpha[index] = glyphPixels[index * glyphInfo.channels + 3];
  }

  /*
   * OUTLINES are declared nearest-to-farthest:
   *
   * 0,0,0,2,r-255,255,255,1,s
   *
   * 1. Black rounded outline extending 2 px.
   * 2. White square outline extending another 1 px.
   */
  const outlineLayers = [];
  // Build outlines as a chain. Each layer expands the mask produced by the
  // previous layer, so mixed styles compose naturally:
  // original -> rounded expansion -> color -> square expansion -> color.
  let currentAlpha = glyphAlpha;

  for (const outline of outlines) {
    const expandedAlpha = dilateAlpha(
      currentAlpha,
      canvasWidth,
      canvasHeight,
      outline.width,
      outline.style,
    );

    // Keep each layer as a ring. Compositing complete expanded masks causes
    // adjacent outline colors to overlap and makes the style of an outer
    // outline affect the inner one.
    const ringAlpha = subtractAlpha(expandedAlpha, currentAlpha);
    currentAlpha = expandedAlpha;

    const coloredPixels = makeColoredMask(
      ringAlpha,
      canvasWidth,
      canvasHeight,
      outline,
    );

    const layerBuffer = await sharp(coloredPixels, {
      raw: {
        width: canvasWidth,
        height: canvasHeight,
        channels: 4,
      },
    })
      .png()
      .toBuffer();

    outlineLayers.push(layerBuffer);
  }

  const finalComposites = [];

  // Draw the farthest outline first.
  for (let index = outlineLayers.length - 1; index >= 0; index -= 1) {
    finalComposites.push({
      input: outlineLayers[index],
      left: 0,
      top: 0,
    });
  }

  // Draw the original glyphs above every outline.
  finalComposites.push({
    input: glyphLayer,
    left: 0,
    top: 0,
  });

  const result = await sharp({
      create: {
      width: canvasWidth,
      height: canvasHeight,
      channels: 4,
      background: '#00000000',
    },
  })
    .composite(finalComposites)
    .png()
    .toBuffer();

  await writeResult(result, outputPath);

  console.log(
    /^(1|true|yes)$/i.test(process.env.CLIPBOARD?.trim() || process.env.CLIPBOARD_ONLY?.trim() || '')
      ? 'Copied PNG to the clipboard.'
      : `Wrote ${outputPath}`,
  );
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
});
