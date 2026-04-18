import * as E from "fp-ts/Either"
import { pipe } from "fp-ts/function"
import * as A from "fp-ts/Array"

export type ValidationError = { field: string; message: string }
export type Validation<T> = E.Either<ValidationError[], T>

export const required =
  (field: string) =>
  (value: string): Validation<string> =>
    value.trim().length > 0
      ? E.right(value)
      : E.left([{ field, message: `${field} is required` }])

export const minLength =
  (field: string, min: number) =>
  (value: string): Validation<string> =>
    value.length >= min
      ? E.right(value)
      : E.left([
          { field, message: `${field} must be at least ${min} characters` },
        ])

export const email =
  (field: string) =>
  (value: string): Validation<string> =>
    /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value)
      ? E.right(value)
      : E.left([{ field, message: `${field} must be a valid email` }])

export const url =
  (field: string) =>
  (value: string): Validation<string> => {
    try {
      new URL(value)
      return E.right(value)
    } catch {
      return E.left([{ field, message: `${field} must be a valid URL` }])
    }
  }

export function validate<T>(
  value: T,
  ...validators: Array<(a: T) => Validation<T>>
): Validation<T> {
  const errors = pipe(
    validators,
    A.map((v) => v(value)),
    A.filter(E.isLeft),
    A.chain((e) => e.left)
  )
  return errors.length > 0 ? E.left(errors) : E.right(value)
}
