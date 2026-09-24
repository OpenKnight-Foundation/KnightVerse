use actix_web::{web, HttpResponse, Responder};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::error;
use uuid::Uuid;

use super::models::*;
use super::service::MatchmakingService;

#[derive(Debug, Deserialize)]
pub struct JoinCasualQueueRequest {
    pub wallet_address: String,
    pub elo: u32,
    pub invite_address: Option<String>,
    #[serde(default)]
    pub time_control: TimeControl,
}

#[derive(Debug, Deserialize)]
pub struct JoinRatedFreeQueueRequest {
    pub wallet_address: String,
    pub elo: u32,
    pub invite_address: Option<String>,
    pub max_elo_diff: Option<u32>,
    #[serde(default)]
    pub time_control: TimeControl,
}

#[derive(Debug, Deserialize)]
pub struct JoinRatedStakedQueueRequest {
    pub wallet_address: String,
    pub elo: u32,
    pub token: String,
    pub amount: u64,
    pub escrow_signature: String,
    pub invite_address: Option<String>,
    pub max_elo_diff: Option<u32>,
    #[serde(default)]
    pub time_control: TimeControl,
}

#[derive(Debug, Deserialize)]
pub struct AcceptInviteRequest {
    pub wallet_address: String,
    pub elo: u32,
    pub inviter_request_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct CancelRequest {
    pub request_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub status: String,
    pub queue_status: Option<QueueStatus>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub status: String,
    pub error: String,
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/matchmaking")
            .route("/join-casual", web::post().to(join_casual_queue))
            .route("/join-rated-free", web::post().to(join_rated_free_queue))
            .route("/join-rated-staked", web::post().to(join_rated_staked_queue))
            .route("/status/{request_id}", web::get().to(get_status))
            .route("/cancel", web::post().to(cancel_request))
            .route("/accept-invite", web::post().to(accept_invite))
            .route("/match/{match_id}", web::get().to(get_match)),
    );
}

async fn join_casual_queue(
    service: web::Data<MatchmakingService>,
    req: web::Json<JoinCasualQueueRequest>,
) -> impl Responder {
    let request_id = Uuid::new_v4();

    let player = Player {
        wallet_address: req.wallet_address.clone(),
        elo: req.elo,
        join_time: Utc::now(),
    };

    let match_request = MatchRequest {
        id: request_id,
        player,
        queue_type: QueueType::CasualUnrated,
        invite_address: req.invite_address.clone(),
        max_elo_diff: None,
        stake_info: None,
        time_control: req.time_control.clone(),
    };

    match service.join_queue(match_request).await {
        Ok(response) => HttpResponse::Ok().json(response),
        Err(e) => {
            error!("Failed to join casual queue: {}", e);
            HttpResponse::ServiceUnavailable().json(ErrorResponse {
                status: "error".to_string(),
                error: "internal_error".to_string(),
            })
        }
    }
}

async fn join_rated_free_queue(
    service: web::Data<MatchmakingService>,
    req: web::Json<JoinRatedFreeQueueRequest>,
) -> impl Responder {
    let request_id = Uuid::new_v4();

    let player = Player {
        wallet_address: req.wallet_address.clone(),
        elo: req.elo,
        join_time: Utc::now(),
    };

    let match_request = MatchRequest {
        id: request_id,
        player,
        queue_type: QueueType::RatedFree,
        invite_address: req.invite_address.clone(),
        max_elo_diff: req.max_elo_diff,
        stake_info: None,
        time_control: req.time_control.clone(),
    };

    match service.join_queue(match_request).await {
        Ok(response) => HttpResponse::Ok().json(response),
        Err(e) => {
            error!("Failed to join rated free queue: {}", e);
            HttpResponse::ServiceUnavailable().json(ErrorResponse {
                status: "error".to_string(),
                error: "internal_error".to_string(),
            })
        }
    }
}

async fn join_rated_staked_queue(
    service: web::Data<MatchmakingService>,
    req: web::Json<JoinRatedStakedQueueRequest>,
) -> impl Responder {
    let request_id = Uuid::new_v4();

    let player = Player {
        wallet_address: req.wallet_address.clone(),
        elo: req.elo,
        join_time: Utc::now(),
    };

    let stake_info = StakeInfo {
        token: req.token.clone(),
        amount: req.amount,
        escrow_signature: Some(req.escrow_signature.clone()),
    };

    let match_request = MatchRequest {
        id: request_id,
        player,
        queue_type: QueueType::RatedStaked { 
            token: req.token.clone(), 
            amount: req.amount 
        },
        invite_address: req.invite_address.clone(),
        max_elo_diff: req.max_elo_diff,
        stake_info: Some(stake_info),
        time_control: req.time_control.clone(),
    };

    match service.join_queue(match_request).await {
        Ok(response) => HttpResponse::Ok().json(response),
        Err(e) => {
            error!("Failed to join rated staked queue: {}", e);
            HttpResponse::ServiceUnavailable().json(ErrorResponse {
                status: "error".to_string(),
                error: "internal_error".to_string(),
            })
        }
    }
}

async fn get_status(
    service: web::Data<MatchmakingService>,
    path: web::Path<Uuid>,
) -> impl Responder {
    let request_id = path.into_inner();

    match service.get_queue_status(request_id).await {
        Ok(Some(status)) => HttpResponse::Ok().json(StatusResponse {
            status: "In queue".to_string(),
            queue_status: Some(status),
        }),
        Ok(None) => HttpResponse::NotFound().json(StatusResponse {
            status: "Request not found".to_string(),
            queue_status: None,
        }),
        Err(e) => {
            error!("Failed to get queue status: {}", e);
            HttpResponse::ServiceUnavailable().json(ErrorResponse {
                status: "error".to_string(),
                error: "internal_error".to_string(),
            })
        }
    }
}

async fn cancel_request(
    service: web::Data<MatchmakingService>,
    req: web::Json<CancelRequest>,
) -> impl Responder {
    match service.cancel_request(req.request_id).await {
        Ok(true) => HttpResponse::Ok().json(serde_json::json!({
            "status": "Request cancelled successfully"
        })),
        Ok(false) => HttpResponse::NotFound().json(serde_json::json!({
            "status": "Request not found"
        })),
        Err(e) => {
            error!("Failed to cancel request: {}", e);
            HttpResponse::ServiceUnavailable().json(ErrorResponse {
                status: "error".to_string(),
                error: "internal_error".to_string(),
            })
        }
    }
}

async fn accept_invite(
    service: web::Data<MatchmakingService>,
    req: web::Json<AcceptInviteRequest>,
) -> impl Responder {
    let player = Player {
        wallet_address: req.wallet_address.clone(),
        elo: req.elo,
        join_time: Utc::now(),
    };

    match service
        .accept_private_invite(req.inviter_request_id, player)
        .await
    {
        Ok(Some(response)) => HttpResponse::Ok().json(response),
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({
            "status": "Invite not found"
        })),
        Err(e) => {
            error!("Failed to accept invite: {}", e);
            HttpResponse::ServiceUnavailable().json(ErrorResponse {
                status: "error".to_string(),
                error: "internal_error".to_string(),
            })
        }
    }
}

async fn get_match(
    service: web::Data<MatchmakingService>,
    path: web::Path<Uuid>,
) -> impl Responder {
    let match_id = path.into_inner();

    if let Some(match_data) = service.get_match(match_id) {
        HttpResponse::Ok().json(match_data)
    } else {
        HttpResponse::NotFound().json(serde_json::json!({
            "status": "Match not found"
        }))
    }
}