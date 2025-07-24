import { WorkerEntrypoint } from "cloudflare:workers";

export default class CollectsWorker extends WorkerEntrypoint<Env> {
  fetch(request: Request) {
    return this.env.ASSETS.fetch(request);
  }
};
