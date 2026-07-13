import { pipeline, env } from '@huggingface/transformers';

type TargetLanguage = 'en' | 'vi';
type ModelId = 'nllb' | 'm2m100';

const MODELS: Record<ModelId, {
  label: string;
  model: string;
  dtype: 'q8';
  source: string;
  targets: Record<TargetLanguage, string>;
}> = {
  nllb: {
    label: 'NLLB 200 distilled 600M',
    model: 'Xenova/nllb-200-distilled-600M',
    dtype: 'q8',
    source: 'zho_Hant',
    targets: { en: 'eng_Latn', vi: 'vie_Latn' }
  },
  m2m100: {
    label: 'M2M100 418M',
    model: 'Xenova/m2m100_418M',
    dtype: 'q8',
    source: 'zh',
    targets: { en: 'en', vi: 'vi' }
  }
};

const translatorPromises = new Map<ModelId, ReturnType<typeof pipeline>>();

function json(data: unknown, status = 200) {
  return new Response(JSON.stringify(data), {
    status,
    headers: {
      'access-control-allow-origin': '*',
      'access-control-allow-methods': 'GET,POST,OPTIONS',
      'access-control-allow-headers': 'content-type',
      'content-type': 'application/json; charset=utf-8'
    }
  });
}

function cleanAsciiPunctuation(text: string) {
  return text
    .replace(/[“”]/g, '"').replace(/[‘’]/g, "'").replace(/[–—]/g, '-').replaceAll('…', '...')
    .replace(/Low Bunny Rich|Ding Dong Monopoly|Dingdang Monopoly/gi, 'Doraemon Monopoly')
    .replace(/Wickrona/gi, 'Soft-World')
    .replace(/\s+/g, ' ')
    .trim();
}

function vietnameseAscii(text: string) {
  return cleanAsciiPunctuation(text)
    .replaceAll('đ', 'd').replaceAll('Đ', 'D')
    .normalize('NFD').replace(/[\u0300-\u036f]/g, '')
    .replace(/cua tuy y|cua bat ky|cua o bat cu dau/gi, 'Canh cua than ky')
    .replace(/chuon chuon tre|chuon chuon bang tre/gi, 'Chong chong tre')
    .replace(/banh bao dau do|banh dau do/gi, 'banh ran')
    .replace(/[^\x00-\x7f]/g, '')
    .trim();
}

function cleanupTranslation(text: string, target: TargetLanguage) {
  if (target === 'en') return cleanAsciiPunctuation(text);
  if (target === 'vi') return vietnameseAscii(text);
  throw new Error(`No cleanup code is available for "${target}".`);
}

async function translator(modelId: ModelId) {
  let translatorPromise = translatorPromises.get(modelId);
  if (!translatorPromise) {
    const model = MODELS[modelId];
    env.allowLocalModels = false;
    env.useBrowserCache = false;
    env.useFSCache = true;
    translatorPromise = pipeline('translation', model.model, {
      dtype: model.dtype
    });
    translatorPromises.set(modelId, translatorPromise);
  }
  return translatorPromise;
}

async function translateText(text: string, target: TargetLanguage, modelId: ModelId) {
  const model = MODELS[modelId];
  const translate = await translator(modelId);
  const result = await translate(text, {
    src_lang: model.source,
    tgt_lang: model.targets[target],
    max_new_tokens: 192
  });
  const output = Array.isArray(result) ? result[0] : result;
  if (!output || typeof output.translation_text !== 'string') {
    throw new Error('The model returned no translation text.');
  }
  return cleanupTranslation(output.translation_text, target);
}

Bun.serve({
  hostname: '127.0.0.1',
  port: 5184,
  async fetch(request) {
    const url = new URL(request.url);
    if (request.method === 'OPTIONS') return json({});
    if (url.pathname === '/api/health') {
      return json({
        ok: true,
        models: Object.entries(MODELS).map(([id, model]) => ({ id, label: model.label, model: model.model }))
      });
    }
    if (url.pathname !== '/api/translate' || request.method !== 'POST') return json({ error: 'Not found.' }, 404);

    try {
      const body = await request.json();
      const target = body?.target;
      const model = body?.model || 'nllb';
      const texts = body?.texts;
      if (target !== 'en' && target !== 'vi') throw new Error('target must be "en" or "vi".');
      if (model !== 'nllb' && model !== 'm2m100') throw new Error('model must be "nllb" or "m2m100".');
      if (!Array.isArray(texts) || !texts.every((text) => typeof text === 'string')) {
        throw new Error('texts must be an array of strings.');
      }
      const translations: string[] = [];
      for (const text of texts) {
        translations.push(text.trim() ? await translateText(text, target, model) : '');
      }
      return json({ translations });
    } catch (error) {
      return json({ error: error instanceof Error ? error.message : String(error) }, 500);
    }
  }
});

console.log('Doraemon translation server listening on http://127.0.0.1:5184');
