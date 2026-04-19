import { Link } from "react-router-dom"
import { Lock, Sparkles } from "lucide-react"
import { usePlan } from "@/hooks/use-plan"
import type { FeatureFlags, PlanTier } from "@/lib/api"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"

type FlagKey = Extract<keyof FeatureFlags, string>

interface FeatureGateProps {
  /** FeatureFlags flag to check (e.g., "sso_enabled"). */
  feature: FlagKey
  /** Human-readable name for the upsell card. */
  title: string
  /** Shown in the upsell card. */
  description?: string
  /** Plan tier shown as "required". Defaults to "professional". */
  requiredPlan?: PlanTier
  children: React.ReactNode
}

const PLAN_LABELS: Record<PlanTier, string> = {
  community: "Open Source",
  professional: "Professional",
  enterprise: "Enterprise",
}

/**
 * Wraps page content and shows an upsell card when the current plan doesn't
 * include the feature. If license data is still loading, renders a skeleton
 * to avoid flashing the upsell.
 */
export function FeatureGate({
  feature,
  title,
  description,
  requiredPlan = "professional",
  children,
}: FeatureGateProps) {
  const { has, plan, isLoading } = usePlan()

  if (isLoading) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-8 w-64" />
        <Skeleton className="h-32 w-full" />
      </div>
    )
  }

  if (has(feature)) {
    return <>{children}</>
  }

  const currentPlan = plan ?? "community"

  return (
    <Card className="max-w-2xl">
      <CardHeader>
        <div className="flex items-center justify-between">
          <CardTitle className="flex items-center gap-2">
            <Lock className="h-5 w-5 text-muted-foreground" />
            {title}
          </CardTitle>
          <Badge variant="outline">
            Requires {PLAN_LABELS[requiredPlan]}
          </Badge>
        </div>
        {description && <CardDescription className="mt-2">{description}</CardDescription>}
      </CardHeader>
      <CardContent>
        <div className="rounded-lg border border-dashed p-6 text-center space-y-3">
          <Sparkles className="h-8 w-8 mx-auto text-muted-foreground" />
          <div>
            <p className="font-medium">
              This feature isn't available on the <strong>{PLAN_LABELS[currentPlan]}</strong> plan.
            </p>
            <p className="text-sm text-muted-foreground mt-1">
              Upgrade to <strong>{PLAN_LABELS[requiredPlan]}</strong> to unlock{" "}
              <em>{title.toLowerCase()}</em>.
            </p>
          </div>
          <Button asChild variant="default">
            <Link to="/billing">View upgrade options</Link>
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}
