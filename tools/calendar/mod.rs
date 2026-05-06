use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::inference::InferenceBackend;

pub struct CalendarEvent {
    pub title: String,
    pub start: String,
    pub end: String,
    pub description: Option<String>,
    pub location: Option<String>,
}

pub struct CalendarDelivery {
    pub saved_to_calendar: bool,
    pub file_opened: bool,
    pub method: &'static str,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OutlookCalendar {
    pub id: String,
    pub store_id: String,
    pub name: String,
    pub store_name: String,
    pub email_address: Option<String>,
    pub path: String,
    #[serde(default)]
    pub is_selected: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct OutlookCalendarPreference {
    calendar_id: String,
    store_id: String,
    calendar_name: String,
    store_name: String,
    email_address: Option<String>,
    path: String,
}

#[derive(Deserialize)]
struct ParsedEvent {
    title: String,
    start: String,
    end: String,
    description: Option<String>,
    location: Option<String>,
}

pub async fn parse_prompt(
    inference: &dyn InferenceBackend,
    model: &str,
    prompt: &str,
) -> anyhow::Result<CalendarEvent> {
    let full_prompt = build_event_parse_prompt(prompt);
    let response = inference.call(&full_prompt, model).await?;
    parse_response_to_event(&response)
}

pub fn build_event_parse_prompt(user_text: &str) -> String {
    let today = chrono::Utc::now();
    let date_hint = today.format("%Y-%m-%d (%A)").to_string();

    format!(
        "You are a strict calendar event parser. Extract event details from the user's text. \
        Return exactly one raw JSON object and nothing else. Do not explain. Do not use markdown. \
        Do not wrap the JSON in code fences, LaTeX, boxed output, prose, bullets, or validation text. \
        Required JSON fields: title (string), start (ISO 8601 datetime string), end (ISO 8601 datetime string), \
        description (string or null), location (string or null). \
        Use today's date ({date_hint}) as the reference for relative dates like 'tomorrow' or 'next week'. \
        Assume a 1-hour duration if no end time is given.\n\nUser text: {user_text}"
    )
}

pub fn parse_response_to_event(response: &str) -> anyhow::Result<CalendarEvent> {
    let cleaned = response
        .lines()
        .filter(|line| !line.trim_start().starts_with("```"))
        .collect::<Vec<_>>()
        .join("\n");

    let json = extract_event_json(&cleaned).ok_or_else(|| {
        anyhow::anyhow!("Failed to find event JSON in LLM response.\nRaw response: {response}")
    })?;

    let parsed: ParsedEvent = serde_json::from_str(&json).map_err(|e| {
        anyhow::anyhow!("Failed to parse extracted event JSON: {e}\nExtracted JSON: {json}\nRaw response: {response}")
    })?;

    Ok(CalendarEvent {
        title: parsed.title,
        start: parsed.start,
        end: parsed.end,
        description: parsed.description,
        location: parsed.location,
    })
}

fn extract_event_json(response: &str) -> Option<String> {
    let direct = response.trim();
    if serde_json::from_str::<serde_json::Value>(direct).is_ok() {
        return Some(direct.to_string());
    }

    let chars: Vec<char> = response.chars().collect();
    let mut start_index = 0;

    while start_index < chars.len() {
        while start_index < chars.len() && chars[start_index] != '{' {
            start_index += 1;
        }

        if start_index >= chars.len() {
            return None;
        }

        let mut depth = 0;
        let mut in_string = false;
        let mut escaped = false;

        for end_index in start_index..chars.len() {
            let ch = chars[end_index];

            if in_string {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    in_string = false;
                }
                continue;
            }

            match ch {
                '"' => in_string = true,
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        let candidate: String = chars[start_index..=end_index].iter().collect();
                        for normalized in normalized_json_candidates(&candidate) {
                            if serde_json::from_str::<ParsedEvent>(&normalized).is_ok() {
                                return Some(normalized);
                            }
                        }
                        break;
                    }
                }
                _ => {}
            }
        }

        start_index += 1;
    }

    None
}

fn normalized_json_candidates(candidate: &str) -> Vec<String> {
    let trimmed = candidate.trim();
    let mut candidates = vec![trimmed.to_string()];

    if trimmed.starts_with("{{") && trimmed.ends_with("}}") {
        candidates.push(trimmed[1..trimmed.len() - 1].to_string());
    }

    candidates
}

fn format_ics_dt(dt: &str) -> String {
    let digits: String = dt.chars().filter(|c| c.is_ascii_digit()).collect();
    let date = digits.get(0..8).unwrap_or("19700101");
    let time = digits.get(8..14).unwrap_or("000000");
    format!("{date}T{time}Z")
}

fn escape_ics_text(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace('\n', "\\n")
}

pub fn generate_ics(event: &CalendarEvent) -> String {
    let uid = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now();
    let dtstamp = now.format("%Y%m%dT%H%M%SZ").to_string();
    let start = format_ics_dt(&event.start);
    let end = format_ics_dt(&event.end);

    let mut ics = String::new();
    ics.push_str("BEGIN:VCALENDAR\r\n");
    ics.push_str("VERSION:2.0\r\n");
    ics.push_str("PRODID:-//AEGIS//Calendar//EN\r\n");
    ics.push_str("BEGIN:VEVENT\r\n");
    ics.push_str(&format!("UID:{}\r\n", uid));
    ics.push_str(&format!("DTSTAMP:{}\r\n", dtstamp));
    ics.push_str(&format!("DTSTART:{}\r\n", start));
    ics.push_str(&format!("DTEND:{}\r\n", end));
    ics.push_str(&format!("SUMMARY:{}\r\n", escape_ics_text(&event.title)));
    if let Some(loc) = &event.location {
        ics.push_str(&format!("LOCATION:{}\r\n", escape_ics_text(loc)));
    }
    if let Some(desc) = &event.description {
        ics.push_str(&format!("DESCRIPTION:{}\r\n", escape_ics_text(desc)));
    }
    ics.push_str("END:VEVENT\r\n");
    ics.push_str("END:VCALENDAR\r\n");
    ics
}

pub fn create_calendar_event(event: &CalendarEvent) -> CalendarDelivery {
    #[cfg(target_os = "windows")]
    {
        match save_to_outlook(event, selected_outlook_calendar().ok().as_ref()) {
            Ok(()) => {
                tracing::info!(title = %event.title, "calendar event saved directly to Outlook");
                return CalendarDelivery {
                    saved_to_calendar: true,
                    file_opened: false,
                    method: "outlook-local",
                    message: "Calendar event saved directly to local Outlook.".to_string(),
                };
            }
            Err(err) => {
                tracing::warn!(error = %err, "local Outlook calendar save failed; falling back to ICS");
            }
        }
    }

    let ics = generate_ics(event);
    match launch_ics(&ics, &event.title) {
        Ok(()) => CalendarDelivery {
            saved_to_calendar: false,
            file_opened: true,
            method: "ics-open",
            message: "Calendar event file opened in the default calendar app.".to_string(),
        },
        Err(err) => CalendarDelivery {
            saved_to_calendar: false,
            file_opened: false,
            method: "failed",
            message: format!("Could not save or open calendar event: {err}"),
        },
    }
}

#[cfg(target_os = "windows")]
pub fn list_outlook_calendars() -> anyhow::Result<Vec<OutlookCalendar>> {
    let script_path = write_outlook_list_script()?;
    let output = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&script_path)
        .output()?;

    let _ = std::fs::remove_file(&script_path);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        anyhow::bail!("Could not list Outlook calendars: {stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        return Ok(Vec::new());
    }

    let value: serde_json::Value = serde_json::from_str(&stdout)?;
    let mut calendars: Vec<OutlookCalendar> = match value {
        serde_json::Value::Array(_) => serde_json::from_value(value)?,
        serde_json::Value::Object(_) => vec![serde_json::from_value(value)?],
        _ => Vec::new(),
    };
    if let Ok(selected) = selected_outlook_calendar() {
        for calendar in &mut calendars {
            calendar.is_selected =
                calendar.id == selected.calendar_id && calendar.store_id == selected.store_id;
        }
    }

    Ok(calendars)
}

#[cfg(not(target_os = "windows"))]
pub fn list_outlook_calendars() -> anyhow::Result<Vec<OutlookCalendar>> {
    Ok(Vec::new())
}

pub fn select_outlook_calendar(calendar_id: &str) -> anyhow::Result<OutlookCalendar> {
    let calendars = list_outlook_calendars()?;
    let selected = calendars
        .into_iter()
        .find(|calendar| calendar.id == calendar_id)
        .ok_or_else(|| anyhow::anyhow!("Outlook calendar was not found."))?;

    let preference = OutlookCalendarPreference {
        calendar_id: selected.id.clone(),
        store_id: selected.store_id.clone(),
        calendar_name: selected.name.clone(),
        store_name: selected.store_name.clone(),
        email_address: selected.email_address.clone(),
        path: selected.path.clone(),
    };

    write_selected_outlook_calendar(&preference)?;
    Ok(OutlookCalendar {
        is_selected: true,
        ..selected
    })
}

#[cfg(target_os = "windows")]
fn save_to_outlook(
    event: &CalendarEvent,
    selected_calendar: Option<&OutlookCalendarPreference>,
) -> anyhow::Result<()> {
    let script_path = write_outlook_script()?;
    let output = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(&script_path)
        .arg(&event.title)
        .arg(&event.start)
        .arg(&event.end)
        .arg(event.description.as_deref().unwrap_or(""))
        .arg(event.location.as_deref().unwrap_or(""))
        .arg(
            selected_calendar
                .map(|calendar| calendar.calendar_id.as_str())
                .unwrap_or(""),
        )
        .arg(
            selected_calendar
                .map(|calendar| calendar.store_id.as_str())
                .unwrap_or(""),
        )
        .output()?;

    let _ = std::fs::remove_file(&script_path);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        anyhow::bail!(
            "Outlook automation failed: {}{}",
            stderr,
            if stdout.is_empty() {
                String::new()
            } else {
                format!(" {stdout}")
            }
        );
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn write_outlook_script() -> anyhow::Result<std::path::PathBuf> {
    let script = r#"
param(
    [Parameter(Mandatory=$true)][string]$Title,
    [Parameter(Mandatory=$true)][string]$Start,
    [Parameter(Mandatory=$true)][string]$End,
    [string]$Description = "",
    [string]$Location = "",
    [string]$CalendarId = "",
    [string]$StoreId = ""
)

function Convert-AegisDateTime([string]$Value) {
    if ([string]::IsNullOrWhiteSpace($Value)) {
        throw "Calendar datetime cannot be empty."
    }

    $culture = [System.Globalization.CultureInfo]::InvariantCulture
    $styles = [System.Globalization.DateTimeStyles]::AssumeLocal
    $formats = @(
        "yyyy-MM-ddTHH:mm:ssK",
        "yyyy-MM-ddTHH:mmK",
        "yyyy-MM-dd HH:mm:ss",
        "yyyy-MM-dd HH:mm",
        "yyyy-MM-ddTHH:mm:ss",
        "yyyy-MM-ddTHH:mm"
    )

    $parsed = [datetime]::MinValue
    if ([datetime]::TryParseExact($Value, $formats, $culture, $styles, [ref]$parsed)) {
        return $parsed
    }

    return [datetime]::Parse($Value, $culture)
}

$outlook = New-Object -ComObject Outlook.Application
$namespace = $outlook.Session
try {
    $namespace.Logon($null, $null, $false, $false)
} catch {}

if (-not [string]::IsNullOrWhiteSpace($CalendarId)) {
    if ([string]::IsNullOrWhiteSpace($StoreId)) {
        $calendar = $namespace.GetFolderFromID($CalendarId)
    } else {
        $calendar = $namespace.GetFolderFromID($CalendarId, $StoreId)
    }
    $appointment = $calendar.Items.Add(1)
} else {
    $appointment = $outlook.CreateItem(1)
}

$appointment.Subject = $Title
$appointment.Start = Convert-AegisDateTime $Start
$appointment.End = Convert-AegisDateTime $End
$appointment.Body = $Description
$appointment.Location = $Location
$appointment.BusyStatus = 2
$appointment.ReminderSet = $true
$appointment.ReminderMinutesBeforeStart = 15
$appointment.Save()
"#;

    let path = std::env::temp_dir().join(format!(
        "aegis_outlook_calendar_{}.ps1",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    std::fs::write(&path, script)?;
    Ok(path)
}

#[cfg(target_os = "windows")]
fn write_outlook_list_script() -> anyhow::Result<std::path::PathBuf> {
    let script = r#"
$outlook = New-Object -ComObject Outlook.Application
$namespace = $outlook.Session
try {
    $namespace.Logon($null, $null, $false, $false)
} catch {}
$calendars = New-Object System.Collections.Generic.List[object]
$storeEmails = @{}

try {
    foreach ($account in $namespace.Accounts) {
        try {
            $smtp = [string]$account.SmtpAddress
            if ([string]::IsNullOrWhiteSpace($smtp)) {
                $smtp = [string]$account.DisplayName
            }

            $deliveryStoreId = ""
            try {
                if ($null -ne $account.DeliveryStore) {
                    $deliveryStoreId = [string]$account.DeliveryStore.StoreID
                }
            } catch {}

            if (-not [string]::IsNullOrWhiteSpace($deliveryStoreId) -and -not [string]::IsNullOrWhiteSpace($smtp)) {
                $storeEmails[$deliveryStoreId] = $smtp
            }
        } catch {}
    }
} catch {}

function Resolve-StoreEmail([string]$StoreId, [string]$StoreName) {
    if ($storeEmails.ContainsKey($StoreId)) {
        return [string]$storeEmails[$StoreId]
    }

    if ($StoreName -match '^[^@\s]+@[^@\s]+\.[^@\s]+$') {
        return $StoreName
    }

    return $null
}

function Add-CalendarFolder($Folder, [string]$StoreName, [string]$StoreId) {
    if ($null -eq $Folder) {
        return
    }

    try {
        if ($Folder.DefaultItemType -eq 1) {
            $calendars.Add([PSCustomObject]@{
                id = [string]$Folder.EntryID
                store_id = $StoreId
                name = [string]$Folder.Name
                store_name = $StoreName
                email_address = Resolve-StoreEmail $StoreId $StoreName
                path = [string]$Folder.FolderPath
                is_selected = $false
            })
        }
    } catch {}

    try {
        foreach ($child in $Folder.Folders) {
            Add-CalendarFolder $child $StoreName $StoreId
        }
    } catch {}
}

foreach ($store in $namespace.Stores) {
    try {
        $storeName = [string]$store.DisplayName
        $storeId = [string]$store.StoreID
        $root = $store.GetRootFolder()
        Add-CalendarFolder $root $storeName $storeId
    } catch {}
}

$calendars | ConvertTo-Json -Compress
"#;

    let path = std::env::temp_dir().join(format!(
        "aegis_outlook_calendars_{}.ps1",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    std::fs::write(&path, script)?;
    Ok(path)
}

fn selected_outlook_calendar() -> anyhow::Result<OutlookCalendarPreference> {
    let path = outlook_calendar_preference_path();
    let contents = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&contents)?)
}

fn write_selected_outlook_calendar(preference: &OutlookCalendarPreference) -> anyhow::Result<()> {
    let path = outlook_calendar_preference_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_string_pretty(preference)?;
    std::fs::write(path, json)?;
    Ok(())
}

fn outlook_calendar_preference_path() -> PathBuf {
    local_aegis_config_dir().join("outlook_calendar.json")
}

fn local_aegis_config_dir() -> PathBuf {
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        return PathBuf::from(local_app_data).join("AEGIS").join("config");
    }

    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        return PathBuf::from(user_profile)
            .join("AppData")
            .join("Local")
            .join("AEGIS")
            .join("config");
    }

    std::env::temp_dir().join("AEGIS").join("config")
}

pub fn launch_ics(ics_content: &str, title: &str) -> anyhow::Result<()> {
    let temp_dir = std::env::temp_dir();
    let safe_name: String = title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let file_name = format!("aegis_{}_{}.ics", safe_name, chrono::Utc::now().timestamp());
    let file_path = temp_dir.join(&file_name);

    let mut file = std::fs::File::create(&file_path)?;
    file.write_all(ics_content.as_bytes())?;

    let path_str = file_path.to_string_lossy().to_string();

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/c", "start", "", &path_str])
            .spawn()?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        std::process::Command::new("open")
            .arg(&path_str)
            .spawn()
            .or_else(|_| {
                std::process::Command::new("xdg-open")
                    .arg(&path_str)
                    .spawn()
            })?;
    }

    tracing::info!(file = %path_str, "calendar event file created and opened");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_json_event() {
        let event = parse_response_to_event(
            r#"{"title":"meeting","start":"2026-05-07T23:00:00","end":"2026-05-08T00:00:00","description":null,"location":null}"#,
        )
        .expect("plain JSON should parse");

        assert_eq!(event.title, "meeting");
        assert_eq!(event.start, "2026-05-07T23:00:00");
        assert_eq!(event.end, "2026-05-08T00:00:00");
    }

    #[test]
    fn extracts_boxed_json_from_noisy_model_output() {
        let event = parse_response_to_event(
            r#"The JSON output is valid and adheres to the specified format.

\boxed{{"title": "set up meeting with jasser", "start": "2026-05-07T23:00:00", "end": "2026-05-08T12:00:00"}}

The ISO 8601 date-time format is used for both times."#,
        )
        .expect("boxed JSON should be extracted");

        assert_eq!(event.title, "set up meeting with jasser");
        assert_eq!(event.start, "2026-05-07T23:00:00");
        assert_eq!(event.end, "2026-05-08T12:00:00");
    }

    #[test]
    fn formats_ics_datetime_with_required_separator() {
        assert_eq!(format_ics_dt("2026-05-07T23:00:00"), "20260507T230000Z");
    }
}
