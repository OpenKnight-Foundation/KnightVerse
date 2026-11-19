import Link from "next/link";

export default function PlayPage() {
  return (
    <main className="min-h-screen p-6 text-white" aria-label="Play chess">
      <div className="mx-auto max-w-3xl space-y-6">
        <h1 className="text-3xl font-extrabold text-teal-300">Play Chess</h1>
        <p className="text-sm text-gray-400">
          Choose a game mode to start playing.
        </p>
        <div className="grid gap-4 sm:grid-cols-2">
          <Link
            href="/offline"
            className="rounded-xl border border-gray-700 bg-gray-900 p-5 transition-colors hover:border-teal-400 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-teal-400"
          >
            <h2 className="text-lg font-bold">Play offline</h2>
            <p className="mt-1 text-sm text-gray-400">
              Practise against the computer.
            </p>
          </Link>
          <Link
            href="/"
            className="rounded-xl border border-gray-700 bg-gray-900 p-5 transition-colors hover:border-teal-400 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-teal-400"
          >
            <h2 className="text-lg font-bold">Play online</h2>
            <p className="mt-1 text-sm text-gray-400">
              Find an opponent in the lobby.
            </p>
          </Link>
        </div>
      </div>
    </main>
  );
}
