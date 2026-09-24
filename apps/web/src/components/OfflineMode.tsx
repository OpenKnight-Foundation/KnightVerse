import React, { useEffect, useState } from 'react';

const AUDIO_EVENTS = ['move', 'capture', 'check', 'castle', 'promote', 'low_time_alarm', 'victory', 'defeat'] as const;
const AUDIO_PACKS = ['classic-wood', 'marble', 'cyberpunk', 'retro-8bit'] as const;
const AUDIO_BASE_PATH = '/sounds';

export const OfflineModeBanner: React.FC = () => {
  const [isOnline, setIsOnline] = useState(navigator.onLine);

  useEffect(() => {
    window.addEventListener('online', () => setIsOnline(true));
    window.addEventListener('offline', () => setIsOnline(false));
  }, []);

  if (isOnline) return null;

  return (
    <div className="fixed top-0 left-0 right-0 bg-yellow-500 text-white p-3 text-center">
      ❤️ Offline mode: Puzzles and audio cached locally
    </div>
  );
};

export async function registerServiceWorker() {
  if ('serviceWorker' in navigator) {
    try {
      await navigator.serviceWorker.register('/sw.js');
      await cacheOfflinePuzzles();
    } catch (error) {
      console.error('SW registration failed:', error);
    }
  }
}

async function cacheOfflinePuzzles() {
  const cache = await caches.open('daily-puzzles-v1');
  const puzzles = await fetch('/api/puzzles/daily').then(r => r.json());
  const urls = puzzles.slice(0, 100).map((p: any) => `/api/puzzles/${p.id}`);
  for (const url of urls) {
    const response = await fetch(url);
    await cache.put(url, response.clone());
  }
  await cacheAudioAssets();
}

async function cacheAudioAssets() {
  const cache = await caches.open('audio-v1');
  const assets = AUDIO_PACKS.flatMap(pack =>
    AUDIO_EVENTS.map(event => `${AUDIO_BASE_PATH}/${pack}/${event}.mp3`)
  );

  await Promise.all(
    assets.map(async url => {
      try {
        const response = await fetch(url);
        if (response.ok) {
          await cache.put(url, response.clone());
        } else {
          console.warn(`Audio asset not found: ${url}`);
        }
      } catch (error) {
        console.warn(`Failed to cache audio asset: ${url}`, error);
      }
    })
  );
}