export type HealthEndpoint = "/healthz" | "/readyz";

export interface BackendPlaceholder {
  baseUrl: string;
  endpoints: HealthEndpoint[];
}

export function backendPlaceholder(): BackendPlaceholder {
  return {
    baseUrl: process.env.NEXT_PUBLIC_API_BASE_URL ?? "http://127.0.0.1:8080",
    endpoints: ["/healthz", "/readyz"]
  };
}
