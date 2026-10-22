use actix_web::web;
use chrono::Utc;
use deadpool_redis::Pool;
use redis::AsyncCommands;
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

use super::models::*;

const REQUEST_QUEUE_INDEX_KEY: &str = "matchmaking:request_queue";
const ELO_RANGE_INCREMENT_PER_5_SECONDS: u32 = 25;
const INITIAL_ELO_RANGE: u32 = 50;
const MAX_ELO_RANGE: u32 = 300;
const DEFAULT_ESTIMATED_WAIT_TIME: Duration = Duration::from_secs(60);
/// How long a matched request id keeps resolving to its match id via `/status`.
const MATCHED_REQUEST_TTL_SECS: u64 = 600;
/// Keep match data available across backend restarts and deployments.
const ACTIVE_MATCH_TTL_SECS: u64 = 24 * 60 * 60;

fn matched_request_key(request_id: Uuid) -> String {
    format!("matchmaking:matched:{}", request_id)
}

fn active_match_key(match_id: Uuid) -> String {
    format!("matchmaking:match:{}", match_id)
}

#[derive(Clone)]
pub struct MatchmakingService {
    redis_pool: Pool,
}

impl MatchmakingService {
    pub fn new(redis_pool: Pool) -> Self {
        Self {
            redis_pool,
        }
    }

    async fn get_redis_connection(&self) -> Result<deadpool_redis::Connection, String> {
        self.redis_pool
            .get()
            .await
            .map_err(|e| format!("Redis connection failed: {}", e))
    }

    // Helper method to verify Soroban escrow signature
    async fn verify_escrow_signature(
        &self,
        signature: &str,
        wallet_address: &str,
        token: &str,
        amount: u64,
    ) -> Result<bool, String> {
        // In a real implementation, this would call Soroban RPC to verify the on-chain deposit
        // For now, we'll implement basic validation - in production, this would be a proper verification
        if signature.is_empty() {
            return Ok(false);
        }

        // Mock verification - in production, replace with actual Soroban call
        tracing::info!(
            "Verifying escrow signature: {} for wallet: {} token: {} amount: {}",
            signature,
            wallet_address,
            token,
            amount
        );

        // For testing purposes, any non-empty signature is considered valid
        Ok(true)
    }

    async fn find_match_for_queue(
        &self,
        request: &MatchRequest,
    ) -> Result<Option<MatchmakingResponse>, String> {
        match &request.queue_type {
            QueueType::RatedFree => self.find_rated_free_match(request).await,
            QueueType::CasualUnrated => self.find_casual_unrated_match(request).await,
            QueueType::RatedStaked { token, amount } => {
                self.find_rated_staked_match(request, token, amount).await
            }
            QueueType::Private => {
                Ok(None) // Private matches are handled separately
            }
        }
    }

    pub async fn join_queue(&self, request: MatchRequest) -> Result<MatchmakingResponse, String> {
        let request_id = request.id;

        // Validate staked queue requirements
        if let QueueType::RatedStaked { token, amount } = &request.queue_type {
            // Verify we have stake info and escrow signature
            let stake_info = request
                .stake_info
                .as_ref()
                .ok_or_else(|| "Missing stake information for staked queue".to_string())?;

            if stake_info.token != *token || stake_info.amount != *amount {
                return Err("Stake info mismatch with queue type".to_string());
            }

            let signature = stake_info
                .escrow_signature
                .as_ref()
                .ok_or_else(|| "Missing escrow signature for staked queue".to_string())?;

            // Verify the escrow signature on-chain
            let is_valid = self
                .verify_escrow_signature(signature, &request.player.wallet_address, token, *amount)
                .await?;

            if !is_valid {
                return Err("Invalid or unverified escrow deposit".to_string());
            }
        }

        match &request.queue_type {
            QueueType::Private => {
                if let Some(invite_address) = &request.invite_address {
                    self.add_private_invite(invite_address, &request).await?;
                    return Ok(MatchmakingResponse {
                        status: "Waiting for invited player".to_string(),
                        match_id: None,
                        request_id,
                    });
                } else {
                    return Ok(MatchmakingResponse {
                        status: "Invalid private match request: missing invite address".to_string(),
                        match_id: None,
                        request_id,
                    });
                }
            }
            _ => {
                if let Some(match_result) = self.find_match_for_queue(&request).await? {
                    return Ok(match_result);
                }
                self.add_to_redis_queue(&request).await?;
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
        let key = request.queue_type.redis_key();
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

        conn.hset::<_, _, _, ()>(
            REQUEST_QUEUE_INDEX_KEY,
            request.id.to_string(),
            &key,
        )
        .await
        .map_err(|e| format!("Redis HSET request queue index failed: {}", e))?;
        conn.expire::<_, ()>(REQUEST_QUEUE_INDEX_KEY, 3600)
            .await
            .map_err(|e| format!("Redis EXPIRE request queue index failed: {}", e))?;

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
        conn.hset::<_, _, _, ()>(
            REQUEST_QUEUE_INDEX_KEY,
            request.id.to_string(),
            key,
        )
        .await
        .map_err(|e| format!("Redis HSET request queue index failed: {}", e))?;
        conn.expire::<_, ()>(REQUEST_QUEUE_INDEX_KEY, 3600)
            .await
            .map_err(|e| format!("Redis EXPIRE request queue index failed: {}", e))?;

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
            local request_index = KEYS[2]
            local target_request_id = ARGV[1]
            
            local invites = redis.call('HGETALL', key)
            
            for i = 1, #invites, 2 do
                local invite_address = invites[i]
                local invite_json = invites[i + 1]
                local invite = cjson.decode(invite_json)
                
                if invite.id == target_request_id then
                    redis.call('HDEL', key, invite_address)
                    redis.call('HDEL', request_index, target_request_id)
                    return invite_json
                end
            end
            
            return nil
        "#;

        let result: Option<String> = redis::Script::new(lua_script)
            .key(key)
            .key(REQUEST_QUEUE_INDEX_KEY)
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
                    queue_type: QueueType::Private,
                    created_at: Utc::now(),
                    stake_info: None,
                    time_control: invite_request.time_control.clone(),
                };

                self.remember_match(&mut conn, &[inviter_request_id], match_id)
                    .await?;
                self.store_match(&mut conn, &new_match).await?;

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

        let indexed_queue: Option<String> = conn
            .hget(REQUEST_QUEUE_INDEX_KEY, request_id.to_string())
            .await
            .map_err(|e| format!("Redis HGET request queue index failed: {}", e))?;
        if let Some(queue_key) = indexed_queue {
            if queue_key == QueueType::Private.redis_key() {
                let removed = self.remove_private_invite(&mut conn, request_id).await?;
                if !removed {
                    conn.hdel::<_, _, ()>(REQUEST_QUEUE_INDEX_KEY, request_id.to_string())
                        .await
                        .map_err(|e| format!("Redis HDEL request queue index failed: {}", e))?;
                }
                return Ok(removed);
            }
            return self
                .remove_from_queue(&mut conn, &queue_key, request_id)
                .await;
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
                    conn.hdel::<_, _, ()>(REQUEST_QUEUE_INDEX_KEY, request_id.to_string())
                        .await
                        .map_err(|e| format!("Redis HDEL request queue index failed: {}", e))?;
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    async fn remove_private_invite(
        &self,
        conn: &mut deadpool_redis::Connection,
        request_id: Uuid,
    ) -> Result<bool, String> {
        let lua_script = r#"
            local invites = redis.call('HGETALL', KEYS[1])
            for i = 1, #invites, 2 do
                local request = cjson.decode(invites[i + 1])
                if request.id == ARGV[1] then
                    redis.call('HDEL', KEYS[1], invites[i])
                    redis.call('HDEL', KEYS[2], ARGV[1])
                    return 1
                end
            end
            return 0
        "#;
        let removed: i32 = redis::Script::new(lua_script)
            .key("matchmaking:invites")
            .key(REQUEST_QUEUE_INDEX_KEY)
            .arg(request_id.to_string())
            .invoke_async(conn)
            .await
            .map_err(|e| format!("Redis Lua script failed: {}", e))?;
        Ok(removed == 1)
    }

    async fn remove_from_queue(
        &self,
        conn: &mut deadpool_redis::Connection,
        key: &str,
        request_id: Uuid,
    ) -> Result<bool, String> {
        let lua_script = r#"
            local members = redis.call('ZRANGE', KEYS[1], 0, -1)
            for _, member in ipairs(members) do
                local request = cjson.decode(member)
                if request.id == ARGV[1] then
                    redis.call('ZREM', KEYS[1], member)
                    redis.call('HDEL', KEYS[2], ARGV[1])
                    return 1
                end
            end
            return 0
        "#;
        let removed: i32 = redis::Script::new(lua_script)
            .key(key)
            .key(REQUEST_QUEUE_INDEX_KEY)
            .arg(request_id.to_string())
            .invoke_async(conn)
            .await
            .map_err(|e| format!("Redis Lua script failed: {}", e))?;
        Ok(removed == 1)
    }

    /// Records which match each request ended up in, so a player who was
    /// already waiting in the queue can discover it by polling `/status`.
    async fn remember_match(
        &self,
        conn: &mut deadpool_redis::Connection,
        request_ids: &[Uuid],
        match_id: Uuid,
    ) -> Result<(), String> {
        for request_id in request_ids {
            conn.set_ex::<_, _, ()>(
                matched_request_key(*request_id),
                match_id.to_string(),
                MATCHED_REQUEST_TTL_SECS,
            )
            .await
            .map_err(|e| format!("Redis SETEX failed: {}", e))?;
        }
        Ok(())
    }

    async fn store_match(
        &self,
        conn: &mut deadpool_redis::Connection,
        game_match: &Match,
    ) -> Result<(), String> {
        let value = serde_json::to_string(game_match)
            .map_err(|e| format!("Match serialization error: {}", e))?;
        conn.set_ex::<_, _, ()>(
            active_match_key(game_match.id),
            value,
            ACTIVE_MATCH_TTL_SECS,
        )
        .await
        .map_err(|e| format!("Redis SETEX active match failed: {}", e))
    }

    /// The match a request was paired into, if it has been matched.
    pub async fn get_matched_id(&self, request_id: Uuid) -> Result<Option<Uuid>, String> {
        let mut conn = self.get_redis_connection().await?;
        let value: Option<String> = conn
            .get(matched_request_key(request_id))
            .await
            .map_err(|e| format!("Redis GET failed: {}", e))?;
        Ok(value.and_then(|v| Uuid::parse_str(&v).ok()))
    }

    pub async fn get_queue_status(&self, request_id: Uuid) -> Result<Option<QueueStatus>, String> {
        let mut conn = self.get_redis_connection().await?;

        // Check rated free queue
        if let Some(status) = self
            .get_status_from_queue(
                &mut conn,
                &QueueType::RatedFree.redis_key(),
                request_id,
                &QueueType::RatedFree,
            )
            .await?
        {
            return Ok(Some(status));
        }

        // Check casual unrated queue
        if let Some(status) = self
            .get_status_from_queue(
                &mut conn,
                &QueueType::CasualUnrated.redis_key(),
                request_id,
                &QueueType::CasualUnrated,
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
                        queue_type: QueueType::Private,
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
        queue_type: &QueueType,
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
                        estimated_wait_time: self.estimate_wait_time(index, queue_type),
                        queue_type: queue_type.clone(),
                    }));
                }
            }
        }

        Ok(None)
    }

    async fn find_rated_free_match(
        &self,
        request: &MatchRequest,
    ) -> Result<Option<MatchmakingResponse>, String> {
        let mut conn = self.get_redis_connection().await?;
        let key = QueueType::RatedFree.redis_key();
        let player_elo = request.player.elo;

        // Calculate expanding search window based on wait time
        let wait_seconds = Utc::now()
            .signed_duration_since(request.player.join_time)
            .num_seconds()
            .max(0) as u32;
        let expansion_steps = wait_seconds / 5;
        let search_range = (INITIAL_ELO_RANGE
            + expansion_steps * ELO_RANGE_INCREMENT_PER_5_SECONDS)
            .min(MAX_ELO_RANGE);

        // Lua script for atomic find-and-remove operation with expanding range
        // This prevents race conditions where two players try to match with the same opponent
        let lua_script = r#"
            local key = KEYS[1]
            local request_index = KEYS[2]
            local player_elo = tonumber(ARGV[1])
            local search_range = tonumber(ARGV[2])
            
            local members = redis.call('ZRANGE', key, 0, -1)
            
            for i, member in ipairs(members) do
                local opponent = cjson.decode(member)
                if opponent.queue_type != "RatedFree" then
                    goto continue
                end
                local elo_diff = math.abs(opponent.player.elo - player_elo)
                
                if elo_diff <= search_range then
                    redis.call('ZREM', key, member)
                    redis.call('HDEL', request_index, opponent.id)
                    return member
                end
                ::continue::
            end
            
            return nil
        "#;

        let result: Option<String> = redis::Script::new(lua_script)
            .key(key)
            .key(REQUEST_QUEUE_INDEX_KEY)
            .arg(player_elo)
            .arg(search_range)
            .invoke_async(&mut conn)
            .await
            .map_err(|e| format!("Redis Lua script failed: {}", e))?;

        if let Some(opponent_json) = result {
            if let Ok(opponent_request) = MatchRequest::from_redis_value(&opponent_json) {
                let match_id = Uuid::new_v4();
                let new_match = Match {
                    id: match_id,
                    player1: opponent_request.player,
                    player2: request.player.clone(),
                    queue_type: QueueType::RatedFree,
                    created_at: Utc::now(),
                    stake_info: None,
                    time_control: request.time_control.clone(),
                };

                self.remember_match(&mut conn, &[opponent_request.id, request.id], match_id)
                    .await?;
                self.store_match(&mut conn, &new_match).await?;

                return Ok(Some(MatchmakingResponse {
                    status: "Match found".to_string(),
                    match_id: Some(match_id),
                    request_id: request.id,
                }));
            }
        }

        Ok(None)
    }

    async fn find_casual_unrated_match(
        &self,
        request: &MatchRequest,
    ) -> Result<Option<MatchmakingResponse>, String> {
        let mut conn = self.get_redis_connection().await?;
        let key = QueueType::CasualUnrated.redis_key();

        // Remove the queue member and its cancellation index atomically.
        let lua_script = r#"
            local members = redis.call('ZRANGE', KEYS[1], 0, 0)
            if #members == 0 then
                return nil
            end
            local member = members[1]
            local request = cjson.decode(member)
            redis.call('ZREM', KEYS[1], member)
            redis.call('HDEL', KEYS[2], request.id)
            return member
        "#;
        let result: Option<String> = redis::Script::new(lua_script)
            .key(&key)
            .key(REQUEST_QUEUE_INDEX_KEY)
            .invoke_async(&mut conn)
            .await
            .map_err(|e| format!("Redis Lua script failed: {}", e))?;

        if let Some(member) = result {
            if let Ok(opponent_request) = MatchRequest::from_redis_value(&member) {
                // Ensure we only match with other CasualUnrated players
                if !matches!(opponent_request.queue_type, QueueType::CasualUnrated) {
                    // Put the player back in the queue if they're not the right type
                    let now = Utc::now();
                    let score = now.timestamp() as f64;
                    conn.zadd::<_, _, _, ()>(&key, &member, score)
                        .await
                        .map_err(|e| format!("Redis ZADD failed: {}", e))?;
                    conn.hset::<_, _, _, ()>(
                        REQUEST_QUEUE_INDEX_KEY,
                        opponent_request.id.to_string(),
                        &key,
                    )
                    .await
                    .map_err(|e| format!("Redis HSET request queue index failed: {}", e))?;
                    return Ok(None);
                }

                let match_id = Uuid::new_v4();
                let new_match = Match {
                    id: match_id,
                    player1: opponent_request.player,
                    player2: request.player.clone(),
                    queue_type: QueueType::CasualUnrated,
                    created_at: Utc::now(),
                    stake_info: None,
                    time_control: request.time_control.clone(),
                };

                self.remember_match(&mut conn, &[opponent_request.id, request.id], match_id)
                    .await?;
                self.store_match(&mut conn, &new_match).await?;

                return Ok(Some(MatchmakingResponse {
                    status: "Match found".to_string(),
                    match_id: Some(match_id),
                    request_id: request.id,
                }));
            }
        }

        Ok(None)
    }

    async fn find_rated_staked_match(
        &self,
        request: &MatchRequest,
        token: &str,
        amount: &u64,
    ) -> Result<Option<MatchmakingResponse>, String> {
        let mut conn = self.get_redis_connection().await?;
        let queue_type = QueueType::RatedStaked {
            token: token.to_string(),
            amount: *amount,
        };
        let key = queue_type.redis_key();
        let player_elo = request.player.elo;

        // Calculate expanding search window based on wait time
        let wait_seconds = Utc::now()
            .signed_duration_since(request.player.join_time)
            .num_seconds()
            .max(0) as u32;
        let expansion_steps = wait_seconds / 5;
        let search_range = (INITIAL_ELO_RANGE
            + expansion_steps * ELO_RANGE_INCREMENT_PER_5_SECONDS)
            .min(MAX_ELO_RANGE);

        // Lua script for atomic find-and-remove operation with expanding range
        // This prevents race conditions where two players try to match with the same opponent
        let lua_script = r#"
            local key = KEYS[1]
            local request_index = KEYS[2]
            local player_elo = tonumber(ARGV[1])
            local search_range = tonumber(ARGV[2])
            local expected_token = ARGV[3]
            local expected_amount = tonumber(ARGV[4])
            
            local members = redis.call('ZRANGE', key, 0, -1)
            
            for i, member in ipairs(members) do
                local opponent = cjson.decode(member)
                -- Only match with players in the same staked queue (same token and amount)
                if opponent.queue_type.RatedStaked 
                    and opponent.queue_type.RatedStaked.token == expected_token
                    and opponent.queue_type.RatedStaked.amount == expected_amount then
                    
                    -- Verify both players have valid escrow signatures
                    if opponent.stake_info 
                        and opponent.stake_info.escrow_signature ~= nil then
                        
                        local elo_diff = math.abs(opponent.player.elo - player_elo)
                        if elo_diff <= search_range then
                            redis.call('ZREM', key, member)
                            redis.call('HDEL', request_index, opponent.id)
                            return member
                        end
                    end
                end
            end
            
            return nil
        "#;

        let result: Option<String> = redis::Script::new(lua_script)
            .key(&key)
            .key(REQUEST_QUEUE_INDEX_KEY)
            .arg(player_elo)
            .arg(search_range)
            .arg(token)
            .arg(amount)
            .invoke_async(&mut conn)
            .await
            .map_err(|e| format!("Redis Lua script failed: {}", e))?;

        if let Some(opponent_json) = result {
            if let Ok(opponent_request) = MatchRequest::from_redis_value(&opponent_json) {
                // Double-check that both players have valid escrow signatures
                let opponent_stake = opponent_request
                    .stake_info
                    .as_ref()
                    .ok_or_else(|| "Opponent missing stake info".to_string())?;
                let player_stake = request
                    .stake_info
                    .as_ref()
                    .ok_or_else(|| "Player missing stake info".to_string())?;

                if opponent_stake.escrow_signature.is_none()
                    || player_stake.escrow_signature.is_none()
                {
                    // Put the player back if one doesn't have a valid signature
                    let now = Utc::now();
                    let score = now.timestamp() as f64;
                    conn.zadd::<_, _, _, ()>(&key, &opponent_json, score)
                        .await
                        .map_err(|e| format!("Redis ZADD failed: {}", e))?;
                    conn.hset::<_, _, _, ()>(
                        REQUEST_QUEUE_INDEX_KEY,
                        opponent_request.id.to_string(),
                        &key,
                    )
                    .await
                    .map_err(|e| format!("Redis HSET request queue index failed: {}", e))?;
                    return Err("Both players must have verified escrow deposits".to_string());
                }

                let match_id = Uuid::new_v4();
                let new_match = Match {
                    id: match_id,
                    player1: opponent_request.player,
                    player2: request.player.clone(),
                    queue_type: QueueType::RatedStaked {
                        token: token.to_string(),
                        amount: *amount,
                    },
                    created_at: Utc::now(),
                    stake_info: Some(StakeInfo {
                        token: token.to_string(),
                        amount: *amount,
                        escrow_signature: None, // Match stores the combined stake info
                    }),
                    time_control: request.time_control.clone(),
                };

                self.remember_match(&mut conn, &[opponent_request.id, request.id], match_id)
                    .await?;
                self.store_match(&mut conn, &new_match).await?;

                return Ok(Some(MatchmakingResponse {
                    status: "Match found".to_string(),
                    match_id: Some(match_id),
                    request_id: request.id,
                }));
            }
        }

        Ok(None)
    }

    fn estimate_wait_time(&self, position: usize, queue_type: &QueueType) -> Duration {
        match queue_type {
            QueueType::RatedFree => Duration::from_secs((30 + position as u64 * 15).min(300)),
            QueueType::CasualUnrated => Duration::from_secs((15 + position as u64 * 10).min(180)),
            QueueType::RatedStaked { .. } => {
                Duration::from_secs((45 + position as u64 * 20).min(400))
            } // Staked matches might have longer wait times
            QueueType::Private => DEFAULT_ESTIMATED_WAIT_TIME,
        }
    }

    pub async fn expand_elo_ranges(&self) -> Result<(), String> {
        let mut conn = self.get_redis_connection().await?;
        let now = Utc::now();

        // Expand ranges for rated free queue
        let rated_free_key = QueueType::RatedFree.redis_key();
        self.expand_elo_ranges_for_queue(&mut conn, &rated_free_key, now)
            .await?;

        // Also expand ranges for all staked queues (in a real implementation, you'd track active staked queues)
        // For simplicity, we'll assume we know the common ones - in production, track them separately

        Ok(())
    }

    async fn expand_elo_ranges_for_queue(
        &self,
        conn: &mut deadpool_redis::Connection,
        key: &str,
        now: chrono::DateTime<Utc>,
    ) -> Result<(), String> {
        let members: Vec<(String, f64)> = conn
            .zrange_withscores(key, 0, -1)
            .await
            .map_err(|e| format!("Redis ZRANGE failed for key {}: {}", key, e))?;

        for (member, score) in members {
            if let Ok(mut request) = MatchRequest::from_redis_value(&member) {
                // Only expand elo ranges for rated queues (both free and staked)
                if !matches!(
                    request.queue_type,
                    QueueType::RatedFree | QueueType::RatedStaked { .. }
                ) {
                    continue;
                }

                let wait_time = now.signed_duration_since(request.player.join_time);
                let wait_seconds = wait_time.num_seconds().max(0) as u32;
                let expansion_steps = wait_seconds / 5;
                let new_range = (INITIAL_ELO_RANGE
                    + expansion_steps * ELO_RANGE_INCREMENT_PER_5_SECONDS)
                    .min(MAX_ELO_RANGE);

                request.max_elo_diff = Some(new_range);

                let updated_value = request
                    .to_redis_value()
                    .map_err(|e| format!("Serialization error: {}", e))?;

                conn.zrem::<_, _, ()>(key, &member)
                    .await
                    .map_err(|e| format!("Redis ZREM failed for key {}: {}", key, e))?;

                conn.zadd::<_, _, _, ()>(key, &updated_value, score)
                    .await
                    .map_err(|e| format!("Redis ZADD failed for key {}: {}", key, e))?;
            }
        }

        Ok(())
    }

    pub async fn get_match(&self, match_id: Uuid) -> Result<Option<Match>, String> {
        let mut conn = self.get_redis_connection().await?;
        let value: Option<String> = conn
            .get(active_match_key(match_id))
            .await
            .map_err(|e| format!("Redis GET active match failed: {}", e))?;
        value
            .map(|json| {
                serde_json::from_str(&json)
                    .map_err(|e| format!("Match deserialization error: {}", e))
            })
            .transpose()
    }
}

pub fn get_matchmaking_service(redis_pool: Pool) -> web::Data<MatchmakingService> {
    web::Data::new(MatchmakingService::new(redis_pool))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[actix_web::test]
    async fn cancelling_a_staked_request_removes_it_from_its_queue() {
        let redis_url = std::env::var("TEST_REDIS_URL")
            .or_else(|_| std::env::var("REDIS_URL"))
            .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
        let pool = crate::redis::create_redis_pool(&redis_url)
            .expect("valid Redis URL is required for matchmaking tests");
        if pool.get().await.is_err() {
            eprintln!("Skipping staked cancellation integration test: Redis is unavailable");
            return;
        }

        let service = MatchmakingService::new(pool);
        let request_id = Uuid::new_v4();
        let token = format!("TEST-{}", Uuid::new_v4());
        let queue_type = QueueType::RatedStaked {
            token: token.clone(),
            amount: 1,
        };
        let request = MatchRequest {
            id: request_id,
            player: Player {
                wallet_address: "GTEST".to_string(),
                elo: 1200,
                join_time: Utc::now(),
            },
            queue_type: queue_type.clone(),
            invite_address: None,
            max_elo_diff: None,
            stake_info: Some(StakeInfo {
                token,
                amount: 1,
                escrow_signature: Some("test-signature".to_string()),
            }),
            time_control: TimeControl::default(),
        };

        service
            .add_to_redis_queue(&request)
            .await
            .expect("staked request should be enqueued");
        assert!(service
            .cancel_request(request_id)
            .await
            .expect("cancellation should succeed"));

        let mut conn = service
            .get_redis_connection()
            .await
            .expect("Redis connection should remain available");
        let queued_count: usize = conn
            .zcard(queue_type.redis_key())
            .await
            .expect("queue should be readable");
        let indexed_queue: Option<String> = conn
            .hget(REQUEST_QUEUE_INDEX_KEY, request_id.to_string())
            .await
            .expect("request index should be readable");
        assert_eq!(queued_count, 0);
        assert_eq!(indexed_queue, None);
    }
}
