import {
  CastleIcon as ChessKnight,
  Puzzle,
  BookOpen,
  Eye,
  Newspaper,
  User,
  MoreHorizontal,
  ChevronLeft,
  Settings,
  LifeBuoy,
  Wallet,
  Menu,
} from "lucide-react"

export function ChessIcon() {
  return <ChessKnight size={20} />
}

export function PuzzleIcon() {
  return <Puzzle size={20} />
}

export function LearnIcon() {
  return <BookOpen size={20} />
}

export function WatchIcon() {
  return <Eye size={20} />
}

export function DashboardIcon() {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      className="h-5 w-5"
      fill="none"
      viewBox="0 0 24 24"
      stroke="currentColor"
      strokeWidth={2}
    >
      <path strokeLinecap="round" strokeLinejoin="round" d="M3 13h4v8H3zM10 9h4v12h-4zM17 5h4v16h-4z" />
    </svg>
  );
}

export function NewsIcon() {
  return <Newspaper size={20} />
}

export function UserIcon() {
  return <User size={20} />
}

export function MoreIcon() {
  return <MoreHorizontal size={20} />
}

export function CollapseIcon() {
  return <ChevronLeft size={20} />
}

export function SettingsIcon() {
  return <Settings size={20} />
}

export function SupportIcon() {
  return <LifeBuoy size={20} />
}

export function WalletIcon() {
  return <Wallet size={20} />
}

export function MenuIcon() {
  return <Menu size={24} />
}
