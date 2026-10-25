"use client"

import React from 'react'
import Link from 'next/link'
import Image from 'next/image'
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

                    <Link href="/" className="flex items-center space-x-2" aria-label="KnightVerse home">
                        <Image
                            src="/images/KnightVerseLogo.png"
                            alt="KnightVerse logo"
                            width={32}
                            height={32}
                            className="h-8 w-8 object-contain"
                            priority
                        />
                        <span className="font-bold text-xl bg-clip-text text-transparent bg-gradient-to-r from-blue-400 to-blue-600">
                            KnightVerse
                        </span>
                    </Link>
                </div>

                <div className="flex items-center space-x-2">
                    <Button variant="ghost" size="icon" onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')} aria-label={`Switch to ${theme === 'dark' ? 'light' : 'dark'} mode`}>
                        {theme === 'dark' ? <SunIcon className="h-5 w-5" /> : <MoonIcon className="h-5 w-5" />}
                    </Button>

                    {user ? (
                        <Link href="/profile" aria-label="User profile">
                            <Button variant="ghost" size="icon">
                                <UserIcon />
                            </Button>
                        </Link>
                    ) : (
                        <Button variant="ghost" size="sm" onClick={() => setSignInOpen(true)} aria-label="Sign in">
                            Sign In
                        </Button>
                    )}
                </div>
            </div>
            <SignInModal open={signInOpen} onOpenChange={setSignInOpen} />
        </header>
    )
}