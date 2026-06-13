// Calendar event creation modal with Outlook calendar selection
import { Calendar, X } from 'lucide-react';
import type { OutlookCalendar, CalendarResult } from '../types';
import { outlookCalendarLabel } from '../lib';

interface CalendarModalProps {
  isDark: boolean;
  calendarOpen: boolean;
  calendarPrompt: string;
  creatingCalendarEvent: boolean;
  loadingOutlookCalendars: boolean;
  outlookCalendars: OutlookCalendar[];
  selectedOutlookCalendarId: string;
  calendarResult: CalendarResult | null;
  calendarMessage: string | null;
  onClose: () => void;
  onCalendarPromptChange: (value: string) => void;
  onCalendarSelect: (id: string) => void;
  onCreateEvent: () => void;
}

export function CalendarModal({
  isDark,
  calendarOpen,
  calendarPrompt,
  creatingCalendarEvent,
  loadingOutlookCalendars,
  outlookCalendars,
  selectedOutlookCalendarId,
  calendarResult,
  calendarMessage,
  onClose,
  onCalendarPromptChange,
  onCalendarSelect,
  onCreateEvent,
}: CalendarModalProps) {
  if (!calendarOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4" onClick={onClose}>
      <div
        className={`w-full max-w-lg rounded-xl border p-6 shadow-2xl ${isDark ? 'border-zinc-800 bg-zinc-950 text-zinc-100' : 'border-stone-300 bg-white text-slate-900'}`}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="mb-4 flex items-center justify-between">
          <div className="flex items-center gap-2 text-lg font-semibold">
            <Calendar size={18} />
            Create Calendar Event
          </div>
          <button className={`rounded-md p-1 ${isDark ? 'hover:bg-zinc-900' : 'hover:bg-stone-100'}`} onClick={onClose} type="button">
            <X size={18} />
          </button>
        </div>

        <div className="mb-4 space-y-2">
          <label className="text-xs font-semibold uppercase tracking-wide opacity-70">Local Outlook calendar</label>
          <select
            className={`w-full rounded-lg border px-3 py-2 text-sm outline-none focus:border-emerald-600 ${isDark ? 'border-zinc-800 bg-zinc-900 text-zinc-100' : 'border-stone-300 bg-white text-slate-900'}`}
            disabled={creatingCalendarEvent || loadingOutlookCalendars || outlookCalendars.length === 0}
            onChange={(e) => onCalendarSelect(e.target.value)}
            value={selectedOutlookCalendarId}
          >
            <option value="">
              {loadingOutlookCalendars ? 'Loading Outlook calendars...' : outlookCalendars.length === 0 ? 'Default Outlook calendar / ICS fallback' : 'Choose an Outlook calendar'}
            </option>
            {outlookCalendars.map((cal) => (
              <option key={`${cal.store_name}-${cal.id}`} value={cal.id}>{outlookCalendarLabel(cal)}</option>
            ))}
          </select>
          <p className="text-xs opacity-60">AEGIS uses local Outlook only.</p>
          {calendarMessage && !calendarResult && (
            <div className={`rounded-lg border px-3 py-2 text-xs ${isDark ? 'border-emerald-800 bg-emerald-950/40 text-emerald-200' : 'border-emerald-300 bg-emerald-50 text-emerald-800'}`}>
              {calendarMessage}
            </div>
          )}
        </div>

        <textarea
          className={`mb-4 w-full rounded-lg border px-4 py-3 text-sm outline-none focus:border-emerald-600 ${isDark ? 'border-zinc-800 bg-zinc-900 text-zinc-100 placeholder:text-zinc-500' : 'border-stone-300 bg-white text-slate-900 placeholder:text-slate-400'}`}
          disabled={creatingCalendarEvent}
          onChange={(e) => onCalendarPromptChange(e.target.value)}
          placeholder='e.g. "Meeting with Jasser tomorrow at 3pm for 1 hour"'
          rows={3}
          value={calendarPrompt}
        />

        <button
          className="flex w-full items-center justify-center gap-2 rounded-lg bg-emerald-600 px-4 py-3 text-sm font-medium text-white hover:bg-emerald-500 disabled:opacity-60"
          disabled={creatingCalendarEvent || !calendarPrompt.trim()}
          onClick={onCreateEvent}
          type="button"
        >
          <Calendar size={16} />
          {creatingCalendarEvent ? 'Creating...' : 'Create Event'}
        </button>

        {(calendarMessage || calendarResult) && (
          <div className={`mt-4 rounded-lg border p-4 text-sm ${isDark ? 'border-emerald-800 bg-emerald-950/40 text-emerald-200' : 'border-emerald-300 bg-emerald-50 text-emerald-800'}`}>
            {calendarMessage && <div className="mb-2 font-semibold">{calendarMessage}</div>}
            {calendarResult && (
              <>
                <div className="mb-1 font-semibold">{calendarResult.title}</div>
                <div className="opacity-80">Start: {calendarResult.start}</div>
                <div className="opacity-80">End: {calendarResult.end}</div>
                {calendarResult.location && <div className="opacity-80">Location: {calendarResult.location}</div>}
                {calendarResult.description && <div className="mt-1 opacity-80">{calendarResult.description}</div>}
              </>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
