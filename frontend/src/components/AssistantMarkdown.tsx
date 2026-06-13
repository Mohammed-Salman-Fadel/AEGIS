// Renders assistant markdown content (headings, lists, code, paragraphs)
import type { MarkdownHeadingLevel } from '../types';
import { parseMarkdownBlocks, renderInlineMarkdown } from '../lib';
import { CodeBlock } from './CodeBlock';
import { ThinkingIndicator } from './ThinkingIndicator';

function MarkdownHeading({ level, text }: { level: MarkdownHeadingLevel; text: string }) {
  const className =
    level === 1
      ? 'mt-1 text-[1.08rem] font-normal leading-7 tracking-[-0.01em] first:mt-0'
      : level === 2
        ? 'mt-3 text-[1.02rem] font-normal leading-7 tracking-[-0.01em] first:mt-0'
        : 'mt-3 text-[0.96rem] font-normal leading-6 tracking-[-0.005em] first:mt-0';

  if (level === 1) return <h3 className={className}>{renderInlineMarkdown(text)}</h3>;
  if (level === 2) return <h4 className={className}>{renderInlineMarkdown(text)}</h4>;
  return <h5 className={className}>{renderInlineMarkdown(text)}</h5>;
}

export function AssistantMarkdown({ content, isDark }: { content: string; isDark: boolean }) {
  const blocks = parseMarkdownBlocks(content || '...');

  if (!content) {
    return <ThinkingIndicator isDark={isDark} />;
  }

  return (
    <div className="space-y-3">
      {blocks.map((block, blockIndex) => {
        if (block.type === 'heading') {
          return <MarkdownHeading key={`heading-${blockIndex}`} level={block.level} text={block.text} />;
        }
        if (block.type === 'ordered') {
          return (
            <ol className="list-decimal space-y-1 pl-5" key={`ol-${blockIndex}`}>
              {block.items.map((item, itemIndex) => (
                <li key={`${blockIndex}-${itemIndex}`}>{renderInlineMarkdown(item)}</li>
              ))}
            </ol>
          );
        }
        if (block.type === 'unordered') {
          return (
            <ul className="list-disc space-y-1 pl-5" key={`ul-${blockIndex}`}>
              {block.items.map((item, itemIndex) => (
                <li key={`${blockIndex}-${itemIndex}`}>{renderInlineMarkdown(item)}</li>
              ))}
            </ul>
          );
        }
        if (block.type === 'code') {
          return <CodeBlock key={`code-${blockIndex}`} language={block.language} text={block.text} />;
        }
        return <p key={`p-${blockIndex}`}>{renderInlineMarkdown(block.text)}</p>;
      })}
    </div>
  );
}
