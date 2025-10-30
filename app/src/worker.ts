export default {
  async fetch(req: Request, env: Env): Promise<Response> {
    const url = new URL(req.url);

    if (url.pathname.startsWith("/api/")) {
      const apiBase = "https://collects-api-145756646168.us-east1.run.app";

      const newPath = url.pathname.substring("/api".length);

      const newUrl = new URL(apiBase + newPath);
      newUrl.search = url.search;

      const newRequest = new Request(newUrl.toString(), req);

      return fetch(newRequest);
    }

    return await env.ASSETS.fetch(req);
  },
};
