/* EasyCue3 Remote — service worker.
 * Caches the app shell for fast reload; live state always comes from the
 * WebSocket and is never cached. Bump CACHE_VERSION when shell files change.
 */
'use strict';

const CACHE_VERSION = 'easycue3-remote-v2';
const SHELL = [
  '/',
  '/app.js',
  '/framework7-bundle.min.css',
  '/framework7-bundle.min.js',
  '/manifest.json',
  '/icon-192.png',
  '/icon-512.png',
];

self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(CACHE_VERSION).then((cache) => cache.addAll(SHELL)).then(() => self.skipWaiting())
  );
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys().then((keys) =>
      Promise.all(keys.filter((k) => k !== CACHE_VERSION).map((k) => caches.delete(k)))
    ).then(() => self.clients.claim())
  );
});

self.addEventListener('fetch', (event) => {
  const url = new URL(event.request.url);
  // Never intercept live data or non-GET requests.
  if (event.request.method !== 'GET' || url.pathname.startsWith('/api/') || url.pathname === '/ws') {
    return;
  }
  // Network-first so shell updates land; cache fallback keeps reloads instant
  // when the console is briefly unreachable.
  event.respondWith(
    fetch(event.request)
      .then((resp) => {
        const copy = resp.clone();
        caches.open(CACHE_VERSION).then((cache) => cache.put(event.request, copy));
        return resp;
      })
      .catch(() => caches.match(event.request))
  );
});
