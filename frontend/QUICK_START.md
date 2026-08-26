# Quick Start Guide - Premium UI Components

## 🚀 Getting Started

This guide will help you quickly integrate the new premium UI components and Web3 features into your XLMate development workflow.

## 📦 What's New

- **15 Premium UI Components** - Cards, badges, progress bars, alerts, and more
- **4 Web3 Components** - Wallet button, transaction button, status tracker, enhanced header
- **11 Custom Animations** - Smooth, performant CSS animations
- **Enhanced Utilities** - Helper functions for formatting, validation, and more

## 🎯 Common Use Cases

### 1. Display a Card with Stats

```tsx
import { Card, CardHeader, CardTitle, CardContent } from "@/components/ui/card";
import { StatCard } from "@/components/ui/stat-card";

export function GameStats() {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Your Stats</CardTitle>
      </CardHeader>
      <CardContent className="grid grid-cols-2 gap-4">
        <StatCard
          label="Total Games"
          value={42}
          icon="♟️"
          trend={{ value: 12, isPositive: true }}
        />
        <StatCard
          label="Win Rate"
          value="68%"
          icon="🏆"
          trend={{ value: 5, isPositive: true }}
        />
      </CardContent>
    </Card>
  );
}
```

### 2. Show Loading State

```tsx
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";

export function LoadingCard() {
  return (
    <div className="space-y-4">
      {/* Skeleton for content */}
      <Skeleton className="h-12 w-full" />
      <Skeleton className="h-8 w-3/4" />
      
      {/* Or use spinner */}
      <div className="flex justify-center">
        <Spinner size="lg" />
      </div>
    </div>
  );
}
```

### 3. Display Status Badges

```tsx
import { Badge } from "@/components/ui/badge";

export function GameStatus({ status }: { status: string }) {
  const variants = {
    active: "success",
    pending: "warning",
    completed: "info",
    failed: "destructive",
  } as const;

  return (
    <Badge variant={variants[status] || "default"}>
      {status}
    </Badge>
  );
}
```

### 4. Show Empty State

```tsx
import { EmptyState } from "@/components/ui/empty-state";
import { Button } from "@/components/ui/button";

export function NoGames() {
  return (
    <EmptyState
      icon="🎮"
      title="No games yet"
      description="Start your first game to see it here"
      action={
        <Button onClick={() => router.push("/play")}>
          Start Playing
        </Button>
      }
    />
  );
}
```

### 5. Handle Transactions

```tsx
import { TransactionButton } from "@/components/Web3/TransactionButton";
import { useTrackedTransaction } from "@/hook/useTrackedTransaction";
import { useAppContext } from "@/context/walletContext";

export function SendPayment() {
  const { sendXLM } = useAppContext();
  const { execute } = useTrackedTransaction({
    type: "payment",
    label: "Send 10 XLM",
    amount: "10",
    destination: "GXXX...",
  });

  return (
    <TransactionButton
      onTransaction={async () => {
        await execute(async () => {
          return await sendXLM("GXXX...", "10");
        });
      }}
      loadingText="Sending..."
      successText="Sent!"
    >
      Send 10 XLM
    </TransactionButton>
  );
}
```

### 6. Show Alerts

```tsx
import { Alert, AlertTitle, AlertDescription } from "@/components/ui/alert";

export function Notifications() {
  return (
    <div className="space-y-4">
      <Alert variant="success">
        <AlertTitle>Success!</AlertTitle>
        <AlertDescription>
          Your transaction was confirmed on the blockchain.
        </AlertDescription>
      </Alert>

      <Alert variant="warning">
        <AlertTitle>Warning</AlertTitle>
        <AlertDescription>
          You're on the testnet. Use test XLM only.
        </AlertDescription>
      </Alert>

      <Alert variant="destructive">
        <AlertTitle>Error</AlertTitle>
        <AlertDescription>
          Transaction failed. Please try again.
        </AlertDescription>
      </Alert>
    </div>
  );
}
```

### 7. Add Progress Tracking

```tsx
import { Progress } from "@/components/ui/progress";
import { useState, useEffect } from "react";

export function GameProgress() {
  const [progress, setProgress] = useState(0);

  useEffect(() => {
    const timer = setInterval(() => {
      setProgress((prev) => (prev >= 100 ? 0 : prev + 10));
    }, 500);
    return () => clearInterval(timer);
  }, []);

  return (
    <div className="space-y-2">
      <div className="flex justify-between text-sm">
        <span>Loading game...</span>
        <span>{progress}%</span>
      </div>
      <Progress value={progress} max={100} />
    </div>
  );
}
```

### 8. Use Tooltips

```tsx
import { Tooltip } from "@/components/ui/tooltip";
import { Button } from "@/components/ui/button";

export function HelpButton() {
  return (
    <Tooltip content="Click to view game rules" position="top">
      <Button variant="ghost" size="icon">
        <span>?</span>
      </Button>
    </Tooltip>
  );
}
```

## 🛠️ Utility Functions

### Format Addresses

```tsx
import { truncateAddress } from "@/lib/utils";

const address = "GXXX...XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX";
const short = truncateAddress(address); // "GXXX...XXXX"
```

### Format Numbers

```tsx
import { formatNumber, formatXLM } from "@/lib/utils";

const games = formatNumber(1234567); // "1,234,567"
const amount = formatXLM(10.5); // "10.50 XLM"
```

### Format Time

```tsx
import { formatRelativeTime, formatDuration } from "@/lib/utils";

const timestamp = Date.now() - 3600000; // 1 hour ago
const relative = formatRelativeTime(timestamp); // "1 hour ago"

const duration = formatDuration(150000); // "2m 30s"
```

### Validate Addresses

```tsx
import { isValidStellarAddress } from "@/lib/utils";

const isValid = isValidStellarAddress("GXXX..."); // true/false
```

### Copy to Clipboard

```tsx
import { copyToClipboard } from "@/lib/utils";
import { useToast } from "@/components/ui/toast";

export function CopyButton({ text }: { text: string }) {
  const { addToast } = useToast();

  const handleCopy = async () => {
    await copyToClipboard(text);
    addToast({
      severity: "success",
      title: "Copied!",
      detail: "Address copied to clipboard",
    });
  };

  return <button onClick={handleCopy}>Copy</button>;
}
```

## 🎨 Styling Tips

### Use Consistent Spacing

```tsx
// Good - consistent spacing
<div className="space-y-4">
  <Card />
  <Card />
</div>

// Good - consistent padding
<Card className="p-6">
  <CardContent className="pt-0" />
</Card>
```

### Use Gradient Backgrounds

```tsx
// Primary gradient
<div className="bg-gradient-to-r from-teal-500 to-blue-600" />

// Success gradient
<div className="bg-gradient-to-r from-emerald-500 to-emerald-600" />

// Error gradient
<div className="bg-gradient-to-r from-red-500 to-red-600" />
```

### Add Hover Effects

```tsx
// Scale on hover
<Card className="hover:scale-[1.02] transition-transform duration-300" />

// Glow on hover
<Button className="hover:shadow-lg hover:shadow-teal-500/50" />
```

### Use Animations

```tsx
// Slide up entrance
<div className="animate-slide-up">Content</div>

// Fade in
<div className="animate-fade-in">Content</div>

// Scale in
<div className="animate-scale-in">Content</div>
```

## 🔧 Configuration

### Tailwind Config

The design system uses these key values:

```typescript
// Colors
colors: {
  primary: "hsl(173 80% 40%)", // Teal
  teal: { 400, 500, 600, 700 },
  blue: { 400, 500, 600 },
  emerald: { 400, 500, 600 },
  // ... more colors
}

// Border Radius
borderRadius: {
  lg: "0.5rem",
  xl: "0.75rem",
  "2xl": "1rem",
}
```

### Animation Timing

```css
/* Fast animations */
duration-200 /* 200ms */

/* Default animations */
duration-300 /* 300ms */

/* Slow animations */
duration-500 /* 500ms */
```

## 🐛 Troubleshooting

### Component Not Rendering

1. Check imports are correct
2. Verify component is wrapped in required providers
3. Check for TypeScript errors
4. Verify Tailwind classes are being applied

### Animations Not Working

1. Ensure `globals.css` is imported in layout
2. Check `tailwindcss-animate` is installed
3. Verify animation classes are correct
4. Check for conflicting CSS

### Web3 Features Not Working

1. Verify Freighter wallet is installed
2. Check wallet is unlocked
3. Verify network is correct (testnet/mainnet)
4. Check console for errors

### Styling Issues

1. Clear Next.js cache: `rm -rf .next`
2. Rebuild: `npm run build`
3. Check Tailwind config is correct
4. Verify CSS variables are defined

## 📚 Additional Resources

- **Full Documentation**: See `PREMIUM_UI_GUIDE.md`
- **Implementation Details**: See `UI_ENHANCEMENT_SUMMARY.md`
- **Testing Guide**: See `IMPLEMENTATION_CHECKLIST.md`

## 💡 Pro Tips

1. **Use TypeScript** - Get autocomplete and type safety
2. **Compose Components** - Build complex UIs from simple components
3. **Keep It Accessible** - Always add ARIA labels
4. **Test Responsively** - Check on mobile, tablet, and desktop
5. **Optimize Performance** - Use React.memo for expensive components
6. **Follow Patterns** - Look at existing components for examples
7. **Document Changes** - Update docs when adding features

## 🎯 Next Steps

1. Explore the component library in `components/ui/`
2. Check out Web3 components in `components/Web3/`
3. Read the full guide in `PREMIUM_UI_GUIDE.md`
4. Start building your feature!

## 🤝 Need Help?

- Check the documentation files
- Look at existing component implementations
- Review TypeScript types for prop definitions
- Test in development environment first

---

**Happy Coding! 🚀**
