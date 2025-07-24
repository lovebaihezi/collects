# Deployment to Cloudflare Workers

This project is configured to be deployed as a Cloudflare Worker with static assets.

## Prerequisites

1. Install [Wrangler](https://developers.cloudflare.com/workers/wrangler/install-and-update/):
   ```bash
   npm install -g wrangler
   ```

2. Authenticate with Cloudflare:
   ```bash
   wrangler login
   ```

## Building the Application

1. Build the Rust WASM application with Trunk:
   ```bash
   trunk build --release
   ```

   This will generate the static files in the `dist/` directory.

## Deploying to Cloudflare Workers

1. Deploy the worker and static assets:
   ```bash
   wrangler deploy
   ```

   This will:
   - Compile the TypeScript worker (`worker/index.ts`)
   - Upload the static assets from the `dist/` directory
   - Deploy both as a single unit to Cloudflare

## How it Works

- The `wrangler.toml` configuration specifies:
  - `main = "worker/index.ts"` - The worker script
  - `[assets]` section - The static assets directory and settings

- The TypeScript worker (`worker/index.ts`) serves static assets using the ASSETS binding
- The `not_found_handling = "single-page-application"` setting ensures all non-API routes serve the main index.html, which is ideal for SPA applications

## Customizing the Worker

You can modify `worker/index.ts` to add custom logic for API routes or other server-side functionality.