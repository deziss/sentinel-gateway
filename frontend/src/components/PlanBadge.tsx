import { usePlan } from "@/hooks/use-plan"
import { Badge } from "@/components/ui/badge"
import { Crown, Rocket, Box } from "lucide-react"
import type { PlanTier } from "@/lib/api"

const PLAN_CONFIG: Record<PlanTier, { label: string; variant: "outline" | "secondary" | "default"; icon: React.ComponentType<{ className?: string }> }> = {
  community: { label: "Open Source", variant: "outline", icon: Box },
  professional: { label: "Pro", variant: "secondary", icon: Rocket },
  enterprise: { label: "Enterprise", variant: "default", icon: Crown },
}

export function PlanBadge() {
  const { plan, isLoading } = usePlan()
  if (isLoading || !plan) return null
  const config = PLAN_CONFIG[plan]
  const Icon = config.icon
  return (
    <Badge variant={config.variant} className="gap-1">
      <Icon className="h-3 w-3" />
      {config.label}
    </Badge>
  )
}
