// Markdown parsing and rendering utilities
import type { ReactNode } from 'react';
import type { MarkdownBlock, MarkdownHeadingLevel } from '../types';

export function normalizeAssistantMarkdownProse(content: string) {
  return content
    .replace(/\r\n/g, '\n')
    .replace(/\(([^()\n]+?)\s+[-*+]\s+([^()\n]+?)\)/g, '($1 and $2)')
    .replace(/(^|\n)\s{0,3}(#{1,6})([^\s#])/g, '$1$2 $3')
    .replace(/([:.!?])\s*(#{1,6}\s+[A-Za-z0-9])/g, '$1\n$2')
    .replace(/([:.!?])\s*(\d+\.\s+)/g, '$1\n$2')
    .replace(/([:.!?])\s*([*+-]\s+)/g, '$1\n$2')
    .replace(/([A-Za-z0-9)])\s+(\d+\.\s+)/g, '$1\n$2')
    .replace(/([^\n])(\d+\.\s+\*\*)/g, '$1\n$2')
    .replace(/\n{3,}/g, '\n\n');
}

export function normalizeAssistantMarkdown(content: string) {
  return content
    .replace(/\r\n/g, '\n')
    .split(/(```[\s\S]*?```)/g)
    .map((segment) =>
      segment.startsWith('```') ? segment : normalizeAssistantMarkdownProse(segment),
    )
    .join('');
}

export function parseMarkdownBlocks(content: string): MarkdownBlock[] {
  const normalized = normalizeAssistantMarkdown(content);
  const lines = normalized.split('\n');
  const blocks: MarkdownBlock[] = [];
  let paragraph: string[] = [];
  let codeLines: string[] = [];
  let codeLanguage = '';
  let inCode = false;

  function flushParagraph() {
    if (paragraph.length === 0) return;
    blocks.push({ type: 'paragraph', text: paragraph.join(' ').trim() });
    paragraph = [];
  }

  function pushList(type: 'ordered' | 'unordered', firstItem: string, startIndex: number) {
    const items = [firstItem.trim()];
    let index = startIndex + 1;
    while (index < lines.length) {
      const line = lines[index].trim();
      const orderedMatch = line.match(/^\d+\.\s+(.*)$/);
      const unorderedMatch = line.match(/^[-*+]\s+(.*)$/);
      if (type === 'ordered' && orderedMatch) { items.push(orderedMatch[1].trim()); index += 1; continue; }
      if (type === 'unordered' && unorderedMatch) { items.push(unorderedMatch[1].trim()); index += 1; continue; }
      break;
    }
    blocks.push({ type, items });
    return index - 1;
  }

  for (let index = 0; index < lines.length; index += 1) {
    const rawLine = lines[index];
    const line = rawLine.trim();
    const fenceMatch = line.match(/^```([A-Za-z0-9_+.#-]*)/);
    if (fenceMatch) {
      if (inCode) {
        blocks.push({ type: 'code', text: codeLines.join('\n'), language: codeLanguage });
        codeLines = []; codeLanguage = ''; inCode = false;
      } else {
        flushParagraph(); inCode = true;
        codeLanguage = fenceMatch[1]?.trim().toLowerCase() || 'text';
      }
      continue;
    }
    if (inCode) { codeLines.push(rawLine); continue; }
    if (!line) { flushParagraph(); continue; }
    if (/^(---|\*\*\*|___)\s*$/.test(line)) {
      flushParagraph();
      blocks.push({ type: 'hr' });
      continue;
    }
    const headingMatch = line.match(/^(#{1,6})\s+(.+)$/);
    if (headingMatch) {
      flushParagraph();
      blocks.push({ type: 'heading', level: headingMatch[1].length as MarkdownHeadingLevel, text: headingMatch[2].trim() });
      continue;
    }
    const orderedMatch = line.match(/^\d+\.\s+(.*)$/);
    if (orderedMatch) { flushParagraph(); index = pushList('ordered', orderedMatch[1], index); continue; }
    const unorderedMatch = line.match(/^[-*+]\s+(.*)$/);
    if (unorderedMatch) { flushParagraph(); index = pushList('unordered', unorderedMatch[1], index); continue; }
    paragraph.push(line);
  }

  if (inCode && codeLines.length > 0) {
    blocks.push({ type: 'code', text: codeLines.join('\n'), language: codeLanguage });
  }
  flushParagraph();
  return blocks.length > 0 ? blocks : [{ type: 'paragraph', text: content }];
}

export function renderInlineMarkdown(text: string, vaultPath?: string, noteDir?: string) {
  const parts: ReactNode[] = [];
  // Match Obsidian image embeds ![[...]], inline code, bold (**/__), italic (*/_)
  // Use negated character classes instead of .+? for reliable matching
  const pattern = /(!\[\[([^\]]+)\]\]|`[^`]+`|\*\*([^*]+)\*\*|__([^_]+)__|\*([^*]+)\*|_([^_]+)_)/g;
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = pattern.exec(text)) !== null) {
    if (match.index > lastIndex) parts.push(text.slice(lastIndex, match.index));
    const full = match[0];
    if (full.startsWith('!')) {
      // Obsidian image embed: ![[filename|options]]
      const inner = match[2];
      const pipeIdx = inner.lastIndexOf('|');
      let filename = pipeIdx >= 0 ? inner.slice(0, pipeIdx) : inner;
      const options = pipeIdx >= 0 ? inner.slice(pipeIdx + 1) : '';
      let width: number | undefined;
      let height: number | undefined;
      if (options) {
        const dims = options.split('x');
        if (dims[0]) width = Math.round(parseInt(dims[0], 10) * 0.8) || undefined;
        if (dims[1]) height = Math.round(parseInt(dims[1], 10) * 0.8) || undefined;
      }
      // Pass filename + optional note directory — backend searches vault root, note dir, images/, etc.
      let src = '';
      if (vaultPath) {
        src = `/api/mcp/obsidian/file?vault_path=${encodeURIComponent(vaultPath)}&path=${encodeURIComponent(filename)}`;
        if (noteDir) src += `&note_dir=${encodeURIComponent(noteDir)}`;
      }
      parts.push(
        <img
          key={`${match.index}-img`}
          src={src}
          alt={filename}
          width={width}
          height={height}
          className="rounded max-w-full h-auto my-2 mx-auto block border border-zinc-700/50"
          loading="lazy"
        />
      );
    } else if (full.startsWith('`')) {
      parts.push(<code className="rounded bg-black/15 px-1.5 py-0.5 font-mono text-[0.92em] text-emerald-500" key={`${match.index}-code`}>{full.slice(1, -1)}</code>);
    } else if (full.startsWith('**') || full.startsWith('__')) {
      parts.push(<strong className="font-semibold" key={`${match.index}-strong`}>{match[3] || match[4]}</strong>);
    } else {
      parts.push(<em className="italic" key={`${match.index}-em`}>{match[5] || match[6]}</em>);
    }
    lastIndex = match.index + full.length;
  }
  if (lastIndex < text.length) parts.push(text.slice(lastIndex));
  return parts;
}

const CODE_KEYWORDS = new Set([
  'as', 'async', 'await', 'break', 'case', 'catch', 'class', 'const', 'continue',
  'def', 'else', 'enum', 'export', 'extends', 'false', 'fn', 'for', 'from', 'function',
  'if', 'impl', 'import', 'in', 'interface', 'let', 'match', 'mod', 'mut', 'new',
  'none', 'null', 'ok', 'pub', 'return', 'self', 'some', 'struct', 'switch', 'this',
  'throw', 'true', 'try', 'type', 'use', 'var', 'while', 'with',
]);

const CODE_TYPES = new Set([
  'bool', 'dict', 'error', 'i32', 'i64', 'number', 'object', 'result', 'str', 'string', 'u32', 'u64', 'vec', 'void',
]);

const CODE_TOKEN_PATTERN = /(\/\/.*|#.*|\/\*.*?\*\/|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|`(?:\\.|[^`\\])*`|\b\d+(?:\.\d+)?\b|\b[A-Za-z_][A-Za-z0-9_]*\b|[{}()[\].,;:+\-*/%=<>!&|?]+)/g;

export function normalizedCodeLanguage(language: string) {
  const label = language.trim().toLowerCase();
  if (!label || label === 'text' || label === 'txt') return 'code';
  if (label === 'ts') return 'typescript';
  if (label === 'js') return 'javascript';
  if (label === 'py') return 'python';
  if (label === 'rs') return 'rust';
  return label;
}

function codeTokenClass(token: string) {
  const lowerToken = token.toLowerCase();
  if (token.startsWith('//') || token.startsWith('#') || token.startsWith('/*')) return 'text-emerald-400/80 italic';
  if (token.startsWith('"') || token.startsWith("'") || token.startsWith('`')) return 'text-amber-300';
  if (/^\d/.test(token)) return 'text-cyan-300';
  if (CODE_KEYWORDS.has(lowerToken)) return 'text-sky-300';
  if (CODE_TYPES.has(lowerToken) || /^[A-Z][A-Za-z0-9_]*$/.test(token)) return 'text-violet-300';
  if (/^[{}()[\].,;:+\-*/%=<>!&|?]+$/.test(token)) return 'text-zinc-400';
  return 'text-zinc-100';
}

export function renderHighlightedCodeLine(line: string, lineIndex: number) {
  const parts: ReactNode[] = [];
  let lastIndex = 0;
  let match: RegExpExecArray | null;
  CODE_TOKEN_PATTERN.lastIndex = 0;
  while ((match = CODE_TOKEN_PATTERN.exec(line)) !== null) {
    if (match.index > lastIndex) parts.push(line.slice(lastIndex, match.index));
    const token = match[0];
    parts.push(<span className={codeTokenClass(token)} key={`${lineIndex}-${match.index}`}>{token}</span>);
    lastIndex = match.index + token.length;
  }
  if (lastIndex < line.length) parts.push(line.slice(lastIndex));
  return parts.length > 0 ? parts : '\u00A0';
}
