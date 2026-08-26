import * as React from "react";
import { Card, CardContent } from "@/components/ui/card";
import { cn } from "@/lib/utils";

interface StatCardProps {
  label: string;
  value: string | number;
  icon?: React.ReactNode;
  trend?: {
    value: number;
    isPositive: boolean;
  };
  className?: string;
}

/**
 * StatCard — Premium statistics card component
 * Displays key metrics with optional trend indicators
 */
export function StatCard({
  label,
  value,
  icon,
  trend,
  className,
}: StatCardProps) {
  return (
    <Card className={cn("hover:scale-[1.02] transition-transform duration-300", className)}>
      <CardContent className="p-6">
        <div className="flex items-start justify-between">
          <div className="flex-1">
            <p className="text-sm text-gray-400 mb-1">{label}</p>
            <p className="text-3xl font-bold text-white">{value}</p>
            {trend && (
              <div
                className={cn(
                  "flex items-center gap-1 mt-2 text-sm font-medium",
                  trend.isPositive ? "text-emerald-400" : "text-red-400"
                )}
              >
                <svg
                  className={cn(
                    "h-4 w-4",
                    !trend.isPositive && "rotate-180"
                  )}
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth={2}
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    d="M5 10l7-7m0 0l7 7m-7-7v18"
                  />
                </svg>
                <span>{Math.abs(trend.value)}%</span>
              </div>
            )}
          </div>
          {icon && (
            <div className="p-3 rounded-xl bg-gradient-to-br from-teal-500/20 to-blue-600/20 text-2xl">
              {icon}
            </div>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
