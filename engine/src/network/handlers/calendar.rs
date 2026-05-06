use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};

use crate::calendar_tool;
use crate::calendar_tool::CalendarEvent;
use crate::network::state::AppState;

type HttpStatus = axum::http::StatusCode;

#[derive(Deserialize)]
pub struct CreateEventRequest {
    pub title: String,
    pub start: String,
    pub end: String,
    pub description: Option<String>,
    pub location: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateFromPromptRequest {
    pub prompt: String,
}

#[derive(Serialize)]
pub struct CreateEventResponse {
    pub event: String,
    pub message: String,
    pub saved_to_calendar: bool,
    pub file_opened: bool,
    pub delivery_method: String,
    pub parsed: Option<ParsedEventOut>,
}

#[derive(Deserialize)]
pub struct SelectOutlookCalendarRequest {
    pub calendar_id: String,
}

#[derive(Serialize)]
pub struct OutlookCalendarsResponse {
    pub calendars: Vec<OutlookCalendarOut>,
}

#[derive(Serialize)]
pub struct OutlookCalendarSelectionResponse {
    pub calendar: OutlookCalendarOut,
    pub message: String,
}

#[derive(Serialize)]
pub struct OutlookCalendarOut {
    pub id: String,
    pub name: String,
    pub store_name: String,
    pub email_address: Option<String>,
    pub path: String,
    pub is_selected: bool,
}

#[derive(Serialize)]
pub struct ParsedEventOut {
    pub title: String,
    pub start: String,
    pub end: String,
    pub description: Option<String>,
    pub location: Option<String>,
}

pub async fn list_outlook_calendars() -> Result<Json<OutlookCalendarsResponse>, (HttpStatus, String)>
{
    let calendars = calendar_tool::list_outlook_calendars().map_err(|err| {
        (
            HttpStatus::INTERNAL_SERVER_ERROR,
            format!("Could not list Outlook calendars: {err}"),
        )
    })?;

    Ok(Json(OutlookCalendarsResponse {
        calendars: calendars
            .into_iter()
            .map(OutlookCalendarOut::from)
            .collect(),
    }))
}

pub async fn select_outlook_calendar(
    Json(payload): Json<SelectOutlookCalendarRequest>,
) -> Result<Json<OutlookCalendarSelectionResponse>, (HttpStatus, String)> {
    let calendar_id = payload.calendar_id.trim();
    if calendar_id.is_empty() {
        return Err((
            HttpStatus::BAD_REQUEST,
            "Outlook calendar id is required.".to_string(),
        ));
    }

    let calendar = calendar_tool::select_outlook_calendar(calendar_id).map_err(|err| {
        (
            HttpStatus::BAD_REQUEST,
            format!("Could not select Outlook calendar: {err}"),
        )
    })?;

    Ok(Json(OutlookCalendarSelectionResponse {
        message: format!(
            "Outlook calendar selected: {} ({})",
            calendar.name, calendar.store_name
        ),
        calendar: OutlookCalendarOut::from(calendar),
    }))
}

pub async fn create_event(
    State(_state): State<AppState>,
    Json(payload): Json<CreateEventRequest>,
) -> Result<Json<CreateEventResponse>, (HttpStatus, String)> {
    let title = payload.title.trim().to_string();
    if title.is_empty() {
        return Err((
            HttpStatus::BAD_REQUEST,
            "Calendar event title cannot be empty.".to_string(),
        ));
    }
    let start = payload.start.trim().to_string();
    let end = payload.end.trim().to_string();
    if start.is_empty() || end.is_empty() {
        return Err((
            HttpStatus::BAD_REQUEST,
            "Calendar event start and end are required.".to_string(),
        ));
    }

    let event = CalendarEvent {
        title: title.clone(),
        start,
        end,
        description: payload
            .description
            .map(|d| d.trim().to_string())
            .filter(|d| !d.is_empty()),
        location: payload
            .location
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty()),
    };

    let delivery = calendar_tool::create_calendar_event(&event);
    if !delivery.saved_to_calendar && !delivery.file_opened {
        return Err((HttpStatus::INTERNAL_SERVER_ERROR, delivery.message));
    }

    Ok(Json(CreateEventResponse {
        event: title,
        message: delivery.message,
        saved_to_calendar: delivery.saved_to_calendar,
        file_opened: delivery.file_opened,
        delivery_method: delivery.method.to_string(),
        parsed: None,
    }))
}

impl From<calendar_tool::OutlookCalendar> for OutlookCalendarOut {
    fn from(calendar: calendar_tool::OutlookCalendar) -> Self {
        Self {
            id: calendar.id,
            name: calendar.name,
            store_name: calendar.store_name,
            email_address: calendar.email_address,
            path: calendar.path,
            is_selected: calendar.is_selected,
        }
    }
}

pub async fn create_from_prompt(
    State(state): State<AppState>,
    Json(payload): Json<CreateFromPromptRequest>,
) -> Result<Json<CreateEventResponse>, (HttpStatus, String)> {
    let prompt = payload.prompt.trim().to_string();
    if prompt.is_empty() {
        return Err((
            HttpStatus::BAD_REQUEST,
            "Prompt cannot be empty.".to_string(),
        ));
    }

    let model_name = state.orchestrator.current_model_name();
    let system_prompt = calendar_tool::build_event_parse_prompt(&prompt);

    let raw_response = state
        .orchestrator
        .call_inference(&system_prompt, &model_name)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "could not parse calendar event from prompt");
            (
                HttpStatus::BAD_REQUEST,
                format!("Could not parse event: {e}"),
            )
        })?;

    let event = calendar_tool::parse_response_to_event(&raw_response).map_err(|e| {
        tracing::error!(error = %e, raw = %raw_response, "failed to parse LLM response as event");
        (
            HttpStatus::BAD_REQUEST,
            format!("Could not parse event: {e}"),
        )
    })?;

    let delivery = calendar_tool::create_calendar_event(&event);
    if !delivery.saved_to_calendar && !delivery.file_opened {
        return Err((HttpStatus::INTERNAL_SERVER_ERROR, delivery.message));
    }

    let response_event = event.title.clone();
    Ok(Json(CreateEventResponse {
        event: response_event,
        message: delivery.message,
        saved_to_calendar: delivery.saved_to_calendar,
        file_opened: delivery.file_opened,
        delivery_method: delivery.method.to_string(),
        parsed: Some(ParsedEventOut {
            title: event.title,
            start: event.start,
            end: event.end,
            description: event.description,
            location: event.location,
        }),
    }))
}
