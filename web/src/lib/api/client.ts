import type { components } from "./schema";

export type UserResponse = components["schemas"]["UserResponse"];
export type CourseListResponse = components["schemas"]["CourseListResponse"];
export type CourseDetail = components["schemas"]["CourseDetail"];
export type LessonResponse = components["schemas"]["LessonResponse"];
export type BlockView = components["schemas"]["BlockView"];
export type TaskView = components["schemas"]["TaskView"];
export type AttemptResponse = components["schemas"]["AttemptResponse"];
export type AttemptRequest = components["schemas"]["AttemptRequest"];

export class ApiError extends Error {
  constructor(
    public status: number,
    public code: string,
    message: string,
  ) {
    super(message);
  }
}

type RequestOptions = {
  method?: "GET" | "POST";
  body?: unknown;
  csrf?: boolean;
  idempotencyKey?: string;
};

export async function api<T>(
  path: string,
  options: RequestOptions = {},
): Promise<T> {
  const headers = new Headers();
  if (options.body !== undefined) {
    headers.set("content-type", "application/json");
  }
  if (options.csrf) {
    headers.set("x-csrf-token", await csrfToken());
  }
  if (options.idempotencyKey) {
    headers.set("idempotency-key", options.idempotencyKey);
  }
  const response = await fetch(`/api/v1${path}`, {
    method: options.method ?? "GET",
    headers,
    body: options.body === undefined ? undefined : JSON.stringify(options.body),
    credentials: "include",
  });
  if (response.status === 204) {
    return undefined as T;
  }
  const payload = (await response.json()) as {
    error?: { code?: string; message?: string };
  };
  if (!response.ok) {
    throw new ApiError(
      response.status,
      payload.error?.code ?? "internal_error",
      payload.error?.message ?? "Request failed",
    );
  }
  return payload as T;
}

async function csrfToken(): Promise<string> {
  const response = await fetch("/api/v1/auth/csrf", { credentials: "include" });
  if (!response.ok) {
    throw new ApiError(response.status, "csrf_rejected", "CSRF token missing");
  }
  const body = (await response.json()) as components["schemas"]["CsrfResponse"];
  return body.token;
}
