// Calendar helper utilities
import type { OutlookCalendar } from '../types';

export function cleanOutlookCalendarName(name: string) {
  return name.replace(/\s*\(this computer only\)\s*/gi, ' ').replace(/\s+/g, ' ').trim();
}

export function isGenericOutlookDataFileCalendar(calendar: OutlookCalendar) {
  const calendarName = cleanOutlookCalendarName(calendar.name).toLowerCase();
  const storeName = calendar.store_name.trim().toLowerCase();
  const hasEmail = Boolean(calendar.email_address?.trim());
  return !hasEmail && calendarName === 'calendar' && storeName.includes('outlook data file');
}

export function isVisibleOutlookCalendar(calendar: OutlookCalendar) {
  return !isGenericOutlookDataFileCalendar(calendar);
}

export function outlookCalendarLabel(calendar: OutlookCalendar) {
  const calendarName = cleanOutlookCalendarName(calendar.name);
  const emailAddress = calendar.email_address?.trim();
  const storeName = calendar.store_name.trim();
  if (emailAddress) return `${calendarName} (${emailAddress})`;
  if (storeName && !storeName.toLowerCase().includes('outlook data file')) return `${calendarName} (${storeName})`;
  return calendarName;
}
