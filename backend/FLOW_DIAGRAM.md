# KnightVerse Backend - Registration & Authentication Flow

## System Architecture Diagram

```
┌─────────────┐
│   Client    │
│  (Browser)  │
└──────┬──────┘
       │
       │ HTTP POST /v1/auth/register
       │ { username, email, password }
       │
       ▼
┌─────────────────────────────────────────┐
│         Actix-web Server                │
│  (127.0.0.1:8080)                       │
│                                         │
│  ┌─────────────────────────────────┐   │
│  │  Route Handler: register()      │   │
│  │  - Validate input               │   │
│  │  - Call UserService            │   │
│  └──────────┬──────────────────────┘   │
│             │                           │
│             ▼                           │
│  ┌─────────────────────────────────┐   │
│  │  UserService::register()        │   │
│  │  - Check duplicate username     │   │
│  │  - Check duplicate email        │   │
│  │  - Hash password with bcrypt    │   │
│  │  - Insert user into database    │   │
│  └──────────┬──────────────────────┘   │
│             │                           │
│             ▼                           │
│  ┌─────────────────────────────────┐   │
│  │  JwtService::generate_token()   │   │
│  │  - Create Claims with user_id   │   │
│  │  - Set expiration (iat + 3600)  │   │
│  │  - Sign with HS256 algorithm    │   │
│  │  - Return JWT token             │   │
│  └──────────┬──────────────────────┘   │
│             │                           │
│             ▼                           │
│  ┌─────────────────────────────────┐   │
│  │  Return Response (201 Created)  │   │
│  │  {                              │   │
│  │    access_token: "eyJ...",      │   │
│  │    token_type: "Bearer",        │   │
│  │    expires_in: 3600,            │   │
│  │    user_id: 1,                  │   │
│  │    username: "chess_master"     │   │
│  │  }                              │   │
│  └─────────────────────────────────┘   │
└─────────────┬──────────────────────────┘
              │
              ▼
         Client receives JWT
```

## Detailed Flow: User Registration

### 1. **Client Initiates Registration**

```http
POST /v1/auth/register HTTP/1.1
Host: localhost:8080
Content-Type: application/json

{
  "username": "chess_master",
  "email": "player@example.com",
  "password": "SecurePass123"
}
```

### 2. **Handler Receives Request**
- File: `backend/modules/api/src/auth.rs` → `register()`
- Actix-web automatically deserializes JSON to `RegisterRequest` DTO
- Validates input using validator crate:
  - Username: 3-32 characters
  - Email: Valid email format
  - Password: Minimum 8 characters

### 3. **Service Layer Processes Registration**
- File: `backend/modules/service/src/user.rs` → `UserService::register()`

```rust
// Step 1: Check username uniqueness
Query: SELECT * FROM users WHERE username = "chess_master"
Result: Not found → Continue

// Step 2: Check email uniqueness  
Query: SELECT * FROM users WHERE email = "player@example.com"
Result: Not found → Continue

// Step 3: Hash password
Input:  "SecurePass123"
Process: bcrypt::hash(password, DEFAULT_COST=12)
Output: "$2b$12$XYZ...hash..." (60 character hash)

// Step 4: Create user model
let user = user::ActiveModel {
    username: AV::Set("chess_master"),
    email: AV::Set("player@example.com"),
    password_hash: AV::Set("$2b$12$XYZ...hash..."),
    created_at: AV::Set(Utc::now()),
    updated_at: AV::Set(Utc::now()),
    ..Default::default()
}

// Step 5: Insert into database
Query: INSERT INTO users (...) VALUES (...)
Result: User { id: 1, username: "chess_master", ... }
```

### 4. **JWT Token Generation**
- File: `backend/modules/security/src/jwt.rs` → `JwtService::generate_token()`

```rust
// Step 1: Create claims
let now = 1705967400 (Unix timestamp)
let claims = Claims {
    sub: "1",                           // user_id as string
    user_id: 1,                         // user_id as integer
    username: "chess_master",
    iat: 1705967400,                    // Issued at
    exp: 1705971000,                    // Expires in 3600 seconds
}

// Step 2: Encode with HS256
Input:  claims + secret_key
Process: jsonwebtoken::encode(
    &Header::default(),           // HS256 by default
    &claims,
    &EncodingKey::from_secret(key)
)
Output: "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9
         .eyJzdWIiOiIxIiwidXNlcl9pZCI6MSwi...
         .X0C8Y9Z5f6g7h8i9j0k1l2m3n4o5p6q7r8s"
```

### 5. **Response Sent to Client**

```http
HTTP/1.1 201 Created
Content-Type: application/json

{
  "access_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "token_type": "Bearer",
  "expires_in": 3600,
  "user_id": 1,
  "username": "chess_master"
}
```

## Database Schema Execution

```sql
-- Migration creates users table:

CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    username VARCHAR(255) UNIQUE NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_users_username ON users(username);
CREATE INDEX idx_users_email ON users(email);

-- Registration execution:

INSERT INTO users 
  (username, email, password_hash, created_at, updated_at)
VALUES 
  ('chess_master', 
   'player@example.com',
   '$2b$12$XYZ...hash...',
   '2024-01-23 12:30:00',
   '2024-01-23 12:30:00')
RETURNING *;

-- Result:
-- id | username | email | password_hash | created_at | updated_at
-- 1  | chess_master | player@example.com | $2b$... | ... | ...
```

## Login Flow (Similar but Simpler)

```
Client Request
  │
  ├─ Username: "chess_master"
  └─ Password: "SecurePass123"
       │
       ▼
  UserService::authenticate()
       │
       ├─ Query: SELECT * FROM users WHERE username = ?
       │
       ├─ Verify: bcrypt::verify(password, hash_from_db)
       │   - Hash input password: "SecurePass123"
       │   - Compare with stored: "$2b$12$XYZ...hash..."
       │   - Timing-safe comparison prevents timing attacks
       │
       └─ On match: Generate JWT token
            │
            ▼
       Return AuthResponse
```

## Protected Route Access Pattern

```
Client has token from registration/login:
"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."

Client Request
  │
  ├─ GET /protected/endpoint
  └─ Header: Authorization: Bearer eyJ...
       │
       ▼
  JwtAuthMiddleware (Extract & Validate)
       │
       ├─ Extract: "Authorization" header
       │
       ├─ Parse: Get token from "Bearer <token>"
       │
       ├─ Decode: jsonwebtoken::decode()
       │   - Verify signature with secret_key
       │   - Check expiration time
       │
       ├─ On success: Inject Claims into request
       │   - Claims available in handler: req.extensions()
       │
       └─ On failure: Return 401 Unauthorized
            │
            ▼
       Handler receives request (if authorized)
            │
            └─ Access claims.user_id, claims.username, etc.
```

## Error Scenarios

### Scenario 1: Duplicate Username
```
Request: Register "chess_master" (already exists)
  │
  ├─ Check query returns Some(user)
  │
  └─ Return Err("Username already exists")
       │
       ▼
  Handler catches error
       │
       └─ HTTP 400 Bad Request
          {
            "message": "Username already exists",
            "code": "REGISTRATION_ERROR"
          }
```

### Scenario 2: Invalid Password at Login
```
Request: Login with wrong password
  │
  ├─ Find user: "chess_master" ✓
  │
  ├─ Verify password:
  │   - Input: "WrongPassword"
  │   - Hash: "$2b$12$XYZ..."
  │   - Match: false ✗
  │
  └─ Return Err("Invalid password")
       │
       ▼
  Handler catches error
       │
       └─ HTTP 401 Unauthorized
          {
            "message": "Invalid password",
            "code": "AUTH_ERROR"
          }
```

### Scenario 3: Expired Token on Protected Route
```
Request: GET /protected/profile
  Header: Authorization: Bearer eyJ...  (expired)
  │
  ├─ Middleware extracts token
  │
  ├─ Decode with jsonwebtoken::decode()
  │   - Check: current_time > exp_time
  │   - Check: current_time > 1705971000
  │   - Result: true (expired) ✗
  │
  └─ Return Err("Invalid or expired token")
       │
       ▼
  Middleware rejects
       │
       └─ HTTP 401 Unauthorized
          {
            "message": "Invalid or expired token",
            "code": "AUTH_ERROR"
          }
```

## Data Flow Summary

### Registration Path
```
Client Input
  ↓
Handler Validation  
  ↓
Service Duplicate Check
  ↓
Password Hashing (bcrypt)
  ↓
Database Insert
  ↓
JWT Generation
  ↓
Response to Client
```

### Authentication Path  
```
Client Credentials
  ↓
Service User Lookup
  ↓
Password Verification (bcrypt)
  ↓
JWT Generation
  ↓
Response to Client
```

### Protected Access Path
```
Client + JWT Token
  ↓
Middleware Extract Token
  ↓
Decode & Verify Signature
  ↓
Check Expiration
  ↓
Inject Claims into Request
  ↓
Route Handler Processes Request
```

## Security Measures in Place

1. **Password Security**
   - Bcrypt hashing with 12 rounds
   - One-way transformation
   - Timing-safe comparison

2. **Token Security**
   - HS256 signing algorithm
   - Configurable expiration (3600s default)
   - Claims include user_id for quick lookup
   - Bearer token extraction

3. **Database Security**
   - Unique constraints on username/email
   - Indexed lookups for performance
   - Prepared statements via SeaORM

4. **Input Validation**
   - Length constraints
   - Format validation (email)
   - Type checking

5. **Error Handling**
   - Generic error messages (no user enumeration)
   - Proper HTTP status codes
   - Logging for auditing

## Performance Considerations

| Operation | Time | Notes |
|-----------|------|-------|
| Password Hash | ~100ms | Intentional, bcrypt is slow by design |
| Token Generation | <1ms | Fast cryptographic signing |
| Token Validation | <1ms | Fast verification |
| Database Query | ~5-10ms | Indexed lookups on username/email |
| Full Registration | ~110-120ms | Mostly bcrypt hashing time |
| Login | ~105-115ms | Mostly bcrypt verification time |

---

This flow ensures **secure, efficient, and user-friendly** authentication for KnightVerse! 🔐
