import { registerCesEvents } from './events';
import { connectRelay } from './relay-bridge';
import { config } from './config';

// Register all CES event listeners
registerCesEvents();

// Connect relay bridge if channel config is available
const relayUrl = `${config.apiGatewayUrl.replace('http', 'ws').replace('3000', '8080')}/ws`;
const channelId = new URLSearchParams(window.location.search).get('channel') || '';

if (channelId) {
  connectRelay(relayUrl, channelId);
}
