export const config = {
  deploymentName: import.meta.env.VITE_CES_DEPLOYMENT_NAME as string,
  modality: (import.meta.env.VITE_MODALITY as string) || 'mixed',
  apiGatewayUrl: (import.meta.env.VITE_API_GATEWAY_URL as string) || 'http://localhost:3000',
};
