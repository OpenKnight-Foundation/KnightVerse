"use client"

import React from 'react'
import Link from 'next/link'
import { MenuIcon, UserIcon } from './icons'
import { Button } from './ui/button'
import { useTheme } from 'next-themes'
import { Sheet, SheetContent, SheetTrigger } from './ui/sheet'
import { GameSidebar } from './GameSidebar'
import { MoonIcon, SunIcon } from 'lucide-react'
import { useAuth } from '../context/authContext'
import { SignInModal } from './SignInModal'

export function Header() {
    const { theme, setTheme } = useTheme()
    const [collapsed, setCollapsed] = React.useState(false)
    const { user } = useAuth()
    const [signInOpen, setSignInOpen] = React.useState(false)

    return (
        <header className="h-16 border-b border-border bg-card/80 backdrop-blur-sm fixed top-0 left-0 right-0 z-50" role="banner">
            <div className="container h-full flex items-center justify-between">
                <div className="flex items-center gap-4">
                    <Sheet>
                        <SheetTrigger asChild>
                            <Button variant="ghost" size="icon" className="md:hidden" aria-label="Open navigation menu">
                                <MenuIcon />
                            </Button>
                        </SheetTrigger>
                        <SheetContent side="left" className="p-0 w-64">
                            <GameSidebar collapsed={collapsed} setCollapsed={setCollapsed} />
                            {!user && (
                                <div className="p-4 border-t border-border">
                                    <Button className="w-full" onClick={() => setSignInOpen(true)}>
                                        Sign In
                                    </Button>
                                </div>
                            )}
                        </SheetContent>
                    </Sheet>

                    <Link href="/" className="flex items-center space-x-2">
                        <div className="relative h-8 w-8">
                            <div className="absolute inset-0 bg-blue-500 rounded-full blur-sm opacity-30 animate-pulse"></div>
                            <svg viewBox="0 0 32 32" fill="none" xmlns="http://www.w3.org/2000/svg" className="relative z-10">
                                <path d="M16 3L22 7L28 5L24 12L28 19L22 17L16 21L16 14L10 17L4 19L8 12L4 5L10 7L16 3Z" fill="#3AA0CD" />
                                <path d="M16 3L10 7L4 5L8 12L4 19L10 17L16 21L16 14L16 3Z" fill="#2C8CB5" />
                                <circle cx="16" cy="12" r="2" fill="white" />
                            </svg>
                        </div>
                        <span className="font-bold text-xl bg-clip-text text-transparent bg-gradient-to-r from-blue-400 to-blue-600">
                            XLMATE
                        </span>
                    </Link>
                </div>

                <div className="flex items-center space-x-2">
                    <Button variant="ghost" size="icon" onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')} aria-label={`Switch to ${theme === 'dark' ? 'light' : 'dark'} mode`}>
                        {theme === 'dark' ? <SunIcon className="h-5 w-5" /> : <MoonIcon className="h-5 w-5" />}
                    </Button>

                    {user ? (
                        <Button variant="ghost" size="sm" aria-label="User profile">
                            <UserIcon />
                            <span data-testid="header-username">{user.username}</span>
                        </Button>
                    ) : (
                        <Button variant="ghost" size="sm" onClick={() => setSignInOpen(true)} aria-label="Sign in" data-testid="header-sign-in">
                            Sign In
                        </Button>
                    )}
                </div>
            </div>
            <SignInModal open={signInOpen} onOpenChange={setSignInOpen} />
        </header>
    )
}