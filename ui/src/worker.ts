import { generateRequestId, getSafeHeaders, log } from "./logger";

const WASM_PATH_PREFIX = "/wasm/";

/**
 * Handle requests for WASM files stored in R2.
 * Path format: /wasm/{pr_number}/{filename}
 * Example: /wasm/123/collects-ui-abc123.wasm
 */
async function handleWasm(req: Request, env: Env): Promise<Response> {
  const requestId = generateRequestId();
  const url = new URL(req.url);

  // Check if R2 bucket is configured
  if (!env.WASM_BUCKET) {
    log({
      timestamp: new Date().toISOString(),
      requestId,
      level: "debug",
      message: "WASM_BUCKET not configured, falling back to assets",
      data: { pathname: url.pathname },
    });
    return new Response("Not Found", { status: 404 });
  }

  // Extract the path after /wasm/
  // Format: /wasm/{pr_number}/{filename}
  const wasmPath = url.pathname.substring(WASM_PATH_PREFIX.length);

  log({
    timestamp: new Date().toISOString(),
    requestId,
    level: "debug",
    message: "Fetching WASM from R2",
    data: { wasmPath },
  });

  try {
    const object = await env.WASM_BUCKET.get(wasmPath);

    if (!object) {
      log({
        timestamp: new Date().toISOString(),
        requestId,
        level: "info",
        message: "WASM file not found in R2",
        data: { wasmPath },
      });
      return new Response("Not Found", { status: 404 });
    }

    const headers = new Headers();
    object.writeHttpMetadata(headers);
    headers.set("etag", object.httpEtag);
    headers.set("content-type", "application/wasm");
    headers.set("cache-control", "public, max-age=31536000, immutable");

    log({
      timestamp: new Date().toISOString(),
      requestId,
      level: "info",
      message: "Serving WASM from R2",
      data: {
        wasmPath,
        size: object.size,
        uploaded: object.uploaded.toISOString(),
      },
    });

    return new Response(object.body, { headers });
  } catch (error) {
    log({
      timestamp: new Date().toISOString(),
      requestId,
      level: "error",
      message: "Failed to fetch WASM from R2",
      data: {
        wasmPath,
        error: error instanceof Error ? error.message : String(error),
      },
    });
    return new Response("Internal Server Error", { status: 500 });
  }
}

async function handleApi(req: Request, env: Env): Promise<Response> {
  const requestId = generateRequestId();
  const requestTimestamp = new Date().toISOString();
  const startTime = performance.now();
  const url = new URL(req.url);
  const projectNumber = "145756646168";
  const apiBase = env.API_BASE || `https://collects-services-${projectNumber}.us-east1.run.app`;
  const newPath = url.pathname.substring("/api".length);
  const newUrl = new URL(apiBase + newPath);
  newUrl.search = url.search;

  // Log debug information about the incoming request
  log({
    timestamp: requestTimestamp,
    requestId,
    level: "debug",
    message: "Incoming API request",
    data: {
      method: req.method,
      originalUrl: url.toString(),
      pathname: url.pathname,
      search: url.search,
      targetUrl: newUrl.toString(),
      apiBase,
      headers: getSafeHeaders(req.headers),
    },
  });

  const newRequest = new Request(newUrl.toString(), req);

  let response: Response;

  try {
    response = await fetch(newRequest);
  } catch (error) {
    const endTime = performance.now();
    const durationMs = endTime - startTime;

    // Log error details
    log({
      timestamp: new Date().toISOString(),
      requestId,
      level: "error",
      message: "API request failed",
      data: {
        method: req.method,
        targetUrl: newUrl.toString(),
        durationMs,
        error: error instanceof Error ? error.message : String(error),
      },
    });

    throw error;
  }

  const endTime = performance.now();
  const durationMs = endTime - startTime;

  // Log metrics information about the completed request
  log({
    timestamp: new Date().toISOString(),
    requestId,
    level: "info",
    message: "API request completed",
    data: {
      method: req.method,
      targetUrl: newUrl.toString(),
      statusCode: response.status,
      statusText: response.statusText,
      durationMs,
      contentType: response.headers.get("content-type"),
      contentLength: response.headers.get("content-length"),
    },
  });

  return response;
}

async function handle(req: Request, env: Env): Promise<Response> {
  const url = new URL(req.url);

  if (url.pathname.startsWith("/api/")) {
    return handleApi(req, env);
  }

  if (url.pathname.startsWith(WASM_PATH_PREFIX)) {
    return handleWasm(req, env);
  }

  // Log non-API requests that result in 404
  log({
    timestamp: new Date().toISOString(),
    requestId: generateRequestId(),
    level: "debug",
    message: "Non-API request - returning 404",
    data: {
      method: req.method,
      pathname: url.pathname,
      fullUrl: url.toString(),
    },
  });

  return new Response("Not Found", { status: 404 });
}

export default {
  async fetch(req: Request, env: Env): Promise<Response> {
    return handle(req, env);
  },
};
