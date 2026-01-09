import { generateRequestId, getSafeHeaders, log } from "./logger";

/**
 * Extract CF_Authorization token from the Cookie header.
 * Cloudflare Zero Trust sets this cookie automatically in the browser.
 */
function extractCfAuthorizationFromCookie(
  cookieHeader: string | null,
): string | null {
  if (!cookieHeader) return null;
  const match = cookieHeader.match(/CF_Authorization=([^;]+)/);
  return match ? match[1] : null;
}

async function handleApi(req: Request, env: Env): Promise<Response> {
  const requestId = generateRequestId();
  const requestTimestamp = new Date().toISOString();
  const startTime = performance.now();
  const url = new URL(req.url);
  const projectNumber = "145756646168";
  const apiBase =
    env.API_BASE ||
    `https://collects-services-${projectNumber}.us-east1.run.app`;
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

  // Clone headers and forward CF_Authorization cookie as cf-authorization header
  // This allows the backend to authenticate requests proxied through the Worker
  const headers = new Headers(req.headers);
  const cfAuthToken = extractCfAuthorizationFromCookie(
    req.headers.get("Cookie"),
  );
  if (cfAuthToken && !headers.has("cf-authorization")) {
    headers.set("cf-authorization", cfAuthToken);
  }

  const newRequest = new Request(newUrl.toString(), {
    method: req.method,
    headers,
    body: req.body,
    redirect: "manual",
  });

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
