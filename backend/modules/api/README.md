# KnightVerse API Documentation

## Overview

This is the OpenAPI/Swagger documentation for the KnightVerse chess platform backend services, providing a comprehensive reference for developers. The documentation covers all API endpoints, WebSocket communication protocols, authentication flows, and includes automatic client SDK generation.

## Features

- **Complete API Coverage**: Documentation for all backend endpoints
- **Interactive Swagger UI**: Test endpoints directly from the browser
- **ReDoc Integration**: Alternative documentation viewer
- **WebSocket Documentation**: Detailed WebSocket event schemas
- **Client SDK Generation**: Automatic TypeScript, Python, and Rust client generation
- **Authentication Flow**: JWT-based authentication documentation
- **Request/Response Examples**: Clear examples for all operations

## Accessing the Documentation

When the server is running:

- **Swagger UI**: http://localhost:8080/api/docs
- **ReDoc**: http://localhost:8080/api/redoc
- **WebSocket Documentation**: http://localhost:8080/api/docs/websocket
- **Raw OpenAPI JSON**: http://localhost:8080/api/docs/openapi.json

## Documented Endpoints

The documentation covers:

### Player Management
- `POST /v1/players` - Create new player
- `GET /v1/players/{id}` - Get player by ID
- `PUT /v1/players/{id}` - Update player
- `DELETE /v1/players/{id}` - Delete player

### Game Management
- `POST /v1/games` - Create new game
- `GET /v1/games/{id}` - Get game by ID
- `PUT /v1/games/{id}/move` - Make a move
- `POST /v1/games/{id}/join` - Join a game
- `GET /v1/games` - List games
- `DELETE /v1/games/{id}` - Abandon game

### Authentication
- `POST /v1/auth/login` - User login
- `POST /v1/auth/register` - User registration
- `POST /v1/auth/refresh` - Refresh token
- `POST /v1/auth/logout` - User logout

### AI Suggestions
- `POST /v1/ai/suggest` - Get AI move suggestion
- `POST /v1/ai/analyze` - Analyze chess position

## Client SDK Generation

Generate client SDKs in multiple languages:

```bash
cd backend/modules/api/scripts
./generate-client-sdks.sh
```

Generated clients will be available in:
- `./generated-clients/typescript/` - TypeScript/JavaScript
- `./generated-clients/python/` - Python
- `./generated-clients/rust/` - Rust

## Updating the Documentation

See [API_DOCUMENTATION_GUIDE.md](API_DOCUMENTATION_GUIDE.md) for detailed instructions on how to maintain and update the API documentation.

## Authentication

The API uses JWT Bearer tokens for authentication:

```http
Authorization: Bearer <your-jwt-token>
```

Protected endpoints (like logout) are secured with JWT authentication middleware, which validates the token's signature and expiration time.

### JWT Configuration

JWT tokens are configured with:
- 1 hour expiration for access tokens
- 7 day expiration for refresh tokens
- Signed using HS256 algorithm with the `JWT_SECRET_KEY`

### Environment Variables

- `JWT_SECRET_KEY` - Secret key for JWT token generation and validation (default: development key, **not secure for production**)

⚠️ **Security Note**: Always set a strong, unique `JWT_SECRET_KEY` in production environments.

## CORS Configuration

The API includes CORS (Cross-Origin Resource Sharing) middleware for handling requests from web clients. By default, it's configured to be permissive in development mode, but can be restricted in production:

### Environment Variables

- `ALLOWED_ORIGINS`: Comma-separated list of allowed origins (e.g., `http://localhost:3000,https://knightverse.com`)

Example with specific origins:
```bash
ALLOWED_ORIGINS=http://localhost:3000,https://knightverse.com cargo run
```

If not specified, the server will allow all origins (suitable for development only).

## WebSocket Communication

The WebSocket protocol is documented at `/api/docs/websocket`, covering:

- Connection establishment
- Player join/leave events
- Move events
- Game state updates
- Chat messages
- Error handling

## Dependencies

- `utoipa`: OpenAPI generation for Rust
- `utoipa-swagger-ui`: Swagger UI integration
- `utoipa-redoc`: ReDoc integration

## License

MIT
