import { DurableObject } from "cloudflare:workers";

export default {
	async fetch(request, _env, _ctx): Promise<Response> {
  return env.ASSETS.fetch(request);
	},
} satisfies ExportedHandler<Env>;
