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
- `/docs` - OpenAPI documentation (Scalar UI) - only available in `internal` and `test-internal` environments
- `/openapi.json` - OpenAPI specification JSON - only available in `internal` and `test-internal` environments

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

## Internal Environment Requirement

For the `internal` and `test-internal` environments, Zero Trust configuration is **required**. The service will fail to start if `CF_ACCESS_TEAM_DOMAIN` or `CF_ACCESS_AUD` are not set:

```
Error: CF_ACCESS_TEAM_DOMAIN and CF_ACCESS_AUD must be set for internal environment.
Internal routes require Zero Trust authentication.
```

This ensures that internal routes (`/internal/*`) are always protected in these deployment environments.

## Setting Up Zero Trust Secrets

Zero Trust secrets are stored in Google Cloud Secret Manager and automatically injected during Cloud Run deployment.

### 1. Get Cloudflare Access Credentials

1. Go to [Cloudflare Zero Trust dashboard](https://one.dash.cloudflare.com)
2. Navigate to **Access > Applications**
3. Create a new **Self-hosted Application** (or use existing)
4. Configure Access policies (e.g., allow specific emails/groups)
5. Note the following values:
   - **Team Domain**: Shown at the top (e.g., `myteam.cloudflareaccess.com`)
   - **Application Audience (AUD)**: Found in the application settings

### 2. Store Secrets in GCP Secret Manager

Use the setup script to store your Zero Trust credentials:

```bash
# Interactive setup (prompts for values)
just scripts::zero-trust-setup --project-id YOUR_GCP_PROJECT_ID

# Check secret status
just scripts::zero-trust-list --project-id YOUR_GCP_PROJECT_ID
```

This creates two secrets:
- `cf-access-team-domain` - Your Cloudflare Access team domain
- `cf-access-aud` - Your application's audience tag

### 3. Grant Access to Compute Service Account

If you've already run `just scripts::actions-setup`, Zero Trust secrets are automatically granted access. Otherwise, run the setup again:

```bash
just scripts::actions-setup
```

### 4. Deploy to Internal Environment

The deployment script automatically injects Zero Trust secrets for the `internal` environment:

```bash
just services::gcloud-deploy internal <image_tag>
```

### Accessing Secrets Locally

For local testing with Zero Trust enabled:

```bash
export CF_ACCESS_TEAM_DOMAIN=$(gcloud secrets versions access latest --secret=cf-access-team-domain)
export CF_ACCESS_AUD=$(gcloud secrets versions access latest --secret=cf-access-aud)
```

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
