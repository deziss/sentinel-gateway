import * as TE from "fp-ts/TaskEither"
import { AxiosError, type AxiosResponse } from "axios"
import { sanitizeApiError } from "./api"

export interface ApiError {
  status: number
  message: string
  details?: unknown
}

function toApiError(err: unknown): ApiError {
  if (err instanceof AxiosError) {
    return {
      status: err.response?.status ?? 0,
      message: sanitizeApiError(err),
      details: err.response?.data,
    }
  }
  return {
    status: 0,
    message: err instanceof Error ? err.message : "Unknown error",
  }
}

export function taskFromApi<A>(
  f: () => Promise<A>
): TE.TaskEither<ApiError, A> {
  return TE.tryCatch(f, toApiError)
}

export function taskFromAxios<A>(
  f: () => Promise<AxiosResponse<A>>
): TE.TaskEither<ApiError, A> {
  return TE.tryCatch(() => f().then((r) => r.data), toApiError)
}
