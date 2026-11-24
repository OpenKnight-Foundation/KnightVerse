"use client";

import { useEffect } from "react";

export function ServiceWorkerRegistrar() {
  useEffect(() => {
    if (typeof window === "undefined" || !("serviceWorker" in navigator)) {
      return;
    }

    navigator.serviceWorker
      .register("/sw.js", { scope: "/" })
      .catch((error) => {
        if (process.env.NODE_ENV !== "production") {
          console.warn("Service worker registration failed:", error);
        }
      });
  }, []);

  return null;
}

export default ServiceWorkerRegistrar;
