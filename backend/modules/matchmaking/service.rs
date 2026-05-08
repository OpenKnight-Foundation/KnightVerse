use actix_web::web;
use chrono::Utc;
use deadpool_redis::Pool;
use redis::AsyncCommands;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use uuid::Uuid;

use super::models::*;

const ELO_RANGE_INCREMENT_PER_MINUTE: u32 = 50;
const DEFAULT_MAX_ELO_DIFF: u32 = 200;
const DEFAULT_ESTIMATED_WAIT_TIME: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub struct MatchmakingService {
    redis_pool: Pool,
    active_matches: Arc<Mutex<HashMap<Uuid, Match>>>,
}

impl MatchmakingService {
    pub fn new(redis_pool: Pool) -> Self {
        Self {
            redis_pool,
            active_matches: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn get_redis_connection(
        &self,
    ) -> Result<deadpool_redis::Connection, String> {
        self.redis_pool
            .get()
            .await
            .map_err(|e| format!("Redis connection failed: {}", e))
    }

    pub async fn join_queue(
        &self,
        request: MatchRequest,
    ) -> Result<MatchmakingResponse, String> {
        let request_id = request.id;

        match request.match_type {
            MatchType::Rated => {
                if let Some(match_result) = self.find_rated_match(&request).await? {
                    return Ok(match_result);
                }
                self.add_to_redis_queue(&request).await?;
            }
            MatchType::Casual => {
                if let Some(match_result) = self.find_casual_match(&request).await? {
                    return Ok(match_result);
                }
                self.add_to_redis_queue(&request).await?;
            }
            MatchType::Private => {
                if let Some(invite_address) = &request.invite_address {
                    self.add_private_invite(invite_address, &request).await?;
                    return Ok(MatchmakingResponse {
                        status: "Waiting for invited player".to_string(),
                        match_id: None,
                        request_id,
                    });
                } else {
                    return Ok(MatchmakingResponse {
                        status: "Invalid private match request: missing invite address"
                            .to_string(),
                        match_id: None,
                        request_id,
                    });
                }
            }
        }

        Ok(MatchmakingResponse {
            status: "Added to queue".to_string(),
            match_id: None,
            request_id,
        })
    }

    async fn add_to_redis_queue(&self, request: &MatchRequest) -> Result<(), String> {
        let mut conn = self.get_redis_connection().await?;
        let key = request.match_type.redis_key();
        let now = Utc::now();
        let score = now.timestamp() as f64;
        let value = request
            .to_redis_value()
            .map_err(|e| format!("Serialization error: {}", e))?;

        let cutoff = (now - chrono::Duration::hours(1)).timestamp() as f64;
        conn.zrembyscore::<_, _, _, ()>(&key, f64::NEG_INFINITY, cutoff)
            .await
            .map_err(|e| format!("Redis ZREMRANGEBYSCORE failed: {}", e))?;

        conn.zadd::<_, _, _, ()>(&key, &value, score)
            .await
            .map_err(|e| format!("Redis ZADD failed: {}", e))?;

        conn.expire::<_, ()>(&key, 3600)
            .await
            .map_err(|e| format!("Redis EXPIRE failed: {}", e))?;

        Ok(())
    }


    async fn add_private_invite(
        &self,
        invite_address: &str,
        request: &MatchRequest,
    ) -> Result<(), String> {
        let mut conn = self.get_redis_connection().await?;
        let key = "matchmaking:invites";
        let value = request
            .to_redis_value()
            .map_err(|e| format!("Serialization error: {}", e))?;

        conn.hset::<_, _, _, ()>(key, invite_address, &value)
            .await
            .map_err(|e| format!("Redis HSET failed: {}", e))?;

        Ok(())
    }

    pub async fn check_private_invite(
        &self,
        wallet_address: &str,
    ) -> Result<Option<MatchRequest>, String> {
        let mut conn = self.get_redis_connection().await?;
        let key = "matchmaking:invites";

        let value: Option<String> = conn
            .hget(key, wallet_address)
            .await
            .map_err(|e| format!("Redis HGET failed: {}", e))?;

        match value {
            Some(json) => MatchRequest::from_redis_value(&json)
                .map(Some)
                .map_err(|e| format!("Deserialization error: {}", e)),
            None => Ok(None),
        }
    }

    pub async fn accept_private_invite(
        &self,
        inviter_request_id: Uuid,
        accepting_player: Player,
    ) -> Result<Option<MatchmakingResponse>, String> {
        let mut conn = self.get_redis_connection().await?;
        let key = "matchmaking:invites";

        // Lua script for atomic find-and-remove operation
        // This prevents race conditions where multiple players try to accept the same invite
        let lua_script = r#"
            local key = KEYS[1]
            local target_request_id = ARGV[1]
            
            local invites = redis.call('HGETALL', key)
            
            for i = 1, #invites, 2 do
                local invite_address = invites[i]
                local invite_json = invites[i + 1]
                local invite = cjson.decode(invite_json)
                
                if invite.id == target_request_id then
                    redis.call('HDEL', key, invite_address)
                    return invite_json
                end
            end
            
            return nil
        "#;

        let result: Option<String> = redis::Script::new(lua_script)
            .key(key)
            .arg(inviter_request_id.to_string())
            .invoke_async(&mut conn)
            .await
            .map_err(|e| format!("Redis Lua script failed: {}", e))?;

        if let Some(invite_json) = result {
            if let Ok(invite_request) = MatchRequest::from_redis_value(&invite_json) {
                // Create match
                let match_id = Uuid::new_v4();
                let new_match = Match {
                    id: match_id,
                    player1: invite_request.player,
                    player2: accepting_player,
                    match_type: MatchType::Private,
                    created_at: Utc::now(),
                };

                let mut active_matches = self.active_matches.lock().unwrap();
                active_matches.insert(match_id, new_match);

                return Ok(Some(MatchmakingResponse {
                    status: "Match created".to_string(),
                    match_id: Some(match_id),
                    request_id: inviter_request_id,
                }));
            }
        }

        Ok(None)

    }

    pub async fn cancel_request(&self, request_id: Uuid) -> Result<bool, String> {
        let mut conn = self.get_redis_connection().await?;

        // Try to remove from rated queue
        if self
            .remove_from_queue(&mut conn, "matchmaking:queue:rated", request_id)
            .await?
        {
            return Ok(true);
        }

        // Try to remove from casual queue
        if self
            .remove_from_queue(&mut conn, "matchmaking:queue:casual", request_id)
            .await?
        {
            return Ok(true);
        }

        // Try to remove from private invites
        let invites: HashMap<String, String> = conn
            .hgetall("matchmaking:invites")
            .await
            .map_err(|e| format!("Redis HGETALL failed: {}", e))?;

        for (invite_address, json) in invites {
            if let Ok(request) = MatchRequest::from_redis_value(&json) {
                if request.id == request_id {
                    conn.hdel::<_, _, ()>("matchmaking:invites", &invite_address)
                        .await
                        .map_err(|e| format!("Redis HDEL failed: {}", e))?;
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    async fn remove_from_queue(
        &self,
        conn: &mut deadpool_redis::Connection,
        key: &str,
        request_id: Uuid,
    ) -> Result<bool, String> {
        let members: Vec<String> = conn
            .zrange(key, 0, -1)
            .await
            .map_err(|e| format!("Redis ZRANGE failed: {}", e))?;

        for member in members {
            if let Ok(request) = MatchRequest::from_redis_value(&member) {
                if request.id == request_id {
                    conn.zrem::<_, _, ()>(key, &member)
                        .await
                        .map_err(|e| format!("Redis ZREM failed: {}", e))?;
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    pub async fn get_queue_status(
        &self,
        request_id: Uuid,
    ) -> Result<Option<QueueStatus>, String> {
        let mut conn = self.get_redis_connection().await?;

        // Check rated queue
        if let Some(status) = self
            .get_status_from_queue(
                &mut conn,
                "matchmaking:queue:rated",
                request_id,
                MatchType::Rated,
            )
            .await?
        {
            return Ok(Some(status));
        }

        // Check casual queue
        if let Some(status) = self
            .get_status_from_queue(
                &mut conn,
                "matchmaking:queue:casual",
                request_id,
                MatchType::Casual,
            )
            .await?
        {
            return Ok(Some(status));
        }

        // Check private invites
        let invites: HashMap<String, String> = conn
            .hgetall("matchmaking:invites")
            .await
            .map_err(|e| format!("Redis HGETALL failed: {}", e))?;

        for (_, json) in invites {
            if let Ok(request) = MatchRequest::from_redis_value(&json) {
                if request.id == request_id {
                    return Ok(Some(QueueStatus {
                        request_id,
                        position: 1,
                        estimated_wait_time: DEFAULT_ESTIMATED_WAIT_TIME,
                        match_type: MatchType::Private,
                    }));
                }
            }
        }

        Ok(None)
    }

    async fn get_status_from_queue(
        &self,
        conn: &mut deadpool_redis::Connection,
        key: &str,
        request_id: Uuid,
        match_type: MatchType,
    ) -> Result<Option<QueueStatus>, String> {
        let members: Vec<String> = conn
            .zrange(key, 0, -1)
            .await
            .map_err(|e| format!("Redis ZRANGE failed: {}", e))?;

        for (index, member) in members.iter().enumerate() {
            if let Ok(request) = MatchRequest::from_redis_value(member) {
                if request.id == request_id {
                    return Ok(Some(QueueStatus {
                        request_id,
                        position: index + 1,
                        estimated_wait_time: self.estimate_wait_time(index, &match_type),
                        match_type,
                    }));
                }
            }
        }

        Ok(None)
    }

    async fn find_rated_match(
        &self,
        request: &MatchRequest,
    ) -> Result<Option<MatchmakingResponse>, String> {
        let mut conn = self.get_redis_connection().await?;
        let key = "matchmaking:queue:rated";
        let player_elo = request.player.elo;
        let max_elo_diff = request.max_elo_diff.unwrap_or(DEFAULT_MAX_ELO_DIFF);

        // Lua script for atomic find-and-remove operation
        // This prevents race conditions where two players try to match with the same opponent
        let lua_script = r#"
            local key = KEYS[1]
            local player_elo = tonumber(ARGV[1])
            local max_elo_diff = tonumber(ARGV[2])
            
            local members = redis.call('ZRANGE', key, 0, -1)
            
            for i, member in ipairs(members) do
                local opponent = cjson.decode(member)
                local elo_diff = math.abs(opponent.player.elo - player_elo)
                
                if elo_diff <= max_elo_diff then
                    redis.call('ZREM', key, member)
                    return member
                end
            end
            
            return nil
        "#;

        let result: Option<String> = redis::Script::new(lua_script)
            .key(key)
            .arg(player_elo)
            .arg(max_elo_diff)
            .invoke_async(&mut conn)
            .await
            .map_err(|e| format!("Redis Lua script failed: {}", e))?;

        if let Some(opponent_json) = result {
            if let Ok(opponent_request) = MatchRequest::from_redis_value(&opponent_json) {
                // Create match
                let match_id = Uuid::new_v4();
                let new_match = Match {
                    id: match_id,
                    player1: opponent_request.player,
                    player2: request.player.clone(),
                    match_type: MatchType::Rated,
                    created_at: Utc::now(),
                };

                let mut active_matches = self.active_matches.lock().unwrap();
                active_matches.insert(match_id, new_match);

                return Ok(Some(MatchmakingResponse {
                    status: "Match found".to_string(),
                    match_id: Some(match_id),
                    request_id: request.id,
                }));
            }
        }

        Ok(None)
    }

    async fn find_casual_match(
        &self,
        request: &MatchRequest,
    ) -> Result<Option<MatchmakingResponse>, String> {
        let mut conn = self.get_redis_connection().await?;
        let key = "matchmaking:queue:casual";

        // Pop the oldest player from queue (FIFO)
        let result: Vec<(String, f64)> = conn
            .zpopmin(key, 1)
            .await
            .map_err(|e| format!("Redis ZPOPMIN failed: {}", e))?;
        
        let result = result.into_iter().next();

        if let Some((member, _score)) = result {
            if let Ok(opponent_request) = MatchRequest::from_redis_value(&member) {
                let match_id = Uuid::new_v4();
                let new_match = Match {
                    id: match_id,
                    player1: opponent_request.player,
                    player2: request.player.clone(),
                    match_type: MatchType::Casual,
                    created_at: Utc::now(),
                };

                let mut active_matches = self.active_matches.lock().unwrap();
                active_matches.insert(match_id, new_match);

                return Ok(Some(MatchmakingResponse {
                    status: "Match found".to_string(),
                    match_id: Some(match_id),
                    request_id: request.id,
                }));
            }
        }

        Ok(None)
    }

    fn estimate_wait_time(&self, position: usize, match_type: &MatchType) -> Duration {
        match match_type {
            MatchType::Rated => Duration::from_secs((30 + position as u64 * 15).min(300)),
            MatchType::Casual => Duration::from_secs((15 + position as u64 * 10).min(180)),
            MatchType::Private => DEFAULT_ESTIMATED_WAIT_TIME,
        }
    }

    pub async fn expand_elo_ranges(&self) -> Result<(), String> {
        let mut conn = self.get_redis_connection().await?;
        let key = "matchmaking:queue:rated";
        let now = Utc::now();

        let members: Vec<(String, f64)> = conn
            .zrange_withscores(key, 0, -1)
            .await
            .map_err(|e| format!("Redis ZRANGE failed: {}", e))?;

        for (member, score) in members {
            if let Ok(mut request) = MatchRequest::from_redis_value(&member) {
                let wait_time = now.signed_duration_since(request.player.join_time);
                let minutes_waiting = wait_time.num_minutes();

                if minutes_waiting > 0 {
                    let additional_range = minutes_waiting as u32 * ELO_RANGE_INCREMENT_PER_MINUTE;
                    request.max_elo_diff = Some(
                        request.max_elo_diff.unwrap_or(DEFAULT_MAX_ELO_DIFF) + additional_range,
                    );

                    // Update in Redis
                    let updated_value = request
                        .to_redis_value()
                        .map_err(|e| format!("Serialization error: {}", e))?;

                    // Remove old entry and add updated one
                    conn.zrem::<_, _, ()>(key, &member)
                        .await
                        .map_err(|e| format!("Redis ZREM failed: {}", e))?;

                    conn.zadd::<_, _, _, ()>(key, &updated_value, score)
                        .await
                        .map_err(|e| format!("Redis ZADD failed: {}", e))?;
                }
            }
        }

        Ok(())
    }

    pub fn get_match(&self, match_id: Uuid) -> Option<Match> {
        let active_matches = self.active_matches.lock().unwrap();
        active_matches.get(&match_id).cloned()
    }
}

pub fn get_matchmaking_service(redis_pool: Pool) -> web::Data<MatchmakingService> {
    web::Data::new(MatchmakingService::new(redis_pool))
}
