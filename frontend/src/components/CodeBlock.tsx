// Code block with syntax highlighting and copy button
import { useState } from 'react';
import { Check, Copy } from 'lucide-react';
import { normalizedCodeLanguage, renderHighlightedCodeLine, copyTextToClipboard } from '../lib';

export function CodeBlock({ language, text }: { language: string; text: string }) {
  const [copied, setCopied] = useState(false);
  const languageLabel = normalizedCodeLanguage(language);
  const lines = text.split('\n');

  async function copyCode() {
    await copyTextToClipboard(text);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1400);
  }

  return (
    <div className="group max-w-[42rem] overflow-hidden rounded-lg border border-zinc-800 bg-zinc-950 shadow-md shadow-white/5">
      <div className="flex items-center justify-between gap-3 px-3 pt-2.5">
        <span className="truncate font-mono text-[11px] uppercase tracking-wide text-zinc-500">
          {languageLabel}
        </span>
        <button
          className="inline-flex items-center gap-1.5 rounded-md border border-zinc-700 bg-zinc-950/80 px-1.5 py-0.5 text-[11px] font-medium text-zinc-300 transition hover:border-emerald-500/70 hover:bg-emerald-500/10 hover:text-emerald-200"
          onClick={copyCode}
          type="button"
        >
          {copied ? <Check size={13} /> : <Copy size={13} />}
          {copied ? 'Copied' : 'Copy'}
        </button>
      </div>
      <pre className="overflow-x-auto px-3 pb-3 pt-2 text-left font-mono text-[12px] leading-5">
        <code>
          {lines.map((line, lineIndex) => (
            <span className="block whitespace-pre" key={`${lineIndex}-${line}`}>
              {renderHighlightedCodeLine(line, lineIndex)}
            </span>
          ))}
        </code>
      </pre>
    </div>
  );
}
