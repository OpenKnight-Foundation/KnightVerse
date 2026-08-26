import { cn } from "@/lib/utils";

function Skeleton({
  className,
  ...props
}: React.HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn(
        "animate-shimmer shimmer-bg rounded-lg",
        className
      )}
      {...props}
    />
  );
}

export { Skeleton };
