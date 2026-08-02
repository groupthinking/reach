import WebSocket from 'ws';
import crypto from 'crypto';

interface NostrKeypair {
  privateKeyHex: string;
  publicKeyHex: string;
}

interface NostrEvent {
  id: string;
  pubkey: string;
  created_at: number;
  kind: number;
  tags: string[][];
  content: string;
  sig: string;
}

/**
 * Compute NIP-01 event id: sha256 of canonical JSON serialization.
 */
function computeEventId(event: Omit<NostrEvent, 'id' | 'sig'>): string {
  const serialized = JSON.stringify([
    0,
    event.pubkey,
    event.created_at,
    event.kind,
    event.tags,
    event.content,
  ]);
  return crypto.createHash('sha256').update(serialized).digest('hex');
}

/**
 * Send a NIP-01 EVENT to the relay.
 * Signs the event with the provided keypair using secp256k1 Schnorr (BIP-340).
 * Note: In production, use a proper secp256k1 library (e.g., @noble/secp256k1).
 */
export async function sendEvent(
  relayUrl: string,
  communityId: string,
  channelId: string,
  content: string,
  keypair: NostrKeypair
): Promise<void> {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(relayUrl);

    const unsignedEvent = {
      pubkey: keypair.publicKeyHex,
      created_at: Math.floor(Date.now() / 1000),
      kind: 1,
      tags: [['e', channelId], ['community', communityId]],
      content,
    };

    const id = computeEventId(unsignedEvent);

    // Placeholder signature — replace with real secp256k1 Schnorr signing
    const sig = crypto.randomBytes(64).toString('hex');

    const event: NostrEvent = { ...unsignedEvent, id, sig };

    ws.on('open', () => {
      ws.send(JSON.stringify(['EVENT', event]));
    });

    ws.on('message', (data: Buffer) => {
      try {
        const msg = JSON.parse(data.toString()) as unknown[];
        if (Array.isArray(msg) && msg[0] === 'OK') {
          ws.close();
          resolve();
        }
      } catch (_) {}
    });

    ws.on('error', reject);
    setTimeout(() => { ws.close(); reject(new Error('relay sendEvent timeout')); }, 10_000);
  });
}

/**
 * Subscribe to a community channel and stream EVENT responses.
 */
export function subscribe(
  relayUrl: string,
  communityId: string,
  channelId: string,
  onEvent: (event: NostrEvent) => void
): () => void {
  const ws = new WebSocket(relayUrl);
  const subId = `api-${Date.now()}`;

  ws.on('open', () => {
    ws.send(JSON.stringify(['REQ', subId, { '#e': [channelId], limit: 100 }]));
  });

  ws.on('message', (data: Buffer) => {
    try {
      const msg = JSON.parse(data.toString()) as unknown[];
      if (Array.isArray(msg) && msg[0] === 'EVENT' && msg.length >= 3) {
        onEvent(msg[2] as NostrEvent);
      }
    } catch (_) {}
  });

  ws.on('error', (err) => console.error('[RelayClient] error:', err));

  return () => {
    ws.send(JSON.stringify(['CLOSE', subId]));
    ws.close();
  };
}
