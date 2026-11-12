//! What the relay calls: another server is there with these files (`PUT /`), or it has gone (`DELETE /`).
//!
//! Only the relay can make such a call: it is signed with the key of the relay (see `relay_auth`). A server cannot, and neither
//! can anyone who has found out some server's token.

use crate::{
    model::Payload,
    registry::{InsertError, Registry},
    relay_auth::RelayVerifier,
};
use ntex::{
    http::Response,
    util::Bytes,
    web::{types::State, HttpRequest},
};
use std::sync::Arc;

pub async fn put_origin(
    req: HttpRequest,
    body: Bytes,
    registry: State<Arc<Registry>>,
    verifier: State<Arc<RelayVerifier>>,
) -> Response {
    let payload = match open(&req, &body, &verifier) {
        Ok(payload) => payload,
        Err(response) => return response,
    };
    // a payload without `files` changes nothing; nobody lists this server in its own page
    let Some(files) = payload.files else {
        return Response::Ok().finish();
    };
    if is_self(&payload.origin, &verifier) {
        return Response::Ok().finish();
    }
    answer(registry.replace(&payload.origin, files).map(|_| ()))
}

pub async fn delete_origin(
    req: HttpRequest,
    body: Bytes,
    registry: State<Arc<Registry>>,
    verifier: State<Arc<RelayVerifier>>,
) -> Response {
    let payload = match open(&req, &body, &verifier) {
        Ok(payload) => payload,
        Err(response) => return response,
    };
    answer(registry.remove(&payload.origin))
}

/// The payload, if the call is from the relay.
fn open(req: &HttpRequest, body: &[u8], verifier: &RelayVerifier) -> Result<Payload, Response> {
    if !verifier.verify_request(req, body) {
        return Err(Response::Unauthorized().finish());
    }
    serde_json::from_slice(body).map_err(|_| Response::BadRequest().body("not a valid payload"))
}

fn is_self(origin: &str, verifier: &RelayVerifier) -> bool {
    origin.trim_end_matches('/') == verifier.origin()
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
