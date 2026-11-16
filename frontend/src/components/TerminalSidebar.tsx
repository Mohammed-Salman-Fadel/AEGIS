import { useEffect, useRef, useState } from 'react';
import type { FormEvent, KeyboardEvent, PointerEvent as ReactPointerEvent } from 'react';
import { LoaderCircle, PanelRightClose } from 'lucide-react';

import { API_BASE } from '../constants';

interface TerminalSidebarProps {
  isDark: boolean;
  isOpen: boolean;
  provider?: string;
  model?: string;
  sessionId: string | null;
  status: string;
  onClose: () => void;
}

interface CommandResponse {
  command: string;
  stdout: string;
  stderr: string;
  exit_code: number | null;
}

interface TerminalEntry {
  command: string;
  output: string;
  failed: boolean;
}

const EMBEDDED_HELP = `Available examples:
  status
  doctor
  version
  provider list
  model list
  session list
  session show <id>
  chat "your prompt"
  save "a fact to remember"
  code inspect --path <repository>

Use --help after a command for its complete CLI options.
Lifecycle and interactive-TTY commands must be run in a system terminal.`;

const TERMINAL_DEFAULT_WIDTH = 420;
const TERMINAL_MIN_WIDTH = 320;
const TERMINAL_MAX_WIDTH = 720;

export function TerminalSidebar({
  isDark,
  isOpen,
  provider,
  model,
  sessionId,
  status,
  onClose,
}: TerminalSidebarProps) {
  const [input, setInput] = useState('');
  const [entries, setEntries] = useState<TerminalEntry[]>([]);
  const [history, setHistory] = useState<string[]>([]);
  const [historyIndex, setHistoryIndex] = useState(-1);
  const [running, setRunning] = useState(false);
  const [terminalWidth, setTerminalWidth] = useState(TERMINAL_DEFAULT_WIDTH);
  const [isResizing, setIsResizing] = useState(false);
  const outputRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const shouldFollowOutputRef = useRef(true);

  useEffect(() => {
    const output = outputRef.current;
    if (!output || !shouldFollowOutputRef.current) return;
    output.scrollTo({ top: output.scrollHeight, behavior: 'smooth' });
  }, [entries, running]);

  useEffect(() => {
    if (isOpen) window.setTimeout(() => inputRef.current?.focus(), 180);
  }, [isOpen]);

  useEffect(() => () => {
    document.body.style.removeProperty('cursor');
    document.body.style.removeProperty('user-select');
  }, []);

  const submitCommand = async (event: FormEvent) => {
    event.preventDefault();
    const command = input.trim();
    if (!command || running) return;

    setInput('');
    setHistory((current) => [...current.filter((item) => item !== command), command]);
    setHistoryIndex(-1);

    if (command.toLowerCase() === 'clear') {
      setEntries([]);
      return;
    }
    if (command.toLowerCase() === 'help') {
      setEntries((current) => [...current, { command, output: EMBEDDED_HELP, failed: false }]);
      return;
    }

    setRunning(true);
    try {
      const response = await fetch(`${API_BASE}/command-line/execute`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ command }),
      });
      const body = await response.text();
      if (!response.ok) throw new Error(body || `Command failed with HTTP ${response.status}.`);
      const result = JSON.parse(body) as CommandResponse;
      const output = [result.stdout.trimEnd(), result.stderr.trimEnd()].filter(Boolean).join('\n');
      setEntries((current) => [...current, {
        command,
        output: output || `(command completed with exit code ${result.exit_code ?? 'unknown'})`,
        failed: result.exit_code !== 0,
      }]);
    } catch (error) {
      setEntries((current) => [...current, {
        command,
        output: error instanceof Error ? error.message : 'The CLI command failed.',
        failed: true,
      }]);
    } finally {
      setRunning(false);
      window.setTimeout(() => inputRef.current?.focus(), 0);
    }
  };

  const navigateHistory = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key !== 'ArrowUp' && event.key !== 'ArrowDown') return;
    event.preventDefault();
    if (history.length === 0) return;
    if (event.key === 'ArrowUp') {
      const next = historyIndex < 0 ? history.length - 1 : Math.max(0, historyIndex - 1);
      setHistoryIndex(next);
      setInput(history[next]);
    } else {
      const next = historyIndex < 0 ? -1 : historyIndex + 1;
      if (next >= history.length) {
        setHistoryIndex(-1);
        setInput('');
      } else {
        setHistoryIndex(next);
        setInput(history[next]);
      }
    }
  };

  const handleOutputScroll = () => {
    const output = outputRef.current;
    if (!output) return;
    shouldFollowOutputRef.current = output.scrollHeight - output.scrollTop - output.clientHeight < 48;
  };

  const startResize = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (window.innerWidth < 768) return;

    event.preventDefault();
    setIsResizing(true);
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';

    const resize = (moveEvent: PointerEvent) => {
      const maxWidth = Math.min(TERMINAL_MAX_WIDTH, Math.max(TERMINAL_MIN_WIDTH, window.innerWidth - 240));
      const nextWidth = Math.max(TERMINAL_MIN_WIDTH, Math.min(maxWidth, window.innerWidth - moveEvent.clientX));
      setTerminalWidth(nextWidth);
    };

    const stopResize = () => {
      setIsResizing(false);
      document.body.style.removeProperty('cursor');
      document.body.style.removeProperty('user-select');
      window.removeEventListener('pointermove', resize);
      window.removeEventListener('pointerup', stopResize);
    };

    window.addEventListener('pointermove', resize);
    window.addEventListener('pointerup', stopResize, { once: true });
  };

  const sidebarWidth = `min(${terminalWidth}px, 94vw)`;

  return (
    <aside
      aria-label="Terminal"
      className={`aegis-terminal-container relative flex shrink-0 flex-col overflow-hidden border-l transition-[width,opacity,transform] duration-300 ease-in-out max-md:fixed max-md:inset-y-0 max-md:right-0 max-md:z-40 ${
        isOpen ? 'opacity-100' : 'border-transparent opacity-0 max-md:translate-x-full'
      } aegis-bg-surface aegis-border-subtle`}
      style={{ width: isOpen ? sidebarWidth : 0 }}
    >
      <div
        aria-hidden="true"
        className={`aegis-terminal-resize-handle max-md:hidden ${isResizing ? 'aegis-terminal-resize-handle-active' : ''}`}
        onPointerDown={startResize}
      />
      <div
        className={`flex min-h-0 flex-1 flex-col ${isOpen ? '' : 'pointer-events-none'}`}
        style={{ minWidth: sidebarWidth }}
      >
        <div className="flex h-12 shrink-0 items-center justify-between border-b px-4 aegis-border-subtle">
          <span className="font-mono text-[12px] font-medium tracking-wide text-emerald-400">AEGIS TERMINAL · LOCAL CLI</span>
          <button
            aria-label="Close terminal"
            className={`rounded-md p-1 transition ${isDark ? 'text-zinc-500 hover:bg-zinc-900 hover:text-zinc-300' : 'text-stone-500 hover:bg-[rgba(94,76,55,0.1)] hover:text-stone-800'}`}
            onClick={onClose}
            type="button"
          >
            <PanelRightClose size={16} />
          </button>
        </div>

        <div
          className="aegis-terminal-screen m-3 flex min-h-0 flex-1 cursor-text flex-col overflow-hidden rounded-lg border font-mono text-[13px] leading-6"
          onClick={() => inputRef.current?.focus()}
        >
          <div onScroll={handleOutputScroll} ref={outputRef} className="terminal-scroll min-h-0 flex-1 overflow-auto p-4">
            <div className="text-emerald-400">AEGIS CLI 0.1.0 — connected to local engine</div>
            <div className="mt-2 terminal-muted">engine: <span className="text-emerald-300">{status}</span> · provider: {provider ?? 'detecting'} · model: {model ?? 'not selected'}</div>
            {sessionId && <div className="terminal-muted break-all">session: {sessionId}</div>}
            <div className={`mt-2 ${isDark ? 'text-zinc-500' : 'text-stone-500'}`}>Type `help` for examples. Use ↑/↓ for command history.</div>

            {entries.map((entry, index) => (
              <div className="mt-4" key={`${entry.command}-${index}`}>
                <div className="text-emerald-300"><span className="text-emerald-500">aegis&gt;</span> {entry.command}</div>
                <pre className={`aegis-terminal-output whitespace-pre-wrap break-words font-mono ${entry.failed ? 'text-red-300' : 'text-emerald-50/90'}`}>{entry.output}</pre>
              </div>
            ))}
            {running && (
              <div className="mt-4 flex items-center gap-2 text-emerald-300">
                <LoaderCircle className="animate-spin" size={13} />
                running…
              </div>
            )}
            <form className="aegis-terminal-prompt mt-4 flex min-w-0 items-center" onSubmit={submitCommand}>
            <label className="mr-2 shrink-0 text-emerald-300" htmlFor="aegis-terminal-input"><span className="text-emerald-500">aegis&gt;</span></label>
            <input
              ref={inputRef}
              id="aegis-terminal-input"
              aria-label="AEGIS CLI command"
              autoComplete="off"
              className="min-w-0 flex-1 bg-transparent text-emerald-50 caret-emerald-300 outline-none"
              disabled={running}
              onChange={(event) => setInput(event.target.value)}
              onKeyDown={navigateHistory}
              spellCheck={false}
              value={input}
            />
            </form>
          </div>
        </div>
      </div>
    </aside>
  );
}
