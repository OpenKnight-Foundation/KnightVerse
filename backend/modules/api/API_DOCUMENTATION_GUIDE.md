# KnightVerse API Documentation Guide

This guide explains how to maintain and update the OpenAPI/Swagger documentation for the KnightVerse chess platform.

## Overview

The API documentation is built using:
- [utoipa](https://github.com/juhaku/utoipa): OpenAPI 3.0+ code generation for Rust
- [Swagger UI](https://swagger.io/tools/swagger-ui/): Interactive API documentation interface
- [ReDoc](https://github.com/Redocly/redoc): Alternative documentation viewer
- [OpenAPI Generator](https://openapi-generator.tech/): Client SDK generation

## Accessing the Documentation

When the server is running, the API documentation is available at:
- Swagger UI: http://localhost:8080/api/docs
- ReDoc: http://localhost:8080/api/redoc
- WebSocket Documentation: http://localhost:8080/api/docs/websocket
- Raw OpenAPI JSON: http://localhost:8080/api/docs/openapi.json

## How to Update the API Documentation

### 1. Adding New Endpoint Documentation

To document a new endpoint, add the `#[utoipa::path]` attribute to your handler function:

```rust
#[utoipa::path(
    post,
    path = "/v1/your-path",
    request_body = YourRequestType,
    responses(
        (status = 200, description = "Success description", body = YourResponseType),
        (status = 400, description = "Error description", body = ErrorResponse)
    ),
    security(
        ("jwt_auth" = [])
    ),
    tag = "Your Tag"
)]
#[post("")]
pub async fn your_handler(payload: Json<YourRequestType>) -> HttpResponse {
    // Implementation
}
```

### 2. Documenting Request/Response Models

Ensure your DTOs (Data Transfer Objects) have the `#[derive(ToSchema)]` attribute:

```rust
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct YourModel {
    #[schema(example = "example value")]
    pub field: String,
    
    #[schema(value_type = String, format = "uuid", example = "123e4567-e89b-12d3-a456-426614174000")]
    pub id: Uuid,
}
```

### 3. Registering the New Endpoint

After creating a new endpoint, add it to the `ApiDoc` struct in `openapi.rs`:

```rust
#[derive(OpenApi)]
#[openapi(
    paths(
        // Add your new endpoint here
        your_module::your_handler,
    ),
    components(
        schemas(
            // Add your new models here
            dto::your_module::YourModel,
        )
    ),
)]
pub struct ApiDoc;
```

### 4. Adding a New Module

If you're adding a completely new module:

1. Create your module file with handler functions (e.g., `src/your_module.rs`)
2. Create DTO models in `dto/src/your_module.rs`
3. Add the module to `src/lib.rs`
4. Add it to `dto/src/lib.rs`
5. Register endpoints in `openapi.rs`
6. Add routes to `server.rs`

### 5. Documenting WebSocket Events

Since WebSocket events can't be automatically documented with utoipa, update the WebSocket documentation in `openapi.rs`:

```rust
pub fn websocket_documentation() -> String {
    r#"
# WebSocket Protocol Documentation

## Your New Event
```json
{
  "type": "your_event",
  "data": {
    "field1": "value",
    "field2": 123
  }
}
```
    "#
}
```

## Generating Client SDKs

To generate client SDKs based on the OpenAPI specification:

1. Start the server
2. Run the SDK generation script:

```bash
cd backend/modules/api/scripts
./generate-client-sdks.sh
```

The script will generate client libraries for:
- TypeScript/JavaScript (with Axios)
- Python
- Rust

## Best Practices

1. **Examples**: Always provide examples in schema attributes
2. **Validation**: Use the `validator` crate with DTO models
3. **Security**: Mark endpoints that require authentication with the `security` attribute
4. **Tags**: Use consistent tags to group related endpoints
5. **Responses**: Document all possible response status codes and bodies
6. **Descriptions**: Write clear descriptions for endpoints and models

## Testing Documentation Changes

After making changes to the documentation:

1. Run the server:
```bash
cargo run
```

2. Check the Swagger UI for errors or missing information
3. Verify the generated client SDKs work correctly

## Common Issues

- **Missing Schema**: If a schema is referenced but not registered in the components section
- **Invalid Examples**: Ensure examples match the actual data type
- **Path Conflicts**: Endpoints with the same path but different methods need careful configuration

## Additional Resources

- [utoipa Documentation](https://docs.rs/utoipa/latest/utoipa/)
- [OpenAPI 3.0 Specification](https://spec.openapis.org/oas/v3.0.3)
- [Swagger UI Configuration](https://swagger.io/docs/open-source-tools/swagger-ui/usage/configuration/)
