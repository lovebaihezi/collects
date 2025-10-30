export default {
  async fetch(req: Request, env: Env): Promise<Response> {
    const url = new URL(req.url)

    // Serve static assets (JS, CSS, images, etc.) directly
    if (/\.[^/]+$/.test(url.pathname) && !url.pathname.endsWith('.html')) {
      return env.ASSETS.fetch(req)
    }

    // SSR for all other routes
    try {
      // Fetch the HTML template from assets
      const templateResponse = await env.ASSETS.fetch('/index.html')
      const template = await templateResponse.text()

      const [before, after] = template.split('<!--app-html-->')

      if (!before || after === undefined) {
        throw new Error('Missing <!--app-html--> marker in index.html')
      }

      const { render } = await import('./entry-worker')
      const reactStream = (await render()) as ReadableStream<Uint8Array>

      const { readable, writable } = new TransformStream()
      const writer = writable.getWriter()
      const encoder = new TextEncoder()

      void (async () => {
        try {
          await writer.write(encoder.encode(before))

          const reader = reactStream.getReader()
          while (true) {
            const { done, value } = await reader.read()
            if (done) break
            await writer.write(value)
          }

          await writer.write(encoder.encode(after))
          await writer.close()
        } catch (err) {
          console.error('Stream error:', err)
          await writer.abort(err instanceof Error ? err : new Error(String(err)))
        }
      })()

      return new Response(readable, {
        headers: {
          'Content-Type': 'text/html; charset=utf-8',
          'Cache-Control': 'no-cache',
        },
      })
    } catch (err) {
      console.error('SSR error:', err)
      return new Response('<h1>500 - Server Error</h1>', {
        status: 500,
        headers: { 'Content-Type': 'text/html' },
      })
    }
  },
}
