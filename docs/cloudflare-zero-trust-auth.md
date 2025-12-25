# Cloudflare Zero Trust Authentication

This document describes how the Cloudflare Zero Trust (CF Access) authentication is implemented in the collects application.

## Overview

The authentication flow consists of three main components:

1. **Cloudflare Access** - Authenticates users and issues JWT tokens
2. **Cloudflare Worker** - Validates JWT tokens and forwards user information to the backend
3. **Rust Backend** - Enforces authorization on internal API routes

## Architecture

```
User Request → CF Access (JWT) → Cloudflare Worker → Backend API
                                      ↓
                                 Validates JWT
                                      ↓
                            Adds X-Auth-* headers
                                      ↓
                                Backend validates
                                  auth headers
```

## Cloudflare Worker (ui/src/worker.ts)

The worker validates incoming CF Access JWT tokens and forwards user information to the backend.

### JWT Token Validation

The worker:
1. Extracts the JWT from the `Cf-Access-Jwt-Assertion` header
2. Decodes the JWT payload (base64url encoded)
3. Validates token expiration
4. Extracts user information (sub, email, name)
5. Forwards user info in custom headers to the backend

### Protected Routes

Routes starting with `/internal/` require valid authentication. If no valid CF Access token is present, the worker returns `401 Unauthorized`.

### Headers Added by Worker

When a valid token is present, the worker adds these headers:

- `X-Auth-User-Id`: User's unique identifier (sub claim)
- `X-Auth-User-Email`: User's email address
- `X-Auth-User-Name`: User's display name (optional)

## Rust Backend (services/src/auth.rs)

The backend validates the auth headers forwarded by the worker.

### Authentication Middleware

The `require_auth` middleware:
1. Extracts user information from request headers
2. Allows the request to proceed if valid auth headers are present
3. Returns `401 Unauthorized` if auth headers are missing

### Usage

To protect a route, use the `require_auth` middleware:

```rust
use crate::auth;

// Protect specific routes
let internal_routes = Router::new()
    .route("/internal/status", get(internal_status))
    .route_layer(axum::middleware::from_fn(auth::require_auth));
```

### Extracting User Information

In your handler, extract user information from the request:

```rust
use crate::auth;

async fn my_handler(request: Request) -> impl IntoResponse {
    let user = auth::extract_auth_user(&request);
    
    match user {
        Some(u) => {
            // User is authenticated
            format!("Hello, {}!", u.email)
        },
        None => {
            // Should not happen if middleware is applied
            "Unauthorized".to_string()
        }
    }
}
```

## Configuration

### Cloudflare Access

Configure Cloudflare Access for your domain with appropriate authentication policies.

### Environment Variables

No additional environment variables are required. The system relies on:
- CF Access automatically adding the `Cf-Access-Jwt-Assertion` header
- The worker forwarding auth information via custom headers

## Security Considerations

### Current Implementation

The current implementation:
- ✅ Validates JWT token format and expiration in the worker
- ✅ Protects internal API routes from unauthenticated access
- ✅ Forwards user information securely via custom headers
- ⚠️ Does not verify JWT signature against CF public keys

### Production Recommendations

For production use, consider enhancing the security:

1. **Verify JWT Signature**: Validate the JWT signature against Cloudflare's public keys
   - Fetch JWKS from Cloudflare's JWKS endpoint
   - Cache the public keys
   - Verify the token signature cryptographically

2. **Add Rate Limiting**: Protect against token enumeration attacks

3. **Add Logging**: Log authentication failures for security monitoring

4. **Validate Additional Claims**: Check issuer (iss), audience (aud), and other JWT claims

### Example: Enhanced JWT Validation

```typescript
// In production, add signature verification:
async function validateCfAccessToken(
  request: Request,
  env: Env
): Promise<{ sub: string; email: string; name?: string } | null> {
  const token = request.headers.get("Cf-Access-Jwt-Assertion");
  
  if (!token) {
    return null;
  }

  // 1. Parse JWT
  const parts = token.split(".");
  if (parts.length !== 3) {
    return null;
  }

  // 2. Decode header and payload
  const header = JSON.parse(atob(parts[0].replace(/-/g, "+").replace(/_/g, "/")));
  const payload = JSON.parse(atob(parts[1].replace(/-/g, "+").replace(/_/g, "/")));
  
  // 3. Validate expiration
  const now = Math.floor(Date.now() / 1000);
  if (payload.exp && payload.exp < now) {
    return null;
  }

  // 4. TODO: Verify signature against CF public keys
  //    Fetch JWKS from: https://<your-team-name>.cloudflareaccess.com/cdn-cgi/access/certs
  //    Verify signature using the appropriate public key
  
  // 5. Validate issuer and audience
  // if (payload.iss !== 'https://<your-team-name>.cloudflareaccess.com') {
  //   return null;
  // }
  
  return {
    sub: payload.sub || payload.user_uuid || "",
    email: payload.email || "",
    name: payload.name || payload.common_name,
  };
}
```

## Testing

### Unit Tests

Run the Rust backend tests:

```bash
cd services
cargo test
```

### Manual Testing

1. **Test Public Endpoint** (no auth required):
   ```bash
   curl https://your-domain.com/api/is-health
   ```

2. **Test Protected Endpoint** (requires CF Access token):
   ```bash
   # This should return 401 without CF Access authentication
   curl https://your-domain.com/api/internal/status
   
   # Access through browser with CF Access authentication should work
   ```

## Troubleshooting

### Common Issues

1. **401 Unauthorized on protected routes**
   - Ensure Cloudflare Access is configured for your domain
   - Check that the `Cf-Access-Jwt-Assertion` header is present
   - Verify the JWT token is not expired

2. **Worker not forwarding auth headers**
   - Check worker logs in Cloudflare dashboard
   - Verify the JWT validation logic is working correctly

3. **Backend not recognizing authenticated requests**
   - Ensure the `require_auth` middleware is applied to protected routes
   - Check that auth headers are being set by the worker

## References

- [Cloudflare Access Documentation](https://developers.cloudflare.com/cloudflare-one/identity/authorization-cookie/)
- [JWT.io](https://jwt.io/) - JWT debugger
- [Cloudflare Workers Documentation](https://developers.cloudflare.com/workers/)
