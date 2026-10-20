import Link from "next/link";

/**
 * Branded 404 page (FE-118 / #1359). Without this file Next.js renders its
 * default 404, which drops users out of the app shell entirely.
 */
export default function NotFound() {
  return (
    <div
      className="min-h-screen flex flex-col items-center justify-center px-4 text-white"
      role="main"
      aria-label="Page not found"
    >
      <p className="text-7xl mb-4" aria-hidden="true">
        ♞
      </p>
      <h1 className="text-3xl font-extrabold bg-gradient-to-r from-teal-400 to-blue-500 bg-clip-text text-transparent">
        404 — Piece not found
      </h1>
      <p className="text-gray-400 text-sm mt-3 max-w-md text-center">
        This square is empty. The page you were looking for has been captured or
        never existed.
      </p>
      <Link
        href="/"
        className="mt-6 px-5 py-2.5 rounded-xl bg-gradient-to-r from-teal-500 to-blue-700 hover:from-teal-600 hover:to-blue-800 text-white font-semibold shadow-lg transition-all"
      >
        Back to Lobby
      </Link>
    </div>
  );
}
