import { createClerkClient } from "@clerk/backend";
import pkce from "pkce-challenge";

async function verifyToken(req: Request, env: Env): Promise<Response | null> {
  const clerkClient = createClerkClient({
    secretKey: env.CLERK_SECRET_KEY,
    publishableKey: env.CLERK_PUBLISHABLE_KEY,
  });

  const { isSignedIn } = await clerkClient.authenticateRequest(req, {
    jwtKey: env.CLERK_JWT_KEY,
  });

  if (isSignedIn) {
    return null;
  } else {
    return new Response("Token not verified", { status: 401 });
  }
}

async function handleLogin(req: Request, env: Env): Promise<Response> {
  const url = new URL(req.url);
  const writeKey = url.searchParams.get("write_key");

  if (writeKey) {
    const { code_verifier, code_challenge } = pkce();
    const state = {
      status: "pending",
      code_verifier: code_verifier,
      session_token: null,
    };
    await env.TOKEN_KV.put(writeKey, JSON.stringify(state));

    const clerkAuthURL = new URL(`https://${env.CLERK_FRONTEND_API}/oauth2/authorize`);
    clerkAuthURL.searchParams.set("response_type", "code");
    clerkAuthURL.searchParams.set("client_id", env.CLERK_PUBLISHABLE_KEY);
    clerkAuthURL.searchParams.set("redirect_uri", `${url.origin}/auth/callback`);
    clerkAuthURL.searchParams.set("scope", "openid profile email");
    clerkAuthURL.searchParams.set("code_challenge", code_challenge);
    clerkAuthURL.searchParams.set("code_challenge_method", "S256");
    clerkAuthURL.searchParams.set("state", writeKey);

    return Response.redirect(clerkAuthURL.toString(), 302);
  } else {
    return new Response("Missing write_key", { status: 400 });
  }
}

async function handleCallback(req: Request, env: Env): Promise<Response> {
  const url = new URL(req.url);
  const writeKey = url.searchParams.get("state");
  const code = url.searchParams.get("code");

  if (writeKey && code) {
    const stateJSON = await env.TOKEN_KV.get(writeKey);

    if (stateJSON) {
      const state = JSON.parse(stateJSON);
      const clerkClient = createClerkClient({
        secretKey: env.CLERK_SECRET_KEY,
      });

      const token = await clerkClient.oauth.getToken("oauth_custom", {
        code: code,
        code_verifier: state.code_verifier,
        redirect_uri: `${url.origin}/auth/callback`,
      });

      state.status = "success";
      state.session_token = token.sessionToken;
      await env.TOKEN_KV.put(writeKey, JSON.stringify(state));

      return new Response("Successfully authenticated", { status: 200 });
    } else {
      return new Response("Invalid write_key", { status: 400 });
    }
  } else {
    return new Response("Missing state or code", { status: 400 });
  }
}

async function handleToken(req: Request, env: Env): Promise<Response> {
  const url = new URL(req.url);
  const writeKey = url.searchParams.get("write_key");

  if (writeKey) {
    const stateJSON = await env.TOKEN_KV.get(writeKey);

    if (stateJSON) {
      const state = JSON.parse(stateJSON);
      if (state.status === "success") {
        return new Response(state.session_token, { status: 200 });
      } else {
        return new Response("Token not ready", { status: 404 });
      }
    } else {
      return new Response("Token not found", { status: 404 });
    }
  } else {
    return new Response("Missing write_key", { status: 400 });
  }
}

async function handleHealthCheck(req: Request, env: Env): Promise<Response> {
	return new Response(JSON.stringify({ status: "ok" }), {
		status: 200,
		headers: { "Content-Type": "application/json" },
	});
}

async function handleApi(req: Request, env: Env): Promise<Response> {
  const verificationResult = await verifyToken(req, env);

  if (verificationResult) {
    return verificationResult;
  }

  const url = new URL(req.url);
  const apiBase = "https://collects-api-145756646168.us-east1.run.app";
  const newPath = url.pathname.substring("/api".length);
  const newUrl = new URL(apiBase + newPath);
  newUrl.search = url.search;
  const newRequest = new Request(newUrl.toString(), req);

  return fetch(newRequest);
}

async function handle(req: Request, env: Env): Promise<Response> {
  const url = new URL(req.url);

  if (url.pathname.startsWith("/auth/login")) {
    return handleLogin(req, env);
  } else if (url.pathname.startsWith("/auth/callback")) {
    return handleCallback(req, env);
    } else if (url.pathname.startsWith("/auth/token")) {
    return handleToken(req, env);
    } else if (url.pathname.startsWith("/health")) {
      return handleHealthCheck(req, env);
    } else if (url.pathname.startsWith("/api/")) {
    return handleApi(req, env);
    }

    return await env.ASSETS.fetch(req);
  },

export default {
  async fetch(req: Request, env: Env): Promise<Response> {
    return handle(req, env);
  },
};
};
