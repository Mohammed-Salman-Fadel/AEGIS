// Calendar tool types

export interface CalendarResult {
  title: string;
  start: string;
  end: string;
  description?: string | null;
  location?: string | null;
}

export interface CalendarCreateResponse {
  message: string;
  saved_to_calendar?: boolean;
  file_opened?: boolean;
  delivery_method?: string;
  parsed: CalendarResult | null;
}

export interface OutlookCalendar {
  id: string;
  name: string;
  store_name: string;
  email_address?: string | null;
  path: string;
  is_selected: boolean;
}

export interface OutlookCalendarsResponse {
  calendars: OutlookCalendar[];
}

export interface OutlookCalendarSelectionResponse {
  calendar: OutlookCalendar;
  message: string;
}
