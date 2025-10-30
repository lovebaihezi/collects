/// <reference types="vite/client" />

// Extend JSX IntrinsicElements to include custom elements
declare namespace JSX {
  interface IntrinsicElements {
    'vite-streaming-end': React.DetailedHTMLProps<React.HTMLAttributes<HTMLElement>, HTMLElement>
  }
}

// Cloudflare Workers types
declare global {
  interface Fetcher {
    fetch(request: Request): Promise<Response>
    fetch(url: string, init?: RequestInit): Promise<Response>
  }

  interface Env {
    ASSETS: Fetcher
    STACK_PROJECT_ID?: string
    STACK_PUBLISHABLE_CLIENT_KEY?: string
    STACK_SECRET_SERVER_KEY?: string
  }
}

export {}
