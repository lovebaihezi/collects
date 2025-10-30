import { StrictMode } from 'react'
import { renderToReadableStream } from 'react-dom/server'
import App from './App'

/**
 * Worker-specific entry point for Cloudflare Workers
 * Uses renderToReadableStream which is compatible with Web Streams API
 */

export async function render(): Promise<ReadableStream> {
  const stream = await renderToReadableStream(
    <StrictMode>
      <App />
    </StrictMode>,
    {
      onError(error: unknown) {
        console.error('SSR rendering error:', error)
      },
    }
  )

  return stream
}

/**
 * Helper to convert ReadableStream to string (for debugging/fallback)
 */
export async function renderToString(): Promise<string> {
  const stream = await render()
  const reader = stream.getReader()
  const decoder = new TextDecoder()
  let result = ''

  while (true) {
    const { done, value } = await reader.read()
    if (done) break
    result += decoder.decode(value, { stream: true })
  }

  return result
}
