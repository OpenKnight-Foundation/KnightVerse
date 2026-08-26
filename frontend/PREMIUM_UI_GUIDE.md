# Premium UI Components & Web3 Integration Guide

## Overview

This guide documents the enhanced UI components and fluid Web3 interactions added to XLMate. All components follow the established design patterns with premium animations, accessibility features, and efficient resource utilization.

## 🎨 New UI Components

### Core Components

#### 1. **Card** (`components/ui/card.tsx`)
Premium card component with hover effects and backdrop blur.

```tsx
import { Card, CardHeader, CardTitle, CardDescription, CardContent, CardFooter } from "@/components/ui/card";

<Card>
  <CardHeader>
    <CardTitle>Title</CardTitle>
    <CardDescription>Description</CardDescription>
  </CardHeader>
  <CardContent>
    Content goes here
  </CardContent>
  <CardFooter>
    Footer actions
  </CardFooter>
</Card>
```

#### 2. **Badge** (`components/ui/badge.tsx`)
Status indicators with multiple variants.

```tsx
import { Badge } from "@/components/ui/badge";

<Badge variant="success">Connected</Badge>
<Badge variant="warning">Testnet</Badge>
<Badge variant="destructive">Error</Badge>
<Badge variant="info">Info</Badge>
```

#### 3. **Progress** (`components/ui/progress.tsx`)
Animated progress bar with gradient styling.

```tsx
import { Progress } from "@/components/ui/progress";

<Progress value={60} max={100} />
```

#### 4. **Spinner** (`components/ui/spinner.tsx`)
Loading spinner with size variants.

```tsx
import { Spinner } from "@/components/ui/spinner";

<Spinner size="sm" />
<Spinner size="md" />
<Spinner size="lg" />
```

#### 5. **Alert** (`components/ui/alert.tsx`)
Contextual alert messages.

```tsx
import { Alert, AlertTitle, AlertDescription } from "@/components/ui/alert";

<Alert variant="success">
  <AlertTitle>Success!</AlertTitle>
  <AlertDescription>Your transaction was confirmed.</AlertDescription>
</Alert>
```

#### 6. **Skeleton** (`components/ui/skeleton.tsx`)
Loading skeleton with shimmer effect.

```tsx
import { Skeleton } from "@/components/ui/skeleton";

<Skeleton className="h-12 w-full" />
```

#### 7. **EmptyState** (`components/ui/empty-state.tsx`)
Empty state placeholder with icon and action.

```tsx
import { EmptyState } from "@/components/ui/empty-state";

<EmptyState
  icon="🎮"
  title="No games yet"
  description="Start your first game to see it here"
  action={<Button>Start Game</Button>}
/>
```

#### 8. **StatCard** (`components/ui/stat-card.tsx`)
Statistics display card with trend indicators.

```tsx
import { StatCard } from "@/components/ui/stat-card";

<StatCard
  label="Total Games"
  value={42}
  icon="♟️"
  trend={{ value: 12, isPositive: true }}
/>
```

#### 9. **Tooltip** (`components/ui/tooltip.tsx`)
CSS-only tooltip component.

```tsx
import { Tooltip } from "@/components/ui/tooltip";

<Tooltip content="Click to connect wallet" position="top">
  <Button>Connect</Button>
</Tooltip>
```

## 🌐 Web3 Components

### 1. **WalletButton** (`components/Web3/WalletButton.tsx`)
Premium wallet connection button with status indicators.

**Features:**
- Real-time connection status
- Network badge (Testnet/Mainnet)
- Truncated address display
- Animated status dots
- Opens WalletConnectModal on click

```tsx
import { WalletButton } from "@/components/Web3/WalletButton";

<WalletButton />
```

### 2. **TransactionButton** (`components/Web3/TransactionButton.tsx`)
Smart button with built-in transaction state management.

**Features:**
- Automatic loading states
- Success/error feedback
- Prevents double-clicks
- Smooth state transitions

```tsx
import { TransactionButton } from "@/components/Web3/TransactionButton";

<TransactionButton
  onTransaction={async () => {
    await sendXLM(destination, amount);
  }}
  loadingText="Sending..."
  successText="Sent!"
>
  Send XLM
</TransactionButton>
```

### 3. **EnhancedTransactionStatus** (`components/Web3/EnhancedTransactionStatus.tsx`)
Premium transaction status indicator with progress tracking.

**Features:**
- Real-time transaction lifecycle tracking
- Progress bars for active transactions
- Transaction type badges
- Copy transaction hash
- Auto-dismiss completed transactions
- Error display

```tsx
// Automatically included in layout.tsx
// No manual integration needed
```

**Transaction Phases:**
1. **Preparing** (20%) - Building transaction
2. **Signing** (40%) - Awaiting wallet signature
3. **Submitting** (60%) - Sending to network
4. **Confirming** (80%) - Waiting for confirmation
5. **Confirmed** (100%) - Transaction complete
6. **Failed** - Transaction error

### 4. **EnhancedHeader** (`components/EnhancedHeader.tsx`)
Premium navigation header with Web3 integration.

**Features:**
- Responsive navigation
- Active route indicators
- Integrated wallet button
- Network status badge
- Mobile-friendly menu
- Smooth animations

```tsx
// Automatically included in layout.tsx
// No manual integration needed
```

## 🎭 Animations

All animations are defined in `app/globals.css` and follow these principles:

### Available Animations

- `animate-toast-in` - Toast entrance
- `animate-modal-in` - Modal entrance
- `animate-overlay-in` - Overlay fade in
- `animate-slide-up` - Slide up entrance
- `animate-fade-in` - Fade in
- `animate-scale-in` - Scale in (for cards)
- `animate-pulse-glow` - Pulsing glow effect
- `animate-shimmer` - Shimmer loading effect
- `animate-float` - Floating animation
- `animate-check` - Success checkmark
- `animate-spin-slow` - Slow spin

### Usage Example

```tsx
<div className="animate-slide-up">
  Content with slide-up animation
</div>
```

## 🔧 Hooks

### useTrackedTransaction

Wraps async operations with full transaction lifecycle tracking.

```tsx
import { useTrackedTransaction } from "@/hook/useTrackedTransaction";

const { execute } = useTrackedTransaction({
  type: "payment",
  label: "Send 10 XLM",
  amount: "10",
  destination: "GXXX...",
  autoDismissMs: 8000, // Optional
});

const handleSend = async () => {
  const result = await execute(async (txId) => {
    return await sendXLM(destination, amount);
  });
  
  if (result) {
    // Transaction succeeded
  }
};
```

## 🎨 Design Tokens

### Colors

The design system uses CSS variables defined in `globals.css`:

**Primary Colors:**
- `--primary` - Teal (173 80% 40%)
- `--primary-foreground` - Light text

**Status Colors:**
- Emerald - Success states
- Red - Error states
- Yellow - Warning states
- Blue - Info states

**Gradients:**
- `from-teal-500 to-blue-600` - Primary gradient
- `from-emerald-500 to-emerald-600` - Success gradient
- `from-red-500 to-red-600` - Error gradient

### Spacing

Follow Tailwind's spacing scale:
- `gap-2` (0.5rem) - Tight spacing
- `gap-4` (1rem) - Default spacing
- `gap-6` (1.5rem) - Loose spacing

### Border Radius

- `rounded-lg` - Default (0.5rem)
- `rounded-xl` - Cards (0.75rem)
- `rounded-2xl` - Modals (1rem)
- `rounded-full` - Circles

## ♿ Accessibility

All components follow WCAG 2.1 AA standards:

1. **Semantic HTML** - Proper use of `<button>`, `<nav>`, etc.
2. **ARIA Labels** - All interactive elements have labels
3. **Keyboard Navigation** - Full keyboard support
4. **Focus Indicators** - Visible focus states
5. **Screen Reader Support** - Descriptive text for assistive tech
6. **Color Contrast** - Meets WCAG contrast ratios

### Example

```tsx
<button
  onClick={handleClick}
  aria-label="Connect wallet to start playing"
  className="..."
>
  Connect Wallet
</button>
```

## 🚀 Performance

### Optimization Strategies

1. **CSS-only animations** - No JavaScript overhead
2. **Efficient re-renders** - Context updates only when needed
3. **Lazy loading** - Dynamic imports for heavy components
4. **Memoization** - React.memo for expensive components
5. **Debouncing** - Rate-limited API calls

### Gas Optimization

Transaction components are designed to minimize gas costs:

1. **Transaction batching** - Combine multiple operations
2. **Optimal timing** - Submit during low-traffic periods
3. **Fee estimation** - Calculate fees before submission
4. **Error prevention** - Validate before signing

## 📝 Testing

### Unit Tests

Test components with standard and edge cases:

```tsx
import { render, screen } from '@testing-library/react';
import { Badge } from '@/components/ui/badge';

test('renders badge with correct variant', () => {
  render(<Badge variant="success">Connected</Badge>);
  expect(screen.getByText('Connected')).toBeInTheDocument();
});
```

### Integration Tests

Test Web3 flows end-to-end:

```tsx
test('wallet connection flow', async () => {
  render(<WalletButton />);
  const button = screen.getByRole('button');
  fireEvent.click(button);
  // Assert modal opens
  // Assert connection succeeds
});
```

## 🎯 Best Practices

1. **Always use TypeScript** - Type safety prevents bugs
2. **Follow component patterns** - Consistent API across components
3. **Use semantic HTML** - Better accessibility and SEO
4. **Optimize images** - Use Next.js Image component
5. **Handle errors gracefully** - Show user-friendly messages
6. **Test on mobile** - Responsive design is critical
7. **Monitor performance** - Use React DevTools Profiler
8. **Document changes** - Update this guide when adding features

## 🔄 Migration Guide

### Updating Existing Components

Replace old components with new ones:

**Before:**
```tsx
<div className="bg-gray-800 p-4 rounded">
  <h3>Title</h3>
  <p>Content</p>
</div>
```

**After:**
```tsx
<Card>
  <CardHeader>
    <CardTitle>Title</CardTitle>
  </CardHeader>
  <CardContent>
    <p>Content</p>
  </CardContent>
</Card>
```

### Updating Transaction Handling

**Before:**
```tsx
const [loading, setLoading] = useState(false);

const handleSend = async () => {
  setLoading(true);
  try {
    await sendXLM(dest, amount);
    toast.success("Sent!");
  } catch (err) {
    toast.error("Failed");
  } finally {
    setLoading(false);
  }
};
```

**After:**
```tsx
const { execute } = useTrackedTransaction({
  type: "payment",
  label: "Send XLM",
  amount,
  destination: dest,
});

const handleSend = async () => {
  await execute(async () => {
    return await sendXLM(dest, amount);
  });
};
```

## 📚 Resources

- [Tailwind CSS Documentation](https://tailwindcss.com/docs)
- [Radix UI Documentation](https://www.radix-ui.com/docs)
- [Stellar SDK Documentation](https://stellar.github.io/js-stellar-sdk/)
- [Next.js Documentation](https://nextjs.org/docs)
- [WCAG Guidelines](https://www.w3.org/WAI/WCAG21/quickref/)

## 🤝 Contributing

When adding new components:

1. Follow the existing patterns
2. Add TypeScript types
3. Include accessibility features
4. Add animations where appropriate
5. Document in this guide
6. Write tests
7. Update examples

---

**Last Updated:** April 2026
**Version:** 1.0.0
