/**
 * Validates the Cloudflare Access JWT token
 * @param request The incoming request
 * @param env Environment variables
 * @returns User information if valid, null if invalid
 */
async function validateCfAccessToken(
  request: Request,
  env: Env,
): Promise<{ sub: string; email: string; name?: string } | null> {
  // Get the JWT token from the Cf-Access-Jwt-Assertion header
  const token = request.headers.get("Cf-Access-Jwt-Assertion");

  if (!token) {
    return null;
  }

  try {
    // Parse the JWT (format: header.payload.signature)
    const parts = token.split(".");
    if (parts.length !== 3) {
      console.error("Invalid JWT format");
      return null;
    }

    // Decode the payload (base64url encoded)
    const payload = JSON.parse(
      atob(parts[1].replace(/-/g, "+").replace(/_/g, "/")),
    );

    // Basic validation: check expiration
    const now = Math.floor(Date.now() / 1000);
    if (payload.exp && payload.exp < now) {
      console.error("Token expired");
      return null;
    }

    // Extract user information from the payload
    return {
      sub: payload.sub || payload.user_uuid || "",
      email: payload.email || "",
      name: payload.name || payload.common_name,
    };
  } catch (error) {
    console.error("Error validating CF Access token:", error);
    return null;
  }
}

async function handleApi(req: Request, env: Env): Promise<Response> {
  const url = new URL(req.url);
  const apiBase = "https://collects-api-145756646168.us-east1.run.app";
  const newPath = url.pathname.substring("/api".length);
  const newUrl = new URL(apiBase + newPath);
  newUrl.search = url.search;

  // Validate CF Access token
  const userInfo = await validateCfAccessToken(req, env);

  // Create new headers with auth information
  const headers = new Headers(req.headers);

  if (userInfo) {
    // Add authenticated user headers for the backend
    headers.set("X-Auth-User-Id", userInfo.sub);
    headers.set("X-Auth-User-Email", userInfo.email);
    if (userInfo.name) {
      headers.set("X-Auth-User-Name", userInfo.name);
    }
  } else {
    // For internal APIs that require authentication, return 401
    if (newPath.startsWith("/internal/")) {
      return new Response("Unauthorized", { status: 401 });
    }
  }

  const newRequest = new Request(newUrl.toString(), {
    method: req.method,
    headers: headers,
    body: req.body,
    redirect: req.redirect,
  });

  return fetch(newRequest);
}

async function handle(req: Request, env: Env): Promise<Response> {
  const url = new URL(req.url);

  if (url.pathname.startsWith("/api/")) {
    return handleApi(req, env);
  }

  return new Response("Not Found", { status: 404 });
}

export default {
  async fetch(req: Request, env: Env): Promise<Response> {
    return handle(req, env);
  },
};
