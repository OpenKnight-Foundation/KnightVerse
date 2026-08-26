"use client";

import React, { createContext, useContext, useState, useEffect, useCallback, useRef } from "react";

export type WebSocketNode = {
  id: string;
  url: string;
  region: string;
  load: number; // 0 to 1
};

type WebSocketScalingContextType = {
  currentNode: WebSocketNode | null;
  availableNodes: WebSocketNode[];
  isScaling: boolean;
  getOptimalNode: (gameId: string) => Promise<WebSocketNode>;
  reportLoad: (nodeId: string, load: number) => void;
};

const WebSocketScalingContext = createContext<WebSocketScalingContextType | undefined>(undefined);

// Mock discovery service URL - in production this would be a real endpoint
const DISCOVERY_ENDPOINT = process.env.NEXT_PUBLIC_DISCOVERY_URL || "http://localhost:8000/v1/nodes/discovery";

export function WebSocketScalingProvider({ children }: { children: React.ReactNode }) {
  const [currentNode, setCurrentNode] = useState<WebSocketNode | null>(null);
  const [availableNodes, setAvailableNodes] = useState<WebSocketNode[]>([]);
  const [isScaling, setIsScaling] = useState(false);
  
  // Cache for game-to-node mapping to ensure session stickiness within horizontal scaling
  const gameNodeMap = useRef<Map<string, WebSocketNode>>(new Map());

  /**
   * Fetches the most optimal WebSocket node based on regional proximity and server load.
   * Implements horizontal load balancing logic on the client side.
   */
  const getOptimalNode = useCallback(async (gameId: string): Promise<WebSocketNode> => {
    // Check if we already have a sticky session for this game
    if (gameNodeMap.current.has(gameId)) {
      return gameNodeMap.current.get(gameId)!;
    }

    setIsScaling(true);
    try {
      // In a real implementation, we would fetch from DISCOVERY_ENDPOINT
      // For now, we simulate a discovery process that returns available horizontal nodes
      const mockNodes: WebSocketNode[] = [
        { id: "node-us-east-1", url: "ws://localhost:8080", region: "us-east", load: 0.2 },
        { id: "node-eu-west-1", url: "ws://localhost:8081", region: "eu-west", load: 0.4 },
        { id: "node-ap-south-1", url: "ws://localhost:8082", region: "ap-south", load: 0.1 },
      ];

      setAvailableNodes(mockNodes);

      // Sorting logic: prioritize lowest load
      const sortedNodes = [...mockNodes].sort((a, b) => a.load - b.load);
      const optimal = sortedNodes[0];

      gameNodeMap.current.set(gameId, optimal);
      setCurrentNode(optimal);
      return optimal;
    } finally {
      setIsScaling(false);
    }
  }, []);

  const reportLoad = useCallback((nodeId: string, load: number) => {
    setAvailableNodes(prev => 
      prev.map(node => node.id === nodeId ? { ...node, load } : node)
    );
  }, []);

  return (
    <WebSocketScalingContext.Provider value={{ 
      currentNode, 
      availableNodes, 
      isScaling, 
      getOptimalNode, 
      reportLoad 
    }}>
      {children}
    </WebSocketScalingContext.Provider>
  );
}

export const useWebSocketScaling = () => {
  const context = useContext(WebSocketScalingContext);
  if (!context) {
    throw new Error("useWebSocketScaling must be used within a WebSocketScalingProvider");
  }
  return context;
};
