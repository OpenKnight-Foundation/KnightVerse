use actix_web::{
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    error::{Error, ErrorUnauthorized},
    body::{BoxBody, MessageBody},
    HttpMessage,
};
use futures_util::future::{ok, LocalBoxFuture, Ready};
use std::task::{Context, Poll};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

/// JWT Claims structure containing user identification and expiration
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// Subject (user ID)
    pub sub: String,
    /// User ID as integer
    pub user_id: i32,
    /// Username
    pub username: String,
    /// Expiration time (Unix timestamp)
    pub exp: usize,
    /// Issued at time (Unix timestamp)
    pub iat: usize,
    /// JWT ID for reconnection tokens (optional)
    pub jti: Option<String>,
    /// Token type (access or reconnect)
    pub token_type: TokenType,
}

/// Token type enumeration
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TokenType {
    Access,
    Reconnect,
}

/// JWT Service for token generation and validation
#[derive(Clone, Debug)]
pub struct JwtService {
    pub secret_key: String,
    expiration_time: usize, // in seconds
    reconnect_expiration_time: usize, // in seconds (shorter for reconnect tokens)
}

impl JwtService {
    /// Create a new JWT service
    pub fn new(secret_key: String, expiration_time: usize) -> Self {
        JwtService {
            secret_key,
            expiration_time,
            reconnect_expiration_time: 30, // 30 seconds for reconnect tokens
        }
    }

    /// Generate a new JWT access token for a user
    pub fn generate_token(&self, user_id: i32, username: &str) -> Result<String, jsonwebtoken::errors::Error> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize;

        let claims = Claims {
            sub: user_id.to_string(),
            user_id,
            username: username.to_string(),
            exp: now + self.expiration_time,
            iat: now,
            jti: None,
            token_type: TokenType::Access,
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret_key.as_ref()),
        )?;

        Ok(token)
    }

    /// Generate a reconnection token for seamless WebSocket reconnection
    pub fn generate_reconnect_token(&self, user_id: i32, username: &str, session_id: &str) -> Result<String, jsonwebtoken::errors::Error> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize;

        let claims = Claims {
            sub: user_id.to_string(),
            user_id,
            username: username.to_string(),
            exp: now + self.reconnect_expiration_time,
            iat: now,
            jti: Some(session_id.to_string()),
            token_type: TokenType::Reconnect,
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.secret_key.as_ref()),
        )?;

        Ok(token)
    }

    /// Validate and decode a JWT token
    pub fn validate_token(&self, token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.secret_key.as_ref()),
            &Validation::new(Algorithm::HS256),
        )?;

        Ok(token_data.claims)
    }

    /// Extract token from Authorization header
    pub fn extract_token_from_header(auth_header: &str) -> Option<String> {
        if auth_header.starts_with("Bearer ") {
            Some(auth_header[7..].to_string())
        } else {
            None
        }
    }
}

/// Middleware for JWT authentication
pub struct JwtAuthMiddleware {
    secret_key: Rc<String>,
    expiration_time: usize,
}

impl JwtAuthMiddleware {
    /// Create a new JWT auth middleware
    pub fn new(secret_key: String, expiration_time: usize) -> Self {
        JwtAuthMiddleware {
            secret_key: Rc::new(secret_key),
            expiration_time,
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for JwtAuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static + MessageBody,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = Error;
    type Transform = JwtAuthMiddlewareService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(JwtAuthMiddlewareService {
            service,
            secret_key: self.secret_key.clone(),
            expiration_time: self.expiration_time,
        })
    }
}

pub struct JwtAuthMiddlewareService<S> {
    service: S,
    secret_key: Rc<String>,
    expiration_time: usize,
}

impl<S, B> Service<ServiceRequest> for JwtAuthMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static + MessageBody,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let secret_key = self.secret_key.clone();
        let expiration_time = self.expiration_time;

        // Extract authorization header
        let auth_header = req.headers().get("Authorization").cloned();

        match auth_header {
            Some(header_value) => {
                let header_str = match header_value.to_str() {
                    Ok(s) => s.to_string(),
                    Err(_) => {
                        return Box::pin(async move {
                            Err(ErrorUnauthorized("Invalid authorization header"))
                        });
                    }
                };

                // Extract token from Bearer scheme
                if let Some(token) = JwtService::extract_token_from_header(&header_str) {
                    // Validate token
                    let jwt_service = JwtService::new((*secret_key).clone(), expiration_time);
                    match jwt_service.validate_token(&token) {
                        Ok(claims) => {
                            // Store claims in request extensions
                            req.extensions_mut().insert(claims);
                            let fut = self.service.call(req);
                            Box::pin(async move { 
                                let res = fut.await?;
                                Ok(res.map_into_boxed_body())
                            })
                        }
                        Err(_) => {
                            Box::pin(async move {
                                Err(ErrorUnauthorized("Invalid or expired token"))
                            })
                        }
                    }
                } else {
                    Box::pin(async move {
                        Err(ErrorUnauthorized("Invalid authorization format"))
                    })
                }
            }
            None => {
                Box::pin(async move {
                    Err(ErrorUnauthorized("Missing authorization header"))
                })
            }
        }
    }
}
