export const GADGETS_LAYOUT = {
  id: 'gadgets',
  label: 'Gadgets',
  maxWidth: 87
} as const;

export const DIALOG_LAYOUT = {
  id: 'dialog',
  label: 'Dialog',
  maxWidth: 309,
  splitWords: false
} as const;

export function sysfontWidth(text: string, widths: readonly number[]) {
  return [...text].reduce((width, character) => {
    const code = character.charCodeAt(0);
    if (code >= 0 && code < widths.length) return width + widths[code];
    const base = character === 'Đ' ? 'D' : character === 'đ' ? 'd' : character.normalize('NFD')[0];
    const baseCode = base.charCodeAt(0);
    return width + (baseCode < widths.length ? widths[baseCode] : 0);
  }, 0);
}

export function reflowGameText(
  text: string,
  maxWidth: number,
  widths: readonly number[],
  splitWords = false
) {
  const width = Math.max(1, Math.floor(maxWidth));
  const lines: string[] = [];
  const oversizedWords: string[] = [];

  for (const sourceLine of text.replaceAll('\r\n', '\n').replaceAll('\r', '\n').split('\n')) {
    if (splitWords) {
      let line = '';
      for (const character of sourceLine) {
        const candidate = line + character;
        if (line && sysfontWidth(candidate, widths) > width) {
          lines.push(line.trimEnd());
          line = character === ' ' ? '' : character;
        } else {
          line = candidate;
        }
      }
      if (line.trim()) lines.push(line.trimEnd());
      continue;
    }
    const words = sourceLine.trim().split(/\s+/).filter(Boolean);
    if (!words.length) {
      lines.push('');
      continue;
    }

    let line = '';
    for (const word of words) {
      // The game renders Latin text in uppercase. Measure that form, but do not
      // mutate the translator's text: capitalization remains their decision.
      if (sysfontWidth(word, widths) > width) oversizedWords.push(word);
      const candidate = line ? `${line} ${word}` : word;
      if (line && sysfontWidth(candidate, widths) > width) {
        lines.push(line);
        line = word;
      } else {
        line = candidate;
      }
    }
    if (line) lines.push(line);
  }

  return { text: lines.join('\n'), oversizedWords };
}
