import { create } from "zustand"
import { persist } from "zustand/middleware"
import * as O from "fp-ts/Option"
import { pipe } from "fp-ts/function"
import * as api from "./api"
import { getClaimsFromToken } from "./jwt"

export interface UserInfo {
  id: string
  email: string
  tenantId: string
  role: string
}

interface AuthState {
  accessToken: string | null
  refreshToken: string | null
  tenantId: string | null
  user: UserInfo | null
  isAuthenticated: boolean
  login: (tenantSlug: string, email: string, password: string) => Promise<void>
  logout: () => void
}

export const useAuth = create<AuthState>()(
  persist(
    (set) => ({
      accessToken: api.getToken(),
      refreshToken: localStorage.getItem("refresh_token"),
      tenantId: api.getTenantId(),
      user: pipe(
        getClaimsFromToken(api.getToken()),
        O.map(
          (c): UserInfo => ({
            id: c.sub,
            email: "",
            tenantId: c.tid,
            role: c.role,
          })
        ),
        O.toNullable
      ),
      isAuthenticated: !!api.getToken(),

      login: async (tenantSlug, email, password) => {
        const res = await api.login(tenantSlug, email, password)
        const claims = getClaimsFromToken(res.access_token)

        const tenantId =
          res.tenant_id ??
          pipe(
            claims,
            O.map((c) => c.tid),
            O.toNullable
          ) ??
          tenantSlug

        const user = pipe(
          claims,
          O.map(
            (c): UserInfo => ({
              id: c.sub,
              email,
              tenantId: c.tid,
              role: c.role,
            })
          ),
          O.toNullable
        )

        set({
          accessToken: res.access_token,
          refreshToken: res.refresh_token,
          tenantId,
          user,
          isAuthenticated: true,
        })
      },

      logout: () => {
        api.clearAuth()
        set({
          accessToken: null,
          refreshToken: null,
          tenantId: null,
          user: null,
          isAuthenticated: false,
        })
      },
    }),
    {
      name: "auth-storage",
      partialize: (state) => ({
        accessToken: state.accessToken,
        refreshToken: state.refreshToken,
        tenantId: state.tenantId,
        user: state.user,
        isAuthenticated: state.isAuthenticated,
      }),
    }
  )
)
