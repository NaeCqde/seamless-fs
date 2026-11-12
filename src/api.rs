//! What the relay calls: another server is there with these files (`PUT /`), or it has gone (`DELETE /`).

use crate::{
    model::Payload,
    registry::{InsertError, Registry},
};
use ntex::{
    http::Response,
    web::types::{Json, State},
};
use std::sync::Arc;

pub async fn put_origin(payload: Json<Payload>, registry: State<Arc<Registry>>) -> Response {
    let payload = payload.into_inner();
    // a payload without `files` changes nothing
    let Some(files) = payload.files else {
        return Response::Ok().finish();
    };
    answer(registry.replace(&payload.origin, files).map(|_| ()))
}

pub async fn delete_origin(payload: Json<Payload>, registry: State<Arc<Registry>>) -> Response {
    answer(registry.remove(&payload.origin))
}

fn answer(result: Result<(), InsertError>) -> Response {
    match result {
        Ok(()) => Response::Ok().finish(),
        Err(InsertError::BadOrigin) => {
            Response::BadRequest().body("origin must be an http or https URL")
        }
        Err(InsertError::TooManyOrigins | InsertError::TooManyEntries) => {
            Response::PayloadTooLarge().body("too many servers or files")
        }
    }
}

/// A registration without the right token.
pub async fn unauthorized() -> Response {
    Response::Unauthorized().finish()
}
