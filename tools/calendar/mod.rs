// Calendar tool — provides Outlook calendar integration.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub title: String,
    pub start: String,
    pub end: String,
    pub description: Option<String>,
    pub location: Option<String>,
}

impl CalendarEvent {
    pub fn new(title: &str, start: &str, end: &str) -> Self {
        Self {
            title: title.to_string(),
            start: start.to_string(),
            end: end.to_string(),
            description: None,
            location: None,
        }
    }

    pub fn with_description(mut self, description: &str) -> Self {
        self.description = Some(description.to_string());
        self
    }

    pub fn with_location(mut self, location: &str) -> Self {
        self.location = Some(location.to_string());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlookCalendar {
    pub id: String,
    pub name: String,
    pub store_name: String,
    pub email_address: Option<String>,
    pub path: String,
    pub is_selected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarDeliveryResult {
    pub saved_to_calendar: bool,
    pub file_opened: bool,
    pub message: String,
    pub method: String,
}

pub fn list_outlook_calendars() -> Result<Vec<OutlookCalendar>, String> {
    Ok(Vec::new())
}

pub fn select_outlook_calendar(_calendar_id: &str) -> Result<OutlookCalendar, String> {
    Err("Outlook integration not available".to_string())
}

pub fn create_calendar_event(_event: &CalendarEvent) -> CalendarDeliveryResult {
    CalendarDeliveryResult {
        saved_to_calendar: false,
        file_opened: false,
        message: "Calendar integration not available".to_string(),
        method: "none".to_string(),
    }
}

pub fn build_event_parse_prompt(prompt: &str) -> String {
    format!("Parse the following into a calendar event: {}", prompt)
}

pub fn parse_response_to_event(_raw: &str) -> Result<CalendarEvent, String> {
    Err("Calendar event parsing not available".to_string())
}
