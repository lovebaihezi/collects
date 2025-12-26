async function handleApi(req: Request, env: Env): Promise<Response> {
  const url = new URL(req.url);
  const projectNumber = "145756646168";
  const apiBase = env.API_BASE || `https://collects-services-${projectNumber}.us-east1.run.app`;
  const newPath = url.pathname.substring("/api".length);
  const newUrl = new URL(apiBase + newPath);
  newUrl.search = url.search;
  const newRequest = new Request(newUrl.toString(), req);

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
