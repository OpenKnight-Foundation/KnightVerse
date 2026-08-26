import * as React from "react";
import { cn } from "@/lib/utils";

interface TooltipProps {
  content: string;
  children: React.ReactNode;
  position?: "top" | "bottom" | "left" | "right";
}

/**
 * Tooltip — Simple CSS-only tooltip component
 * Lightweight and accessible with ARIA labels
 */
export function Tooltip({
  content,
  children,
  position = "top",
}: TooltipProps) {
  const positionClasses = {
    top: "bottom-full left-1/2 -translate-x-1/2 mb-2",
    bottom: "top-full left-1/2 -translate-x-1/2 mt-2",
    left: "right-full top-1/2 -translate-y-1/2 mr-2",
    right: "left-full top-1/2 -translate-y-1/2 ml-2",
  };

  return (
    <div className="relative inline-flex group">
      <div aria-label={content}>{children}</div>
      <div
        className={cn(
          "absolute z-50 px-2 py-1 text-xs font-medium text-white bg-gray-900 rounded-lg shadow-lg border border-gray-700/50 whitespace-nowrap pointer-events-none opacity-0 group-hover:opacity-100 transition-opacity duration-200",
          positionClasses[position]
        )}
        role="tooltip"
      >
        {content}
        <div
          className={cn(
            "absolute w-2 h-2 bg-gray-900 border-gray-700/50 transform rotate-45",
            position === "top" && "bottom-[-4px] left-1/2 -translate-x-1/2 border-b border-r",
            position === "bottom" && "top-[-4px] left-1/2 -translate-x-1/2 border-t border-l",
            position === "left" && "right-[-4px] top-1/2 -translate-y-1/2 border-r border-t",
            position === "right" && "left-[-4px] top-1/2 -translate-y-1/2 border-l border-b"
          )}
        />
      </div>
    </div>
  );
}
