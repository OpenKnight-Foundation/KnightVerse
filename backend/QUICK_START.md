# KnightVerse Backend - Quick Start Guide

This guide helps you get the KnightVerse backend running quickly with functional user authentication.

## 1. Prerequisites Check

```bash
# Verify Rust installation
rustc --version
cargo --version

# Verify PostgreSQL
psql --version
```

## 2. Quick Setup (5 minutes)

### Step 1: Create Database

```bash
# Create PostgreSQL database
createdb knightverse_db

# Or use psql
psql -U postgres -c "CREATE DATABASE knightverse_db;"
```

### Step 2: Configure Environment

```bash
cd backend

# Copy .env template
cp .env.example .env

# Edit .env - at minimum set:
# DATABASE_URL=postgres://your_user:your_password@localhost:5432/knightverse_db
vim .env
```

### Step 3: Run Migrations

```bash
cd modules/db/migrations

# Install sea-orm-cli if not installed
cargo install sea-orm-cli

# Run migrations
sea-orm-cli migrate run

# Or from backend root:
cd /path/to/backend
cargo run --bin migrate_db  # if you create a migration binary
```

### Step 4: Start Server

```bash
# From backend directory
cargo run

# Or from backend/modules/api
cd modules/api
cargo run --bin server
```

Server starts at: **http://localhost:8080**

## 3. Test Authentication

### Option 1: Using curl

```bash
# 1. Register a user
curl -X POST http://localhost:8080/v1/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "username": "player1",
    "email": "player1@example.com",
    "password": "SecurePass123"
  }'

# Expected response:
# {
#   "access_token": "eyJ...",
#   "token_type": "Bearer",
#   "expires_in": 3600,
#   "user_id": 1,
#   "username": "player1"
# }

# 2. Login with same credentials
curl -X POST http://localhost:8080/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "username": "player1",
    "password": "SecurePass123"
  }'

# 3. Health check
curl http://localhost:8080/health
```

### Option 2: Using Swagger UI

1. Open browser: **http://localhost:8080/api/docs**
2. Click "Try it out" on `/v1/auth/register`
3. Fill in test data
4. Click "Execute"

## 4. Project Structure Overview

```
backend/
├── src/main.rs                    # Entry point
├── modules/
│   ├── api/                       # HTTP server & routes
│   │   └── src/auth.rs           # Register/Login handlers
│   ├── db/                        # Database & ORM
│   │   ├── entity/user.rs        # User model
│   │   └── migrations/           # DB migrations
│   ├── service/                   # Business logic
│   │   └── src/user.rs           # User operations
│   ├── security/                  # JWT & Auth
│   │   └── src/jwt.rs            # JWT service
│   └── dto/src/auth.rs            # Request/Response DTOs
├── .env.example                   # Environment template
└── AUTHENTICATION_SETUP.md        # Full documentation
```

## 5. Key Endpoints

| Method | Endpoint | Purpose |
|--------|----------|---------|
| POST | `/v1/auth/register` | Create new user account |
| POST | `/v1/auth/login` | Authenticate and get JWT token |
| GET | `/health` | Server health check |
| GET | `/api/docs` | Swagger UI documentation |
| GET | `/api/redoc` | ReDoc documentation |

## 6. Response Examples

### Successful Registration (201 Created)
```json
{
  "access_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "token_type": "Bearer",
  "expires_in": 3600,
  "user_id": 1,
  "username": "player1"
}
```

### Authentication Error (401 Unauthorized)
```json
{
  "message": "Invalid password",
  "code": "AUTH_ERROR"
}
```

### Validation Error (400 Bad Request)
```json
{
  "message": "Validation failed: ...",
  "code": "VALIDATION_ERROR"
}
```

## 7. Troubleshooting

### Error: Database connection failed
```bash
# Check DATABASE_URL
echo $DATABASE_URL

# Test connection
psql $DATABASE_URL -c "SELECT 1;"

# Verify database exists
psql -l | grep knightverse_db
```

### Error: Port 8080 already in use
```bash
# Find and kill process
lsof -i :8080
kill -9 <PID>

# Or use different port
SERVER_ADDR=127.0.0.1:8081 cargo run
```

### Error: Migrations not found
```bash
# Ensure you're in correct directory
cd backend/modules/db/migrations

# Check sea-orm-cli is installed
cargo install sea-orm-cli

# Run migrations explicitly
sea-orm-cli migrate run
```

## 8. Development Tips

### Enable Debug Logging
```bash
RUST_LOG=debug cargo run
```

### Watch for Changes
```bash
cargo install cargo-watch
cargo watch -x run
```

### Run Tests
```bash
cargo test
```

### Build Release
```bash
cargo build --release
./target/release/knightverse-backend
```

## 9. Next Features to Implement

1. ✅ User Registration
2. ✅ User Login & JWT
3. ⏳ Get Current User Profile
4. ⏳ Update Profile
5. ⏳ Password Reset
6. ⏳ User Logout / Token Blacklist
7. ⏳ Role-Based Access Control

## 10. Documentation Links

- [Full Setup Guide](./AUTHENTICATION_SETUP.md)
- [Swagger UI](http://localhost:8080/api/docs)
- [Actix-web Docs](https://actix.rs/)
- [SeaORM Docs](https://www.sea-ql.org/SeaORM/)
- [JWT Docs](https://tools.ietf.org/html/rfc7519)

## 11. Common Commands Reference

```bash
# Build everything
cargo build

# Run with specific features
cargo run --features default

# Run specific binary
cd modules/api && cargo run --bin server

# Check for issues
cargo check

# Format code
cargo fmt

# Lint code
cargo clippy

# Generate docs
cargo doc --open

# Clean build artifacts
cargo clean
```

## 12. Security Checklist

- [ ] Change `JWT_SECRET_KEY` in production
- [ ] Use strong database password
- [ ] Set `ALLOWED_ORIGINS` for CORS
- [ ] Use HTTPS in production
- [ ] Enable rate limiting (future)
- [ ] Add input sanitization (future)
- [ ] Implement CSRF protection (future)

---

**Ready to code!** 🚀

For more details, see [AUTHENTICATION_SETUP.md](./AUTHENTICATION_SETUP.md)
