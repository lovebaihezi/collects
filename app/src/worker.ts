export default {
	async fetch(req: Request, env: Env): Promise<Response> {
		const url = new URL(req.url);

		if (url.pathname.startsWith('/api/')) {
			const apiBase = 'https://collects-api-145756646168.us-east1.run.app';

			// Remove the /api prefix
			const newPath = url.pathname.substring('/api'.length);

			const newUrl = new URL(apiBase + newPath);
			newUrl.search = url.search;

			// Clone the request, but with the new URL
			const newRequest = new Request(newUrl.toString(), req);

			// Forward the request to the API
			return fetch(newRequest);
		}

		// For all other requests, serve from the static assets
		return env.ASSETS.fetch(req);
	},
};
