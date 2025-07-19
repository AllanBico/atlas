// In frontend/src/hooks/useAtlasSocket.ts

import useWebSocket, { ReadyState } from 'react-use-websocket';
import type { WsMessage } from '../types'; // We will define this type next

// The URL of our Atlas WebSocket server.
// We use an environment variable to make this configurable.
const VITE_WS_URL = import.meta.env.VITE_WS_URL || 'ws://127.0.0.1:8080/ws';

// The interface for the data and methods our hook will provide.
export interface AtlasSocket {
  lastMessage: WsMessage | null;
  readyState: ReadyState;
  connectionStatus: string;
}

export const useAtlasSocket = (): AtlasSocket => {
  const { lastJsonMessage, readyState } = useWebSocket(VITE_WS_URL, {
    shouldReconnect: () => true, // Automatically try to reconnect
  });

  // A user-friendly string for the connection status
  const connectionStatus = {
    [ReadyState.CONNECTING]: 'Connecting',
    [ReadyState.OPEN]: 'Connected',
    [ReadyState.CLOSING]: 'Closing',
    [ReadyState.CLOSED]: 'Disconnected',
    [ReadyState.UNINSTANTIATED]: 'Uninstantiated',
  }[readyState];

  return {
    lastMessage: lastJsonMessage as WsMessage | null,
    readyState,
    connectionStatus,
  };
};