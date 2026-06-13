// Animated dots shown while the assistant is generating a response

export function ThinkingIndicator({ isDark }: { isDark: boolean }) {
  return (
    <div className={`flex items-center gap-2 text-xs font-medium ${isDark ? 'text-zinc-400' : 'text-slate-500'}`}>
      <span>Thinking</span>
      <span className="flex items-center gap-1" aria-hidden="true">
        <span className="thinking-dot" />
        <span className="thinking-dot thinking-dot-delay-1" />
        <span className="thinking-dot thinking-dot-delay-2" />
      </span>
    </div>
  );
}
