import Fastify from 'fastify';
import multipart from '@fastify/multipart';
import dotenv from 'dotenv';
import axios from 'axios';
import { z } from 'zod';
import { generateChatToken } from './ces-client';
import { sendEvent } from './relay-client';
import { infer as audioInfer } from './audio-client';

dotenv.config();

const PORT = parseInt(process.env.PORT || '3000', 10);
const RELAY_URL = process.env.RELAY_URL || 'ws://localhost:8080/ws';
const AUDIO_SERVICE_URL = process.env.AUDIO_SERVICE_URL || 'http://localhost:8000';
const CES_DEPLOYMENT_NAME = process.env.CES_DEPLOYMENT_NAME || '';
const RELAY_COMMUNITY_ID = process.env.RELAY_COMMUNITY_ID || '';
const RELAY_CHANNEL_ID = process.env.RELAY_CHANNEL_ID || '';
const RELAY_KEYPAIR_RAW = process.env.RELAY_KEYPAIR || '{}';

let relayKeypair = { privateKeyHex: '', publicKeyHex: '' };
try {
  relayKeypair = JSON.parse(RELAY_KEYPAIR_RAW);
} catch (_) {}

const ChatEventSchema = z.object({
  session_id: z.string(),
  message: z.string(),
  community_id: z.string().optional(),
  channel_id: z.string().optional(),
});

const app = Fastify({ logger: true });
await app.register(multipart);

/** POST /api/chat — receive CES session event, forward to Buzz relay */
app.post('/api/chat', async (request, reply) => {
  const parsed = ChatEventSchema.safeParse(request.body);
  if (!parsed.success) {
    return reply.status(400).send({ error: 'invalid payload', details: parsed.error });
  }

  const { message, community_id, channel_id } = parsed.data;
  const host = request.headers.host || '';

  try {
    await sendEvent(
      RELAY_URL,
      community_id || RELAY_COMMUNITY_ID,
      channel_id || RELAY_CHANNEL_ID,
      message,
      relayKeypair
    );
    return reply.send({ ok: true });
  } catch (err) {
    app.log.error(err);
    return reply.status(502).send({ error: 'relay unavailable' });
  }
});

/** POST /api/audio — receive audio upload, forward to Audio Flamingo */
app.post('/api/audio', async (request, reply) => {
  const data = await request.file();
  if (!data) {
    return reply.status(400).send({ error: 'no audio file' });
  }

  const chunks: Buffer[] = [];
  for await (const chunk of data.file) {
    chunks.push(chunk);
  }
  const audioBuffer = Buffer.concat(chunks);
  const mimeType = data.mimetype || 'audio/webm';

  try {
    const result = await audioInfer(AUDIO_SERVICE_URL, audioBuffer, mimeType);
    return reply.send(result);
  } catch (err) {
    app.log.error(err);
    return reply.status(502).send({ error: 'audio service unavailable' });
  }
});

/** GET /api/token — CES token broker flow */
app.get('/api/token', async (request, reply) => {
  if (!CES_DEPLOYMENT_NAME) {
    return reply.status(503).send({ error: 'CES_DEPLOYMENT_NAME not configured' });
  }
  try {
    const token = await generateChatToken(CES_DEPLOYMENT_NAME);
    return reply.send({ token });
  } catch (err) {
    app.log.error(err);
    return reply.status(502).send({ error: 'CES token generation failed' });
  }
});

/** GET /health — check all downstream services */
app.get('/health', async (request, reply) => {
  const checks: Record<string, string> = {};

  // Check relay (HTTP GET /)
  try {
    const relayHttpUrl = RELAY_URL.replace('ws://', 'http://').replace('wss://', 'https://').replace('/ws', '/');
    await axios.get(relayHttpUrl, { timeout: 3000 });
    checks.relay = 'ok';
  } catch {
    checks.relay = 'unavailable';
  }

  // Check audio service
  try {
    await axios.get(`${AUDIO_SERVICE_URL}/health`, { timeout: 3000 });
    checks.audio = 'ok';
  } catch {
    checks.audio = 'unavailable';
  }

  // CES connectivity (just check env is set)
  checks.ces = CES_DEPLOYMENT_NAME ? 'configured' : 'not-configured';

  const allOk = checks.relay === 'ok' && checks.audio === 'ok';
  return reply.status(allOk ? 200 : 207).send({ status: allOk ? 'ok' : 'degraded', checks });
});

try {
  await app.listen({ port: PORT, host: '0.0.0.0' });
  app.log.info(`Reach API gateway listening on port ${PORT}`);
} catch (err) {
  app.log.error(err);
  process.exit(1);
}
