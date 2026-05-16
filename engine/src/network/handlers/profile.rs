use axum::{Json, http::StatusCode};
use serde::{Deserialize, Serialize};

use crate::user_profile;

#[derive(Serialize)]
pub struct ProfileResponse {
    contents: String,
    path: String,
}

#[derive(Deserialize)]
pub struct SaveProfileRequest {
    contents: String,
}

pub async fn get_profile() -> Result<Json<ProfileResponse>, (StatusCode, String)> {
    let contents = user_profile::read_profile_text().map_err(profile_error)?;
    Ok(Json(ProfileResponse {
        contents,
        path: user_profile::profile_file_path()
            .to_string_lossy()
            .to_string(),
    }))
}

pub async fn save_profile(
    Json(payload): Json<SaveProfileRequest>,
) -> Result<Json<ProfileResponse>, (StatusCode, String)> {
    let path = user_profile::write_profile_text(&payload.contents).map_err(profile_error)?;
    Ok(Json(ProfileResponse {
        contents: payload.contents,
        path: path.to_string_lossy().to_string(),
    }))
}

fn profile_error(error: std::io::Error) -> (StatusCode, String) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Could not access the local user profile file: {error}"),
    )
}
