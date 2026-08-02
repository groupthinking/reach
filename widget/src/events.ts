import { config } from './config';

declare global {
  interface Window {
    chatSdk: {
      registerContext: (ctx: unknown) => void;
      prebuilts: {
        ces: {
          createContext: (opts: {
            deploymentName: string;
            tokenBroker?: { enableTokenBroker: boolean };
            enableWelcomeEvent?: boolean;
          }) => unknown;
        };
      };
    };
    chatMessenger: {
      renderCustomText: (text: string) => void;
      renderCustomCard: (payload: unknown) => void;
    };
  }
}

/** Register all CES JavaScript event listeners. */
export function registerCesEvents(): void {
  window.addEventListener('chat-messenger-loaded', () => {
    window.chatSdk.registerContext(
      window.chatSdk.prebuilts.ces.createContext({
        deploymentName: config.deploymentName,
        tokenBroker: { enableTokenBroker: true },
        enableWelcomeEvent: true,
      })
    );
  });

  window.addEventListener('chat-messenger-error', (e: Event) => {
    const detail = (e as CustomEvent).detail;
    console.error('[CES] chat-messenger-error', detail?.code, detail?.status);
  });

  window.addEventListener('chat-messenger-close', () => {
    console.info('[CES] chat-messenger-close — cleaning up');
    // Cleanup relay bridge subscription on close
    import('./relay-bridge').then(({ disconnectRelay }) => disconnectRelay());
  });

  window.addEventListener('df-update-cart-count', (e: Event) => {
    const detail = (e as CustomEvent).detail;
    console.info('[CES] df-update-cart-count', detail);
    // Update cart badge in host application if present
    const badge = document.getElementById('cart-count-badge');
    if (badge && detail?.count !== undefined) {
      badge.textContent = String(detail.count);
    }
  });
}

/** Render a plain text message in the CES widget. */
export function renderCustomText(text: string): void {
  if (window.chatMessenger?.renderCustomText) {
    window.chatMessenger.renderCustomText(text);
  }
}

/** Render a rich card payload in the CES widget. */
export function renderCustomCard(payload: unknown): void {
  if (window.chatMessenger?.renderCustomCard) {
    window.chatMessenger.renderCustomCard(payload);
  }
}
