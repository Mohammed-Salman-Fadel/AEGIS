// Renders assistant markdown content (headings, lists, code, paragraphs)
import type { MarkdownHeadingLevel } from '../types';
import { parseMarkdownBlocks, renderInlineMarkdown } from '../lib';
import { CodeBlock } from './CodeBlock';
import { ThinkingIndicator } from './ThinkingIndicator';

function MarkdownHeading({ level, text }: { level: MarkdownHeadingLevel; text: string }) {
  const className =
    level === 1
      ? 'aegis-display mt-1 text-[1.08rem] font-semibold leading-7 tracking-[-0.02em] first:mt-0'
      : level === 2
        ? 'aegis-display mt-4 text-[1.02rem] font-semibold leading-7 tracking-[-0.018em] first:mt-0'
        : 'aegis-display mt-3 text-[0.96rem] font-semibold leading-6 tracking-[-0.015em] first:mt-0';

  if (level === 1) return <h3 className={className}>{renderInlineMarkdown(text)}</h3>;
  if (level === 2) return <h4 className={className}>{renderInlineMarkdown(text)}</h4>;
  return <h5 className={className}>{renderInlineMarkdown(text)}</h5>;
}

export function AssistantMarkdown({ content, isDark, vaultPath, noteDir }: { content: string; isDark: boolean; vaultPath?: string; noteDir?: string }) {
  const blocks = parseMarkdownBlocks(content || '...');
  const inline = (text: string) => renderInlineMarkdown(text, vaultPath, noteDir);

  if (!content) {
    return <ThinkingIndicator isDark={isDark} />;
  }

  return (
    <div className="aegis-prose space-y-3.5">
      {blocks.map((block, blockIndex) => {
        if (block.type === 'heading') {
          return <MarkdownHeading key={`heading-${blockIndex}`} level={block.level} text={block.text} />;
        }
        if (block.type === 'ordered') {
          return (
            <ol className="list-decimal space-y-1.5 pl-5" key={`ol-${blockIndex}`}>
              {block.items.map((item, itemIndex) => (
                <li key={`${blockIndex}-${itemIndex}`}>{inline(item)}</li>
              ))}
            </ol>
          );
        }
        if (block.type === 'unordered') {
          return (
            <ul className="list-disc space-y-1.5 pl-5" key={`ul-${blockIndex}`}>
              {block.items.map((item, itemIndex) => (
                <li key={`${blockIndex}-${itemIndex}`}>{inline(item)}</li>
              ))}
            </ul>
          );
        }
        if (block.type === 'code') {
          return <CodeBlock key={`code-${blockIndex}`} language={block.language} text={block.text} />;
        }
        if (block.type === 'hr') {
          return <hr key={`hr-${blockIndex}`} className="border-t border-zinc-700 my-4" />;
        }
        return <p key={`p-${blockIndex}`}>{inline(block.text)}</p>;
      })}
    </div>
  );
}
