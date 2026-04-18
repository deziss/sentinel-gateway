import * as E from "fp-ts/Either"
import * as O from "fp-ts/Option"
import { pipe } from "fp-ts/function"

export interface JwtClaims {
  sub: string
  tid: string
  role: string
  typ: string
  exp: number
  iat: number
  jti: string
}

export function decodeJwt(token: string): E.Either<string, JwtClaims> {
  return E.tryCatch(
    () => {
      const parts = token.split(".")
      if (parts.length !== 3) throw new Error("Invalid JWT structure")
      const payload = JSON.parse(atob(parts[1]))
      return payload as JwtClaims
    },
    (e) => (e instanceof Error ? e.message : "JWT decode failed")
  )
}

export function getClaimsFromToken(
  token: string | null
): O.Option<JwtClaims> {
  return pipe(
    O.fromNullable(token),
    O.chain((t) =>
      pipe(
        decodeJwt(t),
        E.fold(() => O.none, O.some)
      )
    )
  )
}
