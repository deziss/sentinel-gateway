import { useQuery } from "@tanstack/react-query"
import { getFeatures, type FeatureFlags, type PlanTier } from "@/lib/api"

const PLAN_RANK: Record<PlanTier, number> = {
  community: 0,
  professional: 1,
  enterprise: 2,
}

export function planMeets(actual: PlanTier | undefined, required: PlanTier): boolean {
  if (!actual) return false
  return PLAN_RANK[actual] >= PLAN_RANK[required]
}

export interface UsePlanResult {
  plan: PlanTier | undefined
  features: FeatureFlags | undefined
  isLoading: boolean
  isError: boolean
  /** True if the current plan has the named feature flag enabled. */
  has: (feature: keyof FeatureFlags) => boolean
  /** True if current plan tier >= required tier. */
  meets: (required: PlanTier) => boolean
}

/**
 * Load the current license's feature flags once per session and expose
 * convenience checks. Cached for 5 minutes — plan changes are rare and the
 * backend refreshes its own state periodically.
 */
export function usePlan(): UsePlanResult {
  const { data, isLoading, isError } = useQuery({
    queryKey: ["license-features"],
    queryFn: getFeatures,
    staleTime: 5 * 60 * 1000,
    // If license endpoint fails, fall back to "community" (deny new features).
    retry: 1,
  })

  const plan = data?.plan
  const features = data?.features

  return {
    plan,
    features,
    isLoading,
    isError,
    has: (feature) => {
      if (!features) return false
      const value = features[feature]
      return value === true
    },
    meets: (required) => planMeets(plan, required),
  }
}
