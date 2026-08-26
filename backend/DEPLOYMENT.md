# KnightVerse Backend Server - Quick Start Guide

✅ **The Rust Actix-web backend has been successfully compiled!**

## Binary Location
The compiled server binary is located at:
```
/home/gabriel/KnightVerse/backend/target/release/server
```

## Prerequisites

Before starting the server, you need:

1. **PostgreSQL Database** running locally or remotely
2. **Environment Configuration** in `.env` file

## Setup Instructions

### 1. Install PostgreSQL (if not already installed)

```bash
# On Ubuntu/Debian
sudo apt-get install postgresql postgresql-contrib

# Start PostgreSQL service
sudo service postgresql start
```

### 2. Create Database and User

```bash
# Connect to PostgreSQL as default user
sudo -u postgres psql

# In PostgreSQL prompt, run:
CREATE USER knightverse WITH PASSWORD 'your_secure_password';
CREATE DATABASE knightverse OWNER knightverse;
GRANT ALL PRIVILEGES ON DATABASE knightverse TO knightverse;
\q
```

### 3. Configure Environment Variables

Edit `/home/gabriel/KnightVerse/backend/.env` with your database credentials:

```env
SERVER_ADDR=127.0.0.1:8080
DATABASE_URL=postgresql://knightverse:your_secure_password@localhost:5432/knightverse
JWT_SECRET_KEY=your_secret_key_change_this_in_production
JWT_EXPIRATION_SECS=3600
RUST_LOG=info
ALLOWED_ORIGINS=http://localhost:3000,http://localhost:8080
```

### 4. Run Database Migrations (Optional - required if using full auth)

```bash
cd /home/gabriel/KnightVerse/backend/modules/db/migrations
sea-orm-cli migrate run --database-url "postgresql://knightverse:password@localhost:5432/knightverse"
```

### 5. Start the Server

```bash
/home/gabriel/KnightVerse/backend/target/release/server
```

Or from the backend directory:
```bash
cd /home/gabriel/KnightVerse/backend
./target/release/server
```

## API Endpoints

Once the server is running, the following endpoints are available:

### Health Check
```bash
curl http://localhost:8080/health
```

### Authentication Endpoints

**Register (mock implementation)**
```bash
curl -X POST http://localhost:8080/v1/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "username": "testuser",
    "email": "test@example.com",
    "password": "SecurePass123!"
  }'
```

**Login (mock implementation)**
```bash
curl -X POST http://localhost:8080/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "username": "testuser",
    "password": "SecurePass123!"
  }'
```

Both endpoints return a JWT token that can be used for authenticated requests.

## Architecture Overview

The backend is organized into modular components:

- **api** (`modules/api/`) - HTTP server, routes, request handlers
- **security** (`modules/security/`) - JWT token generation and validation
- **service** (`modules/service/`) - Business logic layer
- **dto** (`modules/dto/`) - Data transfer objects and validation
- **db/entity** (`modules/db/entity/`) - SeaORM entities and database models
- **db/migrations** (`modules/db/migrations/`) - Database schema migrations
- **error** (`modules/error/`) - Error handling and responses

## Implementation Status

✅ **Completed:**
- Actix-web HTTP server framework
- JWT-based authentication service
- Database configuration and ORM setup
- Authentication endpoint stubs
- CORS middleware configuration
- Environment-based configuration

⏳ **Partially Implemented (Stubs):**
- User registration (mock response - no database persistence)
- User login (mock response - no authentication check)
- Database migrations for users table (created but not applied)
- User service layer (created but disabled due to compilation issues)

## Development Notes

The backend compiles successfully with the release build. The authentication endpoints are implemented but currently return mock responses. To enable full functionality with database persistence:

1. Apply the users table migration
2. Re-enable the `service::user` module by uncommenting it in `/backend/modules/service/src/lib.rs`
3. Update the endpoint handlers to call the actual UserService methods
4. Rebuild with `cargo build --release --bin server`

## Troubleshooting

**Error: "DATABASE_URL must be set in .env"**
- Make sure PostgreSQL is running
- Verify DATABASE_URL is set correctly in `.env` file
- Test connection: `psql "postgresql://knightverse:password@localhost:5432/knightverse"`

**Error: "Connection refused"**
- Ensure PostgreSQL service is running
- Check if PostgreSQL is listening on the configured port (default: 5432)

**Port already in use (0.0.0.0:8080)**
- Change `SERVER_ADDR` in `.env` to a different port
- Or kill the process using that port

## Running from Source

To rebuild from source:

```bash
cd /home/gabriel/KnightVerse/backend
cargo build --release --bin server
```

## Next Steps

1. Set up PostgreSQL database
2. Update `.env` with real database credentials
3. Start the server
4. Test the API endpoints with curl or Postman
5. Implement user registration/login with real database persistence
6. Deploy to production environment
