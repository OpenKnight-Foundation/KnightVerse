# KnightVerse Backend - Complete Implementation Guide

Welcome to the KnightVerse Backend! This document serves as the master index for all backend documentation and implementation.

## 📚 Documentation Index

### Getting Started
1. **[QUICK_START.md](./QUICK_START.md)** ⚡
   - 5-minute setup guide
   - Quick testing with curl
   - Common commands reference
   - **Start here if you're new!**

2. **[AUTHENTICATION_SETUP.md](./AUTHENTICATION_SETUP.md)** 📖
   - Comprehensive setup guide
   - Architecture explanation
   - Database schema details
   - API documentation
   - Troubleshooting guide

### Understanding the Implementation
3. **[../IMPLEMENTATION_SUMMARY.md](../IMPLEMENTATION_SUMMARY.md)** 📋
   - Complete implementation overview
   - What was built and why
   - File-by-file breakdown
   - Architecture highlights
   - Usage examples

4. **[FLOW_DIAGRAM.md](./FLOW_DIAGRAM.md)** 🔄
   - Registration flow diagram
   - Login flow explanation
   - Protected route access pattern
   - Error scenarios
   - Security measures in place

## 🚀 Quick Navigation

### I want to...

#### **Run the server right now**
→ Go to [QUICK_START.md](./QUICK_START.md) and follow the 5-minute setup

#### **Understand the full architecture**
→ Read [AUTHENTICATION_SETUP.md](./AUTHENTICATION_SETUP.md) 

#### **See code flow diagrams**
→ Check [FLOW_DIAGRAM.md](./FLOW_DIAGRAM.md)

#### **Test the endpoints**
→ Use [QUICK_START.md - Testing section](./QUICK_START.md#3-test-authentication)

#### **Deploy to production**
→ See [AUTHENTICATION_SETUP.md - Security Checklist](./AUTHENTICATION_SETUP.md#security-checklist)

## 📁 Project Structure

```
backend/
├── QUICK_START.md                      # Quick 5-min setup
├── AUTHENTICATION_SETUP.md             # Full documentation  
├── FLOW_DIAGRAM.md                     # Architecture diagrams
├── .env.example                        # Config template
├── Cargo.toml                          # Root workspace
├── src/
│   └── main.rs                         # Entry point
└── modules/
    ├── api/                            # HTTP server & routes
    │   └── src/
    │       ├── auth.rs                # Register/Login handlers
    │       ├── server.rs              # Server setup
    │       └── lib.rs
    ├── db/                             # Database & ORM
    │   ├── entity/user.rs             # User model
    │   └── migrations/                # Database migrations
    ├── service/                        # Business logic
    │   └── src/user.rs                # User operations
    ├── security/                       # JWT & Auth
    │   └── src/jwt.rs                 # JWT service
    ├── dto/                            # Data Transfer Objects
    │   └── src/auth.rs                # Auth request/response
    └── error/                          # Error handling
```

## ✨ What's Implemented

### ✅ Completed Features

- [x] **Backend Structure** - Clean modular workspace
- [x] **User Registration** - Create accounts with validation
- [x] **User Authentication** - Login with credentials
- [x] **JWT Tokens** - Token generation and validation
- [x] **Password Security** - Bcrypt hashing (12 rounds)
- [x] **Database Layer** - PostgreSQL with SeaORM
- [x] **API Documentation** - Swagger UI + ReDoc
- [x] **Error Handling** - Structured error responses
- [x] **CORS Support** - Configurable origins
- [x] **Logging** - Comprehensive request logging
- [x] **Environment Config** - .env file support

### 📋 Endpoints Available

```
Authentication:
  POST /v1/auth/register          Register new user
  POST /v1/auth/login             Login existing user

Health & Info:
  GET  /health                    Server health check
  GET  /                           Welcome message
  
Documentation:
  GET  /api/docs                  Swagger UI (interactive)
  GET  /api/redoc                 ReDoc UI (read-only)
```

### ⏳ Coming Soon

- [ ] Get Current User Profile
- [ ] Update User Profile  
- [ ] Token Refresh
- [ ] Password Reset Flow
- [ ] Logout/Token Blacklist
- [ ] Role-Based Access Control
- [ ] Game Endpoints
- [ ] Matchmaking
- [ ] Tournament Management

## 🔐 Security Features

✓ **Password Hashing** - Bcrypt with 12 rounds  
✓ **JWT Authentication** - HS256 signed tokens  
✓ **Input Validation** - All inputs validated  
✓ **Unique Constraints** - No duplicate users  
✓ **CORS Protection** - Configurable origins  
✓ **Error Handling** - No sensitive info leaked  
✓ **Logging** - Audit trail available  

## 📊 Technology Stack

| Layer | Technology | Version |
|-------|-----------|---------|
| **Web Framework** | Actix-web | 4.4 |
| **Database ORM** | SeaORM | 1.1.0 |
| **Authentication** | jsonwebtoken | 9.2 |
| **Password Hashing** | bcrypt | 0.15 |
| **Validation** | validator | 0.17.0 |
| **API Docs** | utoipa | 5 |
| **Runtime** | Tokio | 1.38 |
| **Database** | PostgreSQL | 13+ |

## 🛠️ Development Commands

```bash
# Build
cargo build

# Run
cargo run

# Run with logging
RUST_LOG=debug cargo run

# Test
cargo test

# Format
cargo fmt

# Lint
cargo clippy

# Documentation
cargo doc --open
```

## 🧪 Testing

### Quick Health Check
```bash
curl http://localhost:8080/health
```

### Register User
```bash
curl -X POST http://localhost:8080/v1/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "username": "testuser",
    "email": "test@example.com",
    "password": "TestPass123"
  }'
```

### Login
```bash
curl -X POST http://localhost:8080/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "username": "testuser",
    "password": "TestPass123"
  }'
```

### Interactive Testing
Open browser: **http://localhost:8080/api/docs**

## 📝 Configuration

### Required Environment Variables
```env
DATABASE_URL=postgres://user:pass@localhost:5432/knightverse_db
```

### Optional with Defaults
```env
SERVER_ADDR=127.0.0.1:8080
JWT_SECRET_KEY=knightverse_dev_secret_key_change_in_production
JWT_EXPIRATION_SECS=3600
RUST_LOG=info
ALLOWED_ORIGINS=http://localhost:3000,http://localhost:3001
```

## 🔄 Request/Response Examples

### Register Request
```json
{
  "username": "chess_master",
  "email": "player@example.com",
  "password": "SecurePass123"
}
```

### Register Response (201 Created)
```json
{
  "access_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
  "token_type": "Bearer",
  "expires_in": 3600,
  "user_id": 1,
  "username": "chess_master"
}
```

### Error Response (400 Bad Request)
```json
{
  "message": "Username already exists",
  "code": "REGISTRATION_ERROR"
}
```

## 🎯 Next Steps

### For Developers
1. Read [QUICK_START.md](./QUICK_START.md) to get running
2. Review [FLOW_DIAGRAM.md](./FLOW_DIAGRAM.md) to understand architecture
3. Check existing endpoint implementations in `modules/api/src/auth.rs`
4. Extend with new features

### For Production
1. Change `JWT_SECRET_KEY` to a strong random value
2. Set `ALLOWED_ORIGINS` to your domain(s)
3. Use environment-specific `.env` files
4. Enable HTTPS
5. Set up database backups
6. Configure logging/monitoring

### For Integration
1. Frontend should store JWT in localStorage or sessionStorage
2. Include token in `Authorization: Bearer <token>` header
3. Handle 401 responses by redirecting to login
4. Implement token refresh logic (coming soon)

## 📞 Support

### Debugging
- Check logs: `RUST_LOG=debug cargo run`
- Review [AUTHENTICATION_SETUP.md - Troubleshooting](./AUTHENTICATION_SETUP.md#troubleshooting)
- Test database connection: `psql $DATABASE_URL`

### Common Issues
- **Port already in use** → Kill process or change `SERVER_ADDR`
- **Database connection failed** → Check `DATABASE_URL` and database exists
- **Migrations not found** → Ensure in correct directory and sea-orm-cli installed
- **CORS errors** → Set `ALLOWED_ORIGINS` environment variable

## 📚 Additional Resources

- [Actix-web Documentation](https://actix.rs/)
- [SeaORM Documentation](https://www.sea-ql.org/SeaORM/)
- [JWT RFC 7519](https://tools.ietf.org/html/rfc7519)
- [Bcrypt Algorithm](https://en.wikipedia.org/wiki/Bcrypt)
- [PostgreSQL Documentation](https://www.postgresql.org/docs/)

## 📞 Contact & Contributing

For questions or issues:
1. Check the troubleshooting sections in documentation
2. Review existing code implementations
3. Create detailed issue reports with logs
4. Submit pull requests with tests

---

## 📋 Quick Reference

| Document | Purpose | Read Time |
|----------|---------|-----------|
| QUICK_START.md | Get running fast | 5 min |
| AUTHENTICATION_SETUP.md | Full documentation | 15 min |
| FLOW_DIAGRAM.md | Understand architecture | 10 min |
| IMPLEMENTATION_SUMMARY.md | What was built | 10 min |

**Start with [QUICK_START.md](./QUICK_START.md)** and refer to other docs as needed.

---

**Happy Coding!** 🎉

Built with ❤️ for KnightVerse Chess Platform
