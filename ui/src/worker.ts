/**
 * Generates a unique request ID for tracing requests through the system.
 */
function generateRequestId(): string {
  return crypto.randomUUID();
}

/**
 * Structured log entry for API requests.
 * Provides consistent format for debugging and metrics collection.
 */
interface LogEntry {
  timestamp: string;
  requestId: string;
  level: "debug" | "info" | "warn" | "error";
  message: string;
  data?: Record<string, unknown>;
}

/**
 * Logs a structured entry to the console.
 * Uses JSON format for easy parsing by log aggregation tools.
 */
function log(entry: LogEntry): void {
  const logLine = JSON.stringify(entry);
  switch (entry.level) {
    case "debug":
      console.debug(logLine);
      break;
    case "info":
      console.info(logLine);
      break;
    case "warn":
      console.warn(logLine);
      break;
    case "error":
      console.error(logLine);
      break;
  }
}

/**
 * Headers that should be redacted from logs to prevent sensitive data exposure.
 */
const SENSITIVE_HEADERS = new Set([
  "authorization",
  "cookie",
  "set-cookie",
  "x-api-key",
  "x-auth-token",
  "x-csrf-token",
  "x-xsrf-token",
]);

/**
 * Filters headers to redact sensitive information.
 */
function getSafeHeaders(headers: Headers): Record<string, string> {
  const safeHeaders: Record<string, string> = {};
  headers.forEach((value, key) => {
    const lowerKey = key.toLowerCase();
    if (SENSITIVE_HEADERS.has(lowerKey)) {
      safeHeaders[key] = "[REDACTED]";
    } else {
      safeHeaders[key] = value;
    }
  });
  return safeHeaders;
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
