"use client";

import { EnhancedSocketStatus } from "@/hook/useEnhancedChessSocket";

interface ConnectionStatusIndicatorProps {
  status: EnhancedSocketStatus;
  reconnectAttempts: number;
  latency: number | null;
}

export function ConnectionStatusIndicator({
  status,
  reconnectAttempts,
  latency,
}: ConnectionStatusIndicatorProps) {
  const getStatusColor = () => {
    switch (status) {
      case "connected":
        return "bg-emerald-400";
      case "connecting":
      case "reconnecting":
        return "bg-yellow-400 animate-pulse";
      case "disconnected":
        return "bg-orange-400";
      case "error":
        return "bg-red-400";
      default:
        return "bg-gray-400";
    }
  };

  const getStatusText = () => {
    switch (status) {
      case "connected":
        return "Connected";
      case "connecting":
        return "Connecting...";
      case "reconnecting":
        return `Reconnecting... (${reconnectAttempts})`;
      case "disconnected":
        return "Disconnected";
      case "error":
        return "Connection Error";
      default:
        return "Idle";
    }
  };

  const getLatencyQuality = () => {
    if (latency === null) return "Unknown";
    if (latency < 50) return "Excellent";
    if (latency < 100) return "Good";
    if (latency < 200) return "Fair";
    return "Poor";
  };

  const getLatencyColor = () => {
    if (latency === null) return "text-gray-400";
    if (latency < 50) return "text-emerald-400";
    if (latency < 100) return "text-green-400";
    if (latency < 200) return "text-yellow-400";
    return "text-red-400";
  };

  return (
    <div className="flex items-center gap-3 p-3 bg-gray-800/50 border border-gray-700/50 rounded-lg">
      {/* Status Indicator */}
      <div className="flex items-center gap-2">
        <div className={`w-2.5 h-2.5 rounded-full ${getStatusColor()}`} />
        <span className="text-sm font-medium text-gray-300">{getStatusText()}</span>
      </div>

      {/* Latency */}
      {latency !== null && (
        <div className="flex items-center gap-2 ml-4 pl-4 border-l border-gray-700">
          <span className="text-xs text-gray-400">Latency:</span>
          <span className={`text-xs font-mono font-semibold ${getLatencyColor()}`}>
            {latency}ms
          </span>
          <span className={`text-xs ${getLatencyColor()}`}>
            ({getLatencyQuality()})
          </span>
        </div>
      )}

      {/* Reconnection Attempts */}
      {reconnectAttempts > 0 && (
        <div className="flex items-center gap-2 ml-4 pl-4 border-l border-gray-700">
          <span className="text-xs text-gray-400">Reconnect attempts:</span>
          <span className="text-xs font-semibold text-yellow-400">
            {reconnectAttempts}
          </span>
        </div>
      )}
    </div>
  );
}
