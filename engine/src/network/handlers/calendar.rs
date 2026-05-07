use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};

use crate::calendar_tool;
use crate::calendar_tool::CalendarEvent;
use crate::network::state::AppState;

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

#[derive(Deserialize)]
pub struct SelectOutlookCalendarRequest {
    pub calendar_id: String,
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

pub async fn list_outlook_calendars() -> Result<Json<OutlookCalendarsResponse>, (StatusCode, String)>
{
    let calendars = calendar_tool::list_outlook_calendars().map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Could not list Outlook calendars: {error}"),
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
) -> Result<Json<OutlookCalendarSelectionResponse>, (StatusCode, String)> {
    let calendar_id = payload.calendar_id.trim();
    if calendar_id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Outlook calendar id is required.".to_string(),
        ));
    }

    let calendar = calendar_tool::select_outlook_calendar(calendar_id).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            format!("Could not select Outlook calendar: {error}"),
        )
    })?;

    Ok(Json(OutlookCalendarSelectionResponse {
        message: format!(
            "Outlook calendar selected: {} ({})",
            calendar.name,
            calendar
                .email_address
                .as_deref()
                .unwrap_or(&calendar.store_name)
        ),
        calendar: OutlookCalendarOut::from(calendar),
    }))
}

pub async fn create_event(
    State(_state): State<AppState>,
    Json(payload): Json<CreateEventRequest>,
) -> Result<Json<CreateEventResponse>, (StatusCode, String)> {
    let title = payload.title.trim().to_string();
    let start = payload.start.trim().to_string();
    let end = payload.end.trim().to_string();

    if title.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Calendar event title cannot be empty.".to_string(),
        ));
    }
    if start.is_empty() || end.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Calendar event start and end are required.".to_string(),
        ));
    }

    let event = CalendarEvent {
        title: title.clone(),
        start,
        end,
        description: payload
            .description
            .map(|description| description.trim().to_string())
            .filter(|description| !description.is_empty()),
        location: payload
            .location
            .map(|location| location.trim().to_string())
            .filter(|location| !location.is_empty()),
    };

    let delivery = calendar_tool::create_calendar_event(&event);
    if !delivery.saved_to_calendar && !delivery.file_opened {
        return Err((StatusCode::INTERNAL_SERVER_ERROR, delivery.message));
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

pub async fn create_from_prompt(
    State(state): State<AppState>,
    Json(payload): Json<CreateFromPromptRequest>,
) -> Result<Json<CreateEventResponse>, (StatusCode, String)> {
    let prompt = payload.prompt.trim();
    if prompt.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Prompt cannot be empty.".to_string(),
        ));
    }

    let model_name = state.orchestrator.current_model_name();
    let system_prompt = calendar_tool::build_event_parse_prompt(prompt);
    let raw_response = state
        .orchestrator
        .call_inference(&system_prompt, &model_name)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "could not parse calendar event from prompt");
            (
                StatusCode::BAD_GATEWAY,
                format!("Could not parse event: {error}"),
            )
        })?;

    let event = calendar_tool::parse_response_to_event(&raw_response).map_err(|error| {
        tracing::error!(error = %error, raw = %raw_response, "failed to parse LLM response as event");
        (
            StatusCode::BAD_REQUEST,
            format!("Could not parse event: {error}"),
        )
    })?;

    let delivery = calendar_tool::create_calendar_event(&event);
    if !delivery.saved_to_calendar && !delivery.file_opened {
        return Err((StatusCode::INTERNAL_SERVER_ERROR, delivery.message));
    }

    let event_title = event.title.clone();
    Ok(Json(CreateEventResponse {
        event: event_title,
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
