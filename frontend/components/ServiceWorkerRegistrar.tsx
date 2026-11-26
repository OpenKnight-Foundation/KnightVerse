"use client";

import { useEffect } from "react";
import { debugLog } from "@/lib/debugLogger";

export function ServiceWorkerRegistrar() {
  useEffect(() => {
    if (typeof window === "undefined" || !("serviceWorker" in navigator)) {
      return;
    }

    navigator.serviceWorker
      .register("/sw.js", { scope: "/" })
      .catch((error) => {
        debugLog("Service worker registration failed:", error);
      });
  }, []);

  return null;
}

export default ServiceWorkerRegistrar;
