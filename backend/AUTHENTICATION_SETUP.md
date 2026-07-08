# KnightVerse Backend - Authentication Setup Guide

This document describes the clean backend implementation with user registration and authentication endpoints.

## Project Structure

```
backend/
├── src/
│   └── main.rs                 # Single unified entry point
├── modules/
│   ├── api/                    # HTTP server and route handlers
│   │   ├── src/
│   │   │   ├── auth.rs        # Authentication handlers (register, login)
│   │   │   ├── server.rs      # Server configuration and initialization
│   │   │   └── lib.rs
│   │   └── Cargo.toml
│   ├── db/                     # Database and ORM layer
│   │   ├── entity/            # SeaORM entities
│   │   │   ├── user.rs        # User model
│   │   │   └── mod.rs
│   │   ├── migrations/        # Database migrations
│   │   │   └── src/
│   │   │       ├── m20250123_000001_create_users_table.rs
│   │   │       └── lib.rs
│   │   └── Cargo.toml
│   ├── service/               # Business logic
│   │   ├── src/
│   │   │   ├── user.rs        # User service (register, authenticate)
│   │   │   └── lib.rs
│   │   └── Cargo.toml
│   ├── security/              # JWT and auth middleware
│   │   ├── src/
│   │   │   ├── jwt.rs         # JWT service and middleware
│   │   │   └── main.rs
│   │   └── Cargo.toml
│   ├── dto/                   # Data Transfer Objects
│   │   ├── src/
│   │   │   ├── auth.rs        # Auth request/response DTOs
│   │   │   └── lib.rs
│   │   └── Cargo.toml
│   ├── error/                 # Error handling
│   │   └── ...
│   ├── Cargo.toml             # Main modules workspace
│   └── src/
│       └── main.rs            # Module re-exports for bin/server.rs
├── Cargo.toml                 # Root workspace
└── .env.example               # Environment variables template
```

## Features Implemented

### 1. **User Entity & Database**
- User model with fields: id, username, email, password_hash, created_at, updated_at
- Database migration for users table with proper indexes
- Unique constraints on username and email

### 2. **JWT Authentication**
- Token generation with configurable expiration
- Token validation and extraction from Authorization headers
- JWT claims with user_id, username, and standard iat/exp

### 3. **User Service**
- `register()` - Create new user with password hashing (bcrypt)
- `authenticate()` - Verify username/password credentials
- `get_by_id()`, `get_by_username()`, `get_by_email()` - Query utilities

### 4. **HTTP Endpoints**
```
POST /v1/auth/register    - Register new user
POST /v1/auth/login       - Login existing user
GET  /health              - Health check
GET  /                     - Welcome message
GET  /api/docs            - Swagger UI documentation
GET  /api/redoc           - ReDoc documentation
```

### 5. **Middleware & Security**
- JWT authentication middleware for protected routes
- CORS configuration with environment variables
- Request/response validation
- Proper error handling with structured responses

## Setup Instructions

### Prerequisites
- Rust 1.71+ (https://rustup.rs/)
- PostgreSQL 13+
- Environment with bash/zsh

### 1. Environment Setup

```bash
# Navigate to backend directory
cd backend

# Copy .env.example to .env
cp .env.example .env

# Edit .env with your values
vim .env
```

**Required Environment Variables:**
```env
DATABASE_URL=postgres://username:password@localhost:5432/knightverse_db
SERVER_ADDR=127.0.0.1:8080
JWT_SECRET_KEY=your_secret_key_here
JWT_EXPIRATION_SECS=3600
RUST_LOG=info
```

### 2. Database Setup

```bash
# Create PostgreSQL database
createdb knightverse_db

# Run migrations (from backend/modules directory)
cd modules/db/migrations
sea-orm-cli migrate run

# Or if migrations are managed by the app on startup:
# The app will auto-run migrations on first connection
```

### 3. Build & Run

```bash
# Build the project
cd backend
cargo build

# Run the server
cargo run

# Or run in release mode
cargo build --release
./target/release/knightverse-backend
```

The server will start at `http://127.0.0.1:8080`

## API Endpoints

### Health Check
```bash
curl http://localhost:8080/health
```

### User Registration
```bash
curl -X POST http://localhost:8080/v1/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "username": "chess_master",
    "email": "user@example.com",
    "password": "SecurePass123"
  }'
```

**Response:**
```json
{
  "access_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "token_type": "Bearer",
  "expires_in": 3600,
  "user_id": 1,
  "username": "chess_master"
}
```

### User Login
```bash
curl -X POST http://localhost:8080/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "username": "chess_master",
    "password": "SecurePass123"
  }'
```

**Response:** Same as registration response

### Using JWT Token
```bash
# Include token in Authorization header for protected endpoints
curl http://localhost:8080/protected/endpoint \
  -H "Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
```

## Database Schema

### users table
```sql
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
```

## Testing

### Using Swagger UI
Visit: `http://localhost:8080/api/docs`

All endpoints are documented with:
- Request/response schemas
- Example values
- Error responses
- "Try it out" functionality

### Using curl with registration flow
```bash
# 1. Register
REGISTER=$(curl -s -X POST http://localhost:8080/v1/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "username": "testuser",
    "email": "test@example.com",
    "password": "TestPass123"
  }')

echo $REGISTER
TOKEN=$(echo $REGISTER | jq -r '.access_token')

# 2. Use token on protected route (once implemented)
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/v1/protected/profile
```

## Architecture Decisions

### 1. Workspace Structure
- **Separated modules** for clear concerns (api, db, service, security, dto, error)
- **Single main.rs** entry point that delegates to api::server
- **Workspace Cargo.toml** for unified dependency management

### 2. JWT Implementation
- **HS256 algorithm** for token signing (fast, suitable for monolithic apps)
- **Configurable expiration** via environment variable
- **Claims structure** includes user_id and username for quick access
- **Middleware integration** using Actix-web Transform trait

### 3. Password Security
- **bcrypt hashing** with DEFAULT_COST (12 rounds)
- **No plaintext passwords** stored
- **Password verification** uses timing-safe comparison

### 4. Error Handling
- **Structured error responses** with message and code
- **HTTP status codes** mapped to application errors
- **Validation errors** returned with detailed messages

## Next Steps

1. **Add more endpoints:**
   - GET /v1/profile (protected) - Get current user
   - PUT /v1/profile (protected) - Update user
   - POST /v1/auth/logout - Logout (token blacklist)
   - POST /v1/auth/refresh - Refresh token

2. **Implement password reset:**
   - POST /v1/auth/forgot-password
   - POST /v1/auth/reset-password

3. **Add role-based access control (RBAC)**

4. **Integrate with game logic endpoints**

5. **Add integration tests**

## Troubleshooting

### Port already in use
```bash
# Find process using port 8080
lsof -i :8080

# Kill the process
kill -9 <PID>
```

### Database connection issues
```bash
# Test connection
psql postgresql://user:pass@localhost:5432/knightverse_db

# Check environment variables
echo $DATABASE_URL
```

### Compilation errors
```bash
# Clean build
cargo clean
cargo build

# Check Rust version
rustc --version
```

## License

This project is part of KnightVerse - see LICENSE file in root directory.
