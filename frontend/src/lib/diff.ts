// Unified diff extraction and application utilities

export function extractUnifiedDiff(content: string) {
  const fencedMatch = content.match(/```(?:diff|patch)?\s*\n([\s\S]*?^```)/m);
  const candidate = fencedMatch
    ? fencedMatch[1].replace(/\n```$/, '')
    : content.slice(content.indexOf('diff --git'));
  if (!candidate || !candidate.includes('--- ') || !candidate.includes('+++ ')) return '';
  return candidate.trim();
}

export function parsePatchTarget(diff: string) {
  const plusLine = diff.split('\n').find((line) => line.startsWith('+++ '));
  if (!plusLine) return '';
  return plusLine.replace(/^\+\+\+\s+/, '').replace(/^[ab]\//, '').replace(/^\/dev\/null/, '').trim();
}

export function applySimpleUnifiedDiff(original: string, diff: string) {
  const lines = original.split('\n');
  const output: string[] = [];
  let sourceIndex = 0;
  const diffLines = diff.split(/\r?\n/);
  let index = 0;

  while (index < diffLines.length) {
    const line = diffLines[index];
    const hunkMatch = line.match(/^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/);
    if (!hunkMatch) { index += 1; continue; }
    const hunkStart = Math.max(0, Number(hunkMatch[1]) - 1);
    while (sourceIndex < hunkStart) { output.push(lines[sourceIndex] ?? ''); sourceIndex += 1; }
    index += 1;
    while (index < diffLines.length && !diffLines[index].startsWith('@@ ')) {
      const hunkLine = diffLines[index];
      const marker = hunkLine[0];
      const value = hunkLine.slice(1);
      if (marker === ' ') {
        sourceIndex += 1;
        if (sourceIndex <= lines.length) output.push(value);
      } else if (marker === '-') {
        sourceIndex += 1;
      } else if (marker === '+') { output.push(value); }
      index += 1;
    }
  }
  while (sourceIndex < lines.length) { output.push(lines[sourceIndex] ?? ''); sourceIndex += 1; }
  return output.join('\n');
}
