pub mod entity;
use crate::{
    AppState,
    error::{AppError, IssueCode, ValidationErrors},
};
use axum::{
    Json,
    extract::{
        Path, State,
        rejection::{JsonRejection, PathRejection},
    },
    http::StatusCode,
};
use sea_orm::{ActiveModelTrait, ConnectionTrait, EntityTrait, Set};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateNote {
    /// Whitespace is trimmed before validating 1 to 200 Unicode scalar values.
    pub title: String,
}
#[derive(Serialize, ToSchema)]
pub struct Note {
    pub id: Uuid,
    pub title: String,
}

#[derive(Deserialize)]
pub struct NoteInput {
    #[serde(default)]
    title: Value,
    #[serde(flatten)]
    unknown: BTreeMap<String, serde::de::IgnoredAny>,
}

impl NoteInput {
    fn validate(self) -> Result<CreateNote, AppError> {
        let mut errors = ValidationErrors::default();
        if !self.unknown.is_empty() {
            errors.add(
                [],
                IssueCode::UnrecognizedKeys,
                "Unknown fields are not allowed.",
            );
        }
        let title = match self.title {
            Value::String(title) => match validate_title(&title) {
                Ok(title) => title,
                Err(title_errors) => {
                    errors.merge(title_errors);
                    String::new()
                }
            },
            Value::Null => {
                errors.add(
                    ["title".into()],
                    IssueCode::InvalidType,
                    "The title field is required.",
                );
                String::new()
            }
            _ => {
                errors.add(
                    ["title".into()],
                    IssueCode::InvalidType,
                    "The title must be a string.",
                );
                String::new()
            }
        };
        errors.finish()?;
        Ok(CreateNote { title })
    }
}

pub fn validate_title(title: &str) -> Result<String, ValidationErrors> {
    let title = title.trim();
    let issue = if title.is_empty() {
        Some((IssueCode::TooSmall, "The title field is required."))
    } else if title.chars().count() > 200 {
        Some((
            IssueCode::TooBig,
            "The title must not be greater than 200 characters.",
        ))
    } else {
        None
    };
    if let Some((code, message)) = issue {
        let mut errors = ValidationErrors::default();
        errors.add(["title".into()], code, message);
        return Err(errors);
    }
    Ok(title.into())
}

pub async fn create_note(db: &impl ConnectionTrait, input: CreateNote) -> Result<Note, AppError> {
    let title = validate_title(&input.title)?;
    let model = entity::ActiveModel {
        id: Set(Uuid::now_v7()),
        title: Set(title),
    }
    .insert(db)
    .await?;
    Ok(Note {
        id: model.id,
        title: model.title,
    })
}

#[utoipa::path(post, path = "/example/notes", request_body = CreateNote, responses(
    (status = 201, description = "Created note", body = Note),
    (status = 400, description = "Malformed JSON", body = crate::error::Problem, content_type = "application/problem+json"),
    (status = 413, description = "Body too large", body = crate::error::Problem, content_type = "application/problem+json"),
    (status = 415, description = "Expected JSON", body = crate::error::Problem, content_type = "application/problem+json"),
    (status = 422, description = "Invalid note", body = crate::error::Problem, content_type = "application/problem+json"),
    (status = 408, description = "Deadline exceeded", body = crate::error::Problem, content_type = "application/problem+json"),
    (status = 503, description = "Database unavailable", body = crate::error::Problem, content_type = "application/problem+json")
))]
pub async fn create(
    State(state): State<AppState>,
    input: Result<Json<NoteInput>, JsonRejection>,
) -> Result<(StatusCode, Json<Note>), AppError> {
    let Json(input) = input.map_err(|error| match error {
        JsonRejection::JsonDataError(_) => {
            let mut errors = ValidationErrors::default();
            errors.add(
                [],
                IssueCode::Custom,
                "Expected an object with no duplicate fields.",
            );
            AppError::from(errors)
        }
        _ => AppError::new(error.status(), "Invalid JSON request"),
    })?;
    let input = input.validate()?;
    Ok((
        StatusCode::CREATED,
        Json(create_note(&state.db, input).await?),
    ))
}

#[utoipa::path(get, path = "/example/notes/{id}", params(("id" = Uuid, Path, description = "Note ID")), responses(
    (status = 200, description = "Note", body = Note),
    (status = 400, description = "Invalid ID", body = crate::error::Problem, content_type = "application/problem+json"),
    (status = 404, description = "Missing note", body = crate::error::Problem, content_type = "application/problem+json"),
    (status = 408, description = "Deadline exceeded", body = crate::error::Problem, content_type = "application/problem+json"),
    (status = 503, description = "Database unavailable", body = crate::error::Problem, content_type = "application/problem+json")
))]
pub async fn get(
    State(state): State<AppState>,
    id: Result<Path<Uuid>, PathRejection>,
) -> Result<Json<Note>, AppError> {
    let Path(id) = id.map_err(|_| AppError::new(StatusCode::BAD_REQUEST, "id must be a UUID"))?;
    let model = entity::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or(AppError::new(StatusCode::NOT_FOUND, "Note not found"))?;
    Ok(Json(Note {
        id: model.id,
        title: model.title,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn title_boundaries() {
        assert_eq!(validate_title(" hi ").ok().unwrap(), "hi");
        assert!(validate_title(" \n").is_err());
        assert!(validate_title(&"é".repeat(200)).is_ok());
        assert!(validate_title(&"a".repeat(201)).is_err());
    }
}
