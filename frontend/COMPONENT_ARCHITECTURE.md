# Component Architecture

> **Note (FE-126 / FE-127 / FE-129):** Several components this guide once
> described were deleted as dead code — `ui/card`, `ui/progress`, `ui/spinner`,
> `ui/skeleton`, `ui/stat-card`, `ui/tooltip`, `ui/DotPulseLoader`,
> `Web3/TransactionButton`, `Web3/EnhancedTransactionStatus` and
> `ui/ToastNotificationCenter`. The live replacements are
> `ui/LoadingSkeleton`, `components/dashboard/StatCard`,
> `components/TransactionStatusIndicator`, `components/Web3StatusBar`,
> `hook/useTrackedTransaction` and the `ui/toast` context. Examples below that
> import the removed files are kept only as historical notes.

## 🏗️ System Overview

```
┌─────────────────────────────────────────────────────────────┐
│                        XLMate Frontend                       │
│                     (Next.js 15 + React 19)                  │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                      Root Layout (layout.tsx)                │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  Theme Provider (Dark Mode)                            │ │
│  │  ┌──────────────────────────────────────────────────┐ │ │
│  │  │  App Provider (Wallet Context)                   │ │ │
│  │  │  ┌────────────────────────────────────────────┐ │ │ │
│  │  │  │  Matchmaking Provider                      │ │ │ │
│  │  │  │  ┌──────────────────────────────────────┐ │ │ │ │
│  │  │  │  │  Toast Provider                      │ │ │ │ │
│  │  │  │  │  ┌────────────────────────────────┐ │ │ │ │ │
│  │  │  │  │  │  Transaction Provider          │ │ │ │ │ │
│  │  │  │  │  │                                │ │ │ │ │ │
│  │  │  │  │  │  ┌──────────────────────────┐ │ │ │ │ │ │
│  │  │  │  │  │  │  EnhancedHeader          │ │ │ │ │ │ │
│  │  │  │  │  │  └──────────────────────────┘ │ │ │ │ │ │
│  │  │  │  │  │                                │ │ │ │ │ │
│  │  │  │  │  │  ┌──────────────────────────┐ │ │ │ │ │ │
│  │  │  │  │  │  │  Main Content (Pages)    │ │ │ │ │ │ │
│  │  │  │  │  │  └──────────────────────────┘ │ │ │ │ │ │
│  │  │  │  │  │                                │ │ │ │ │ │
│  │  │  │  │  │  ┌──────────────────────────┐ │ │ │ │ │ │
│  │  │  │  │  │  │  EnhancedTransactionStatus│ │ │ │ │ │
│  │  │  │  │  │  └──────────────────────────┘ │ │ │ │ │ │
│  │  │  │  │  └────────────────────────────────┘ │ │ │ │ │
│  │  │  │  └──────────────────────────────────────┘ │ │ │ │
│  │  │  └────────────────────────────────────────────┘ │ │ │
│  │  └──────────────────────────────────────────────────┘ │ │
│  └────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## 📦 Component Hierarchy

### UI Components Layer
```
components/ui/
├── card.tsx                 (Container component)
│   ├── Card
│   ├── CardHeader
│   ├── CardTitle
│   ├── CardDescription
│   ├── CardContent
│   └── CardFooter
│
├── badge.tsx               (Status indicator)
│   └── Badge (7 variants)
│
├── progress.tsx            (Progress bar)
│   └── Progress
│
├── spinner.tsx             (Loading indicator)
│   └── Spinner (3 sizes)
│
├── alert.tsx               (Alert messages)
│   ├── Alert (5 variants)
│   ├── AlertTitle
│   └── AlertDescription
│
├── skeleton.tsx            (Loading skeleton)
│   └── Skeleton
│
├── button.tsx              (Action button)
│   └── Button
│
├── toast.tsx               (Notifications)
│   ├── ToastProvider
│   ├── ToastContainer
│   └── useToast
│
├── empty-state.tsx         (Empty placeholder)
│   └── EmptyState
│
├── stat-card.tsx           (Statistics display)
│   └── StatCard
│
├── tooltip.tsx             (Hover tooltip)
│   └── Tooltip
│
├── input.tsx               (Form input)
│   └── Input
│
├── sheet.tsx               (Side panel)
│   └── Sheet
│
└── LoadingSkeleton.tsx     (Legacy skeleton)
    └── LoadingSkeleton
```

### Web3 Components Layer
```
components/Web3/
├── WalletButton.tsx        (Wallet connection)
│   ├── Uses: useAppContext
│   ├── Opens: WalletConnectModal
│   └── Shows: Badge, Button
│
├── TransactionButton.tsx   (Smart transaction button)
│   ├── Uses: Button, Spinner
│   └── Manages: Transaction states
│
└── EnhancedTransactionStatus.tsx (Transaction tracker)
    ├── Uses: useTransactionContext
    ├── Shows: Card, Badge, Progress
    └── Tracks: Transaction lifecycle
```

### Layout Components
```
components/
├── EnhancedHeader.tsx      (Navigation header)
│   ├── Uses: WalletButton, Badge
│   ├── Shows: Navigation, Logo
│   └── Responsive: Mobile menu
│
├── WalletConnectModal.tsx  (Wallet modal)
│   ├── Uses: Dialog, Button, Badge
│   ├── Shows: Connection UI
│   └── Handles: Freighter integration
│
└── ClientRoot.tsx          (Client wrapper)
    └── Wraps: Page content
```

## 🔄 Data Flow

### Wallet Connection Flow
```
User Click
    │
    ▼
WalletButton
    │
    ▼
WalletConnectModal
    │
    ▼
useAppContext.connectWallet()
    │
    ▼
Freighter API
    │
    ▼
Update Context State
    │
    ▼
Re-render Components
    │
    ▼
Show Connected State
```

### Transaction Flow
```
User Action
    │
    ▼
TransactionButton
    │
    ▼
useTrackedTransaction.execute()
    │
    ├─► startTransaction() ──► Phase: Preparing (20%)
    │
    ├─► updatePhase() ──────► Phase: Signing (40%)
    │   └─► Freighter Sign
    │
    ├─► updatePhase() ──────► Phase: Submitting (60%)
    │   └─► Submit to Network
    │
    ├─► updatePhase() ──────► Phase: Confirming (80%)
    │   └─► Wait for Confirmation
    │
    └─► updatePhase() ──────► Phase: Confirmed (100%)
        └─► Auto-dismiss after 8s
```

### Context Hierarchy
```
ThemeProvider (next-themes)
    │
    ├─► AppProvider (Wallet Context)
    │   ├─► address: string | undefined
    │   ├─► status: "connected" | "disconnected" | "connecting" | "error"
    │   ├─► connectWallet()
    │   ├─► disconnectWallet()
    │   ├─► sendXLM()
    │   └─► invokeSorobanContract()
    │
    ├─► MatchmakingProvider
    │   ├─► status: string
    │   ├─► playerColor: string
    │   ├─► gameId: string
    │   └─► joinMatchmaking()
    │
    ├─► ToastProvider
    │   ├─► toasts: ToastItem[]
    │   ├─► addToast()
    │   └─► removeToast()
    │
    └─► TransactionProvider
        ├─► transactions: TransactionRecord[]
        ├─► startTransaction()
        ├─► updatePhase()
        ├─► dismissTransaction()
        └─► clearResolved()
```

## 🎨 Styling Architecture

### Tailwind Configuration
```
tailwind.config.ts
├── Colors
│   ├── Primary (Teal)
│   ├── Secondary
│   ├── Destructive (Red)
│   ├── Success (Emerald)
│   ├── Warning (Yellow)
│   └── Info (Blue)
│
├── Border Radius
│   ├── lg (0.5rem)
│   ├── xl (0.75rem)
│   └── 2xl (1rem)
│
└── Animations
    └── From tailwindcss-animate
```

### Global Styles
```
app/globals.css
├── CSS Variables (HSL colors)
├── Font Configuration (Rowdies)
├── Custom Animations (11 total)
│   ├── toast-in
│   ├── modal-in
│   ├── overlay-in
│   ├── slide-up
│   ├── fade-in
│   ├── scale-in
│   ├── pulse-glow
│   ├── shimmer
│   ├── float
│   ├── check
│   └── spin-slow
│
└── Scrollbar Styling
```

## 🔧 Utility Layer

### Helper Functions
```
lib/utils.ts
├── cn()                    (Class name merger)
├── truncateAddress()       (Address formatting)
├── formatNumber()          (Number formatting)
├── formatXLM()             (XLM formatting)
├── formatRelativeTime()    (Time formatting)
├── formatDuration()        (Duration formatting)
├── isValidStellarAddress() (Address validation)
├── copyToClipboard()       (Clipboard utility)
├── debounce()              (Function debouncing)
├── throttle()              (Function throttling)
├── sleep()                 (Async sleep)
└── generateId()            (ID generation)
```

### Custom Hooks
```
hook/
├── useTrackedTransaction.ts (Transaction tracking)
├── useChessSocket.ts        (WebSocket connection)
├── useMatchmaking.ts        (Matchmaking logic)
└── useToast.ts              (Toast notifications)
```

## 📱 Responsive Breakpoints

```
Mobile First Approach:

Default (Mobile)
    │  < 768px
    │  Full width
    │  Stacked layout
    │  Mobile menu
    │
    ▼
Tablet (md)
    │  ≥ 768px
    │  2-column layout
    │  Expanded navigation
    │
    ▼
Desktop (lg)
    │  ≥ 1024px
    │  3-column layout
    │  Full navigation
    │
    ▼
Large Desktop (xl)
    │  ≥ 1280px
    │  Max width container
    │  Optimized spacing
    │
    ▼
Extra Large (2xl)
    │  ≥ 1536px
    │  Centered content
    │  Maximum 1400px
```

## 🎯 Component Dependencies

### Core Dependencies
```
Card
├── cn (utils)
└── React

Badge
├── cn (utils)
├── cva (class-variance-authority)
└── React

Progress
├── cn (utils)
└── React

WalletButton
├── useAppContext
├── WalletConnectModal
├── Button
├── Badge
└── React

TransactionButton
├── Button
├── Spinner
├── cn (utils)
└── React

EnhancedTransactionStatus
├── useTransactionContext
├── Card
├── Badge
├── Progress
└── React

EnhancedHeader
├── usePathname (next/navigation)
├── WalletButton
├── Badge
├── cn (utils)
└── React
```

## 🔐 Security Architecture

```
Security Layers:

1. Client-Side Validation
   ├── Address validation
   ├── Amount validation
   └── Input sanitization

2. Wallet Integration
   ├── Freighter API
   ├── User confirmation
   └── Signature verification

3. Network Layer
   ├── HTTPS only
   ├── CORS configuration
   └── CSP headers

4. Error Handling
   ├── Try-catch blocks
   ├── User-friendly messages
   └── Error logging
```

## 📊 Performance Optimization

```
Optimization Strategy:

1. Code Splitting
   ├── Dynamic imports
   ├── Route-based splitting
   └── Component lazy loading

2. Rendering Optimization
   ├── React.memo
   ├── useMemo
   ├── useCallback
   └── Context selectors

3. Asset Optimization
   ├── CSS-only animations
   ├── Optimized images
   ├── Font optimization
   └── Bundle size monitoring

4. Network Optimization
   ├── API debouncing
   ├── Request caching
   ├── Parallel requests
   └── Error retry logic
```

## 🧪 Testing Strategy

```
Testing Pyramid:

E2E Tests (Few)
    │  Complete user flows
    │  Critical paths
    │  Cross-browser
    │
    ▼
Integration Tests (Some)
    │  Component interactions
    │  Context integration
    │  API integration
    │
    ▼
Unit Tests (Many)
    │  Component rendering
    │  Utility functions
    │  Hook behavior
    │
    ▼
Static Analysis (Always)
    │  TypeScript
    │  ESLint
    │  Prettier
```

---

**Last Updated**: April 23, 2026
**Version**: 1.0.0
