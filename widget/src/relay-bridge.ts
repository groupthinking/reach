import { config } from './config';
import { renderCustomCard, renderCustomText } from './events';

let ws: WebSocket | null = null;
let subId: string | null = null;

interface NostrEvent {
  id: string;
  pubkey: string;
  created_at: number;
  kind: number;
  tags: string[][];
  content: string;
  sig: string;
}

export function connectRelay(relayUrl: string, channelId: string): void {
  if (ws) { ws.close(); }

  ws = new WebSocket(relayUrl);
  subId = `reach-${Date.now()}`;

  ws.onopen = () => {
    console.info('[RelayBridge] Connected to relay:', relayUrl);
    const req = JSON.stringify(['REQ', subId, { '#e': [channelId], limit: 50 }]);
    ws!.send(req);
  };

  ws.onmessage = (event: MessageEvent) => {
    try {
      const msg = JSON.parse(event.data as string) as unknown[];
      if (!Array.isArray(msg)) return;
      const [type, ...rest] = msg;
      if (type === 'EVENT' && rest.length >= 2) {
        handleRelayEvent(rest[1] as NostrEvent);
      } else if (type === 'AUTH' && rest.length >= 1) {
        console.info('[RelayBridge] AUTH challenge received - NIP-42 auth required');
      } else if (type === 'NOTICE') {
        console.warn('[RelayBridge] NOTICE:', rest[0]);
      }
    } catch (err) {
      console.error('[RelayBridge] parse error:', err);
    }
  };

  ws.onerror = (err) => { console.error('[RelayBridge] WebSocket error:', err); };
  ws.onclose = () => { console.info('[RelayBridge] Disconnected'); };
}

export function disconnectRelay(): void {
  if (ws) {
    if (subId) {
      try { ws.send(JSON.stringify(['CLOSE', subId])); } catch (_) {}
    }
    ws.close();
    ws = null;
    subId = null;
  }
}

function handleRelayEvent(event: NostrEvent): void {
  if (event.kind === 1) {
    renderCustomText(event.content);
  } else {
    renderCustomCard({
      type: 'relay_event',
      event_id: event.id,
      kind: event.kind,
      content: event.content,
      pubkey: event.pubkey,
      created_at: event.created_at,
    });
  }
}

export async function postAudioEvent(audioBlob: Blob): Promise<void> {
  const formData = new FormData();
  formData.append('audio', audioBlob, 'audio.webm');
  try {
    const resp = await fetch(`${config.apiGatewayUrl}/api/audio`, { method: 'POST', body: formData });
    if (!resp.ok) { console.error('[RelayBridge] audio POST failed:', resp.status); return; }
    const result = (await resp.json()) as { transcript: string; understanding: string };
    if (result.transcript) { renderCustomText(`[Audio] ${result.transcript}`); }
  } catch (err) {
    console.error('[RelayBridge] audio POST error:', err);
  }
}
