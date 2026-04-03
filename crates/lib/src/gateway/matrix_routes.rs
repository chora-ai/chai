//! HTTP endpoints for Matrix E2EE verification (SAS) without Element.

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    Json,
};
use matrix_channel::matrix_sdk::ruma::OwnedUserId;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::channels::MatrixChannel;
use crate::config;
use crate::gateway::server::GatewayState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationTarget {
    pub user_id: String,
    pub flow_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationQuery {
    pub user_id: String,
    pub flow_id: String,
}

fn authorize_matrix_http(state: &GatewayState, headers: &HeaderMap) -> Result<(), StatusCode> {
    if let Some(ref tok) = state.required_token {
        let auth = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let expected = format!("Bearer {}", tok);
        if auth != expected {
            return Err(StatusCode::UNAUTHORIZED);
        }
        return Ok(());
    }
    if !config::is_loopback_bind(&state.config.gateway.bind) {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(())
}

fn matrix_or_404(state: &GatewayState) -> Result<&Arc<MatrixChannel>, StatusCode> {
    state
        .matrix_channel
        .as_ref()
        .ok_or(StatusCode::NOT_FOUND)
}

fn parse_user_id(s: &str) -> Result<OwnedUserId, StatusCode> {
    s.parse().map_err(|_| StatusCode::BAD_REQUEST)
}

/// GET /matrix/verification/pending — list pending to-device verification requests seen since startup.
pub async fn matrix_verification_pending(
    State(state): State<GatewayState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    authorize_matrix_http(&state, &headers)?;
    let m = matrix_or_404(&state)?;
    let list = m.pending_verifications().lock().unwrap().clone();
    Ok(Json(json!({ "pending": list })))
}

/// POST /matrix/verification/accept — accept an incoming verification request (`m.key.verification.request`).
pub async fn matrix_verification_accept(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    Json(body): Json<VerificationTarget>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    authorize_matrix_http(&state, &headers)?;
    let m = matrix_or_404(&state)?;
    let user_id = parse_user_id(body.user_id.trim())?;
    let req = m
        .client()
        .encryption()
        .get_verification_request(&user_id, body.flow_id.trim())
        .await;
    let Some(req) = req else {
        return Err(StatusCode::NOT_FOUND);
    };
    req.accept().await.map_err(|e| {
        log::warn!("matrix verification accept failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(json!({ "ok": true })))
}

/// POST /matrix/verification/start-sas — start SAS verification after the request is accepted and ready.
pub async fn matrix_verification_start_sas(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    Json(body): Json<VerificationTarget>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    authorize_matrix_http(&state, &headers)?;
    let m = matrix_or_404(&state)?;
    let user_id = parse_user_id(body.user_id.trim())?;
    let req = m
        .client()
        .encryption()
        .get_verification_request(&user_id, body.flow_id.trim())
        .await;
    let Some(req) = req else {
        return Err(StatusCode::NOT_FOUND);
    };
    let sas = req.start_sas().await.map_err(|e| {
        log::warn!("matrix verification start_sas failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let Some(_) = sas else {
        return Ok(Json(json!({ "ok": false, "reason": "SAS not available (wrong state?)" })));
    };
    Ok(Json(json!({ "ok": true })))
}

#[derive(Serialize)]
struct SasEmojiJson {
    symbol: String,
    description: String,
}

/// GET /matrix/verification/sas?user_id=&flow_id= — short auth string (emoji / decimals) for SAS verification.
pub async fn matrix_verification_sas(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    Query(q): Query<VerificationQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    authorize_matrix_http(&state, &headers)?;
    let m = matrix_or_404(&state)?;
    let user_id = parse_user_id(q.user_id.trim())?;
    let v = m
        .client()
        .encryption()
        .get_verification(&user_id, q.flow_id.trim())
        .await;
    let Some(v) = v else {
        return Err(StatusCode::NOT_FOUND);
    };
    let Some(sas) = v.sas() else {
        return Ok(Json(json!({
            "sas": false,
            "reason": "not a SAS verification yet"
        })));
    };
    let emoji = sas.emoji().map(|arr| {
        arr.iter()
            .map(|e| SasEmojiJson {
                symbol: e.symbol.to_string(),
                description: e.description.to_string(),
            })
            .collect::<Vec<_>>()
    });
    let decimals = sas.decimals();
    Ok(Json(json!({
        "sas": true,
        "emoji": emoji,
        "decimals": decimals,
        "canBePresented": sas.can_be_presented(),
        "isDone": sas.is_done(),
        "isCancelled": sas.is_cancelled(),
    })))
}

/// POST /matrix/verification/confirm — confirm SAS (emoji/decimals match the other device).
pub async fn matrix_verification_confirm(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    Json(body): Json<VerificationTarget>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    authorize_matrix_http(&state, &headers)?;
    let m = matrix_or_404(&state)?;
    let user_id = parse_user_id(body.user_id.trim())?;
    let v = m
        .client()
        .encryption()
        .get_verification(&user_id, body.flow_id.trim())
        .await;
    let Some(v) = v else {
        return Err(StatusCode::NOT_FOUND);
    };
    let Some(sas) = v.sas() else {
        return Err(StatusCode::BAD_REQUEST);
    };
    sas.confirm().await.map_err(|e| {
        log::warn!("matrix verification confirm failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    m.remove_pending_verification(user_id.as_str(), body.flow_id.trim());
    Ok(Json(json!({ "ok": true })))
}

/// POST /matrix/verification/mismatch — SAS did not match.
pub async fn matrix_verification_mismatch(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    Json(body): Json<VerificationTarget>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    authorize_matrix_http(&state, &headers)?;
    let m = matrix_or_404(&state)?;
    let user_id = parse_user_id(body.user_id.trim())?;
    let v = m
        .client()
        .encryption()
        .get_verification(&user_id, body.flow_id.trim())
        .await;
    let Some(v) = v else {
        return Err(StatusCode::NOT_FOUND);
    };
    let Some(sas) = v.sas() else {
        return Err(StatusCode::BAD_REQUEST);
    };
    sas.mismatch().await.map_err(|e| {
        log::warn!("matrix verification mismatch failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    m.remove_pending_verification(user_id.as_str(), body.flow_id.trim());
    Ok(Json(json!({ "ok": true })))
}

/// POST /matrix/verification/cancel — cancel verification request or SAS flow.
pub async fn matrix_verification_cancel(
    State(state): State<GatewayState>,
    headers: HeaderMap,
    Json(body): Json<VerificationTarget>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    authorize_matrix_http(&state, &headers)?;
    let m = matrix_or_404(&state)?;
    let user_id = parse_user_id(body.user_id.trim())?;
    let uid_str = user_id.as_str();
    let fid = body.flow_id.trim();

    if let Some(req) = m
        .client()
        .encryption()
        .get_verification_request(&user_id, fid)
        .await
    {
        req.cancel().await.map_err(|e| {
            log::warn!("matrix verification cancel (request) failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        m.remove_pending_verification(uid_str, fid);
        return Ok(Json(json!({ "ok": true, "phase": "request" })));
    }

    if let Some(v) = m.client().encryption().get_verification(&user_id, fid).await {
        if let Some(sas) = v.sas() {
            sas.cancel().await.map_err(|e| {
                log::warn!("matrix verification cancel (sas) failed: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
            m.remove_pending_verification(uid_str, fid);
            return Ok(Json(json!({ "ok": true, "phase": "sas" })));
        }
    }

    Err(StatusCode::NOT_FOUND)
}
