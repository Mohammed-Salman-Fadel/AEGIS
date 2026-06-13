// Full-screen voice mode overlay with orb visualization, recording controls, and TTS toggle
import { X, Mic, Volume2, VolumeX } from 'lucide-react';
import { VoiceOrb } from './VoiceOrb';
import type { Message } from '../types';

interface VoiceModeOverlayProps {
  isDark: boolean;
  isRecording: boolean;
  isSpeaking: boolean;
  isTranscribing: boolean;
  isStreaming: boolean;
  isTtsEnabled: boolean;
  analyser?: AnalyserNode;
  messages: Message[];
  onClose: () => void;
  onToggleTts: () => void;
  onStartRecording: () => void;
  onStopDictation: () => void;
}

export function VoiceModeOverlay({
  isDark,
  isRecording,
  isSpeaking,
  isTranscribing,
  isStreaming,
  isTtsEnabled,
  analyser,
  messages,
  onClose,
  onToggleTts,
  onStartRecording,
  onStopDictation,
}: VoiceModeOverlayProps) {
  const lastUserMessage = [...messages].reverse().find((m) => m.role === 'user');
  const lastAssistantMessage = [...messages].reverse().find((m) => m.role === 'assistant');

  return (
    <div className={`fixed inset-0 z-50 flex flex-col items-center justify-center p-6 backdrop-blur-xl transition-all duration-500 ${isDark ? 'bg-zinc-950/80' : 'bg-white/80'}`}>
      <button
        onClick={onClose}
        className={`absolute top-6 right-6 p-2 rounded-full transition ${isDark ? 'text-zinc-500 hover:bg-zinc-900 hover:text-zinc-100' : 'text-slate-400 hover:bg-stone-100 hover:text-slate-800'}`}
      >
        <X size={24} />
      </button>

      <VoiceOrb
        isListening={isRecording}
        isSpeaking={isSpeaking}
        isProcessing={isTranscribing || isStreaming}
        analyser={analyser}
        isDark={isDark}
      />

      <div className="mt-4 flex flex-col items-center gap-4 max-w-2xl px-4 text-center">
        {lastUserMessage && (
          <p className={`text-sm italic font-medium px-4 py-2 rounded-lg max-w-lg ${isDark ? 'text-zinc-300 bg-zinc-900/40' : 'text-slate-700 bg-stone-100/40'}`}>
            &ldquo;{lastUserMessage.content}&rdquo;
          </p>
        )}
        {lastAssistantMessage && lastAssistantMessage.content && (
          <div className={`w-full max-h-48 overflow-y-auto rounded-xl p-4 text-left text-sm border shadow-inner ${isDark ? 'bg-zinc-900/60 border-zinc-800 text-zinc-200' : 'bg-stone-50/60 border-stone-200 text-slate-800'}`}>
            <p className="whitespace-pre-wrap leading-relaxed">{lastAssistantMessage.content}</p>
          </div>
        )}
      </div>

      <div className="mt-8 flex flex-col items-center gap-6">
        <button
          onClick={() => { isRecording ? onStopDictation() : onStartRecording(); }}
          className={`group relative flex h-20 w-20 items-center justify-center rounded-full transition-all duration-300 ${isRecording
            ? 'bg-red-500 shadow-[0_0_40px_rgba(239,68,68,0.4)] scale-110'
            : 'bg-emerald-600 shadow-[0_0_30px_rgba(16,185,129,0.3)] hover:scale-105'}`}
        >
          {isRecording ? <X size={32} className="text-white" /> : <Mic size={32} className="text-white" />}
          {isRecording && <span className="absolute inset-0 animate-ping rounded-full bg-red-500/40" />}
        </button>

        <div className="flex items-center gap-4">
          <button
            onClick={onToggleTts}
            className={`flex items-center gap-2 rounded-lg px-4 py-2 text-xs font-medium transition ${isDark ? 'bg-zinc-900 text-zinc-400 hover:text-zinc-200' : 'bg-stone-100 text-slate-500 hover:text-slate-800'}`}
          >
            {isTtsEnabled ? <Volume2 size={16} /> : <VolumeX size={16} />}
            {isTtsEnabled ? 'Speech On' : 'Speech Off'}
          </button>
        </div>
      </div>
    </div>
  );
}
