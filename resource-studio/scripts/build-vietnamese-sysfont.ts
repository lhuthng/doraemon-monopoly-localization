import { readFileSync, writeFileSync } from 'node:fs';
import { parseSysFont, rebuildSysFont } from '../src/lib/formats';
import { extendSysFont } from '../src/lib/vietnamese-font';

const input = process.argv[2];
const output = process.argv[3];
if (!input || !output) throw new Error('Usage: bun build-vietnamese-sysfont.ts INPUT.DAT OUTPUT.DAT');
const font = extendSysFont(parseSysFont(new Uint8Array(readFileSync(input))));
const rebuilt = rebuildSysFont(font);
writeFileSync(output, rebuilt);
console.log(`Wrote ${output}: ${font.count} glyphs, ${rebuilt.length} bytes.`);
