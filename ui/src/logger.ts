/**
 * Structured logging utilities for Cloudflare Workers.
 * Provides consistent JSON-formatted logs for debugging and metrics collection.
 */

/**
 * Generates a unique request ID for tracing requests through the system.
 */
export function generateRequestId(): string {
  return crypto.randomUUID();
}

/**
 * Structured log entry for API requests.
 * Provides consistent format for debugging and metrics collection.
 */
export interface LogEntry {
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
export function log(entry: LogEntry): void {
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
export function getSafeHeaders(headers: Headers): Record<string, string> {
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
