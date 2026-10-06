# Voice bank-1 transcript (slots 000–022)

Date: 2026-10-05
Target: `Voice.dat` character/bank-1/slots `000`–`022`, version 1.26 Cantonese
Scope: transcribe the shared menu/action/misc voice lines, one line per slot,
each recorded by all (or some) of the six characters

Confidence vocabulary follows
[`reverse-engineering-journal.md`](reverse-engineering-journal.md):
**Confirmed by format decoding**, **Strong inference**, ear-checked, model-only.

## Final transcript

| Slot | Line | Status |
| --- | --- | --- |
| `00*/001/000` | 嗨。 | model + consistency |
| `00*/001/001` | 大富翁hi。 | model + consistency |
| `00*/001/002` | 嗨开始玩游戏啦。 | model + consistency |
| `00*/001/003` | 繼續進度。 | ear-checked |
| `00*/001/004` | 成绩单。 | model + consistency |
| `00*/001/005` | 嘿开始玩小游戏啦。 | model + consistency |
| `00*/001/006` | 比赛模式。 | model + consistency |
| `00*/001/007` | 练习模式。 | model + consistency |
| `00*/001/008` | 操控设定。 | model + consistency |
| `00*/001/009` | 排行榜。 | model + consistency |
| `00*/001/010` | 拜拜。 | model + consistency |
| `00*/001/011` | 前进。 | model + consistency, matches `000/031` link in code |
| `00*/001/012` | 百宝袋。 | ear-checked; **disagrees with** `000/032` 使用道具 link in code |
| `00*/001/013` | 系统设定。 | model + ear-checked; voice adds 设定 vs `000/033` 系统 |
| `00*/001/014` | 睇睇地图先啦。 | ear-checked; fuller sentence than `000/034` 地图 |
| `00*/001/015` | 资料统计。 | model + consistency, matches `000/035` link in code |
| `00*/001/016` | 好鬼大富翁。 | ear-checked, Doraemon only |
| `00*/001/017` | 技巧。益智。八款小游戏。 | Doraemon only; byte-identical audio to 018/019 |
| `00*/001/018` | 技巧。益智。八款小游戏。 | Doraemon only; byte-identical audio to 017/019 |
| `00*/001/019` | 技巧。益智。八款小游戏。 | Doraemon only; byte-identical audio to 017/018 |
| `00*/001/020` | 大家齊玩啦。 | reconstructed (see below), Doraemon only |
| `00*/001/021` | 叮当大富翁。 | model + consistency (title, Cantonese name) |
| `00*/001/022` | Doraemon 大富翁。 | model + consistency (title, English name) |

## Method

Three independent passes, weakest first, each checking the previous:

1. **Machine transcript (model-only).** Each clip was decoded from
   `voice.dat` with `decodeVoiceRecord`
   (`packages/dubbing-core/src/voice-formats.ts`) to mono 22.05 kHz 16-bit
   WAV, then sent to the public FunAudioLLM/SenseVoice Space with language
   `yue` (Cantonese). No local model, no API key, ~30 s per clip over the
   free queue. Raw drafts (including the model's emotion emoji) are kept out
   of the repo; only the corrected lines above are claimed.
2. **Consistency cross-check.** Shared slots must say the same thing across
   all six characters. Every slot `000`–`015`, `021`–`022` passed: the six
   drafts agreed up to homophone spelling (e.g. 代富翁/大富翁, 系/hi on a
   0.6 s blip). Agreement across six independent recordings is **strong
   inference** the draft is right; disagreement would have flagged a bad
   draft. None diverged in meaning.
3. **Ear check.** Slots where the model wobbled (`003`'s last word,
   `012`/`013`, `014`, `016`, `020`) were decided by listening. The model
   normalizes Cantonese toward Mandarin-style simplified (e.g. 睇下 → 系统
   setting prose), so colloquial lines needed ears.

## Case study: slot 020 (reconstructed)

The model only ever produced 大家 ("everyone…") plus noise: the clip is
group speech, which ASR handles poorly. Reconstruction:

- Measured: 5 syllables, starts with 大家 (`daai6-gaa1`), ends with 啦
  (`laa1`, suggestion particle), so the shape is 大家 + X + X + 啦.
- Context constraint: game menu, must be meaningful and natural in
  Cantonese.
- Asked DeepSeek for a 5-syllable menu-natural phrase fitting
  大家…啦; candidate 齊玩啦 (`cai4-wun2-laa1`, "let's play together")
  fit the shape.
- Ear verification: the middle syllable carries 玩 (`waan2`, "play") -
  confirmed by listening, giving 大家齊玩啦 ("everyone, let's play!").

Status: reconstructed, not verbatim-confirmed. A native speaker re-listen is
the remaining check.

## Inconsistencies found (audio vs code links)

Open question: should slots `011`–`013` match their linked menu text
(`000/031`–`000/033`)? Current evidence:

- Slot `011` says 前进, matching `000/031` 前进 exactly.
- Slot `013` says 系统设定, one word more than `000/033` 系统.
- Slot `012` says 百宝袋 ("treasure bag") but pairs with `000/032`
  使用道具 ("use item").

So the pattern is inconsistent: exact, near, and no match side by side.
Slot `014` is the same story (full sentence 睇睇地圖先 vs one-word
`000/034` 地图), while `015` matches `000/035` exactly.
Undecided whether the code links, the menu labels, or the recordings
drifted during development. Needs a runtime check (which voice the game
plays when highlighting each action-menu item) before claiming either side.

Dubbing rule used for the Vietnamese replacements: slots `013`/`014`/`015`
say the in-game translated menu strings (Hệ Thống / Bản Đồ / Chỉ số from
`000/033`–`000/035` in the Vietnamese `strings.dat`), not literal
translations of the heard audio. Drafts live in `tmp/voice-translation-vi.json`.
- The `legacy/VOICE_DAT_RESEARCH.md` formula (`000/N` → slot `N−8`)
  predicts slot `002` = `000/010` (sage-robot luck line); the recording is a
  "let's start playing" menu prompt. That formula does not hold for these
  slots in 1.26 Cantonese: the Studio's "Menu" label
  (`StringStudio.svelte`, bank 1 slot ≤ 10) is the better description, but it
  is a display heuristic, not a transcript link.

## Coverage notes

- Slots `016`–`020` exist for Doraemon only; other characters carry
  one-byte empty markers there (structural placeholders, not audio).
- Slots `016`, `020`, `021`, `022` are registered under character 0
  (Doraemon) but are **group voices for all characters**, not
  Doraemon-specific lines: `021`/`022` carry one recording per character of
  the shared title call, while `016`/`020` carry only Doraemon's recording
  and the other characters are empty: the single recording serves the
  whole group. The Studio files them under Doraemon either way
  (`voiceOnlyRecords` filters by `path[0]`), which understates their shared
  role.
- Slots `017`/`018`/`019` decode to byte-identical WAVs: one recording
  shipped three times.
- Slots `023`–`027` were out of scope (Doraemon-only per prior research).
