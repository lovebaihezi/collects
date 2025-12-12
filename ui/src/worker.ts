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

  if (url.pathname.startsWith("/health")) {
    return handleHealthCheck(req, env);
  } else if (url.pathname.startsWith("/api/")) {
    return handleApi(req, env);
  }

  return await env.ASSETS.fetch(req);
}

export default {
  async fetch(req: Request, env: Env): Promise<Response> {
    return handle(req, env);
  },
};
