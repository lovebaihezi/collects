# Cloudflare Zero Trust Authentication

This service supports protecting internal routes with Cloudflare Access (Zero Trust) JWT tokens.

## Configuration

To enable Zero Trust authentication, set the following environment variables:

```bash
# Required for Zero Trust authentication
CF_ACCESS_TEAM_DOMAIN=myteam.cloudflareaccess.com
CF_ACCESS_AUD=your-application-audience-tag
```

## Protected Routes

When Zero Trust is configured, the following routes require authentication:

- `/internal/*` - All internal endpoints (e.g., `/internal/users`)

## Public Routes

These routes are always accessible without authentication:

- `/is-health` - Health check endpoint
- `/auth/*` - Authentication endpoints (e.g., `/auth/verify-otp`)

## How It Works

1. **Token Extraction**: The middleware extracts JWT tokens from:
   - `CF-Authorization` header (Cloudflare specific)
   - `Authorization` header with `Bearer` prefix
   - `Authorization` header without prefix

2. **Token Validation**: The token is validated against Cloudflare's public keys:
   - Fetches JWKS from `https://{team_domain}/cdn-cgi/access/certs`
   - Verifies JWT signature using RS256 algorithm
   - Validates audience and issuer claims
   - Checks token expiration

3. **Request Processing**: On successful validation:
   - Claims are stored in request extensions
   - Request is forwarded to the route handler
   - Handlers can optionally extract `ZeroTrustAuth` to access user info

## Usage in Route Handlers

You can use the `ZeroTrustAuth` extractor in protected route handlers to access user information:

```rust
use collects_services::auth::ZeroTrustAuth;

async fn protected_handler(auth: ZeroTrustAuth) -> impl IntoResponse {
    let email = auth.email().unwrap_or("unknown");
    format!("Hello, {}!", email)
}
```

## Local Development

When `CF_ACCESS_TEAM_DOMAIN` and `CF_ACCESS_AUD` are not set, the middleware is not applied and all routes are accessible. This makes local development easier without needing to configure Zero Trust.

## Testing

Run the tests with:

```bash
cargo test
```

Integration tests verify:
- Routes work without Zero Trust configuration
- Auth routes are always accessible
- Health check is always accessible
- Zero Trust configuration can be created

## Cloudflare Access Setup

1. Go to Cloudflare Zero Trust dashboard
2. Create a new Application
3. Configure Access policies (e.g., allow specific emails/groups)
4. Note the Application Audience (AUD) tag
5. Configure the environment variables in your deployment

## Security Considerations

- Tokens are validated on every request to protected routes
- Public keys are fetched from Cloudflare's JWKS endpoint
- Token signature, expiration, audience, and issuer are all verified
- Failed authentication returns 401 Unauthorized with error details
