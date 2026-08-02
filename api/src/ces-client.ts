import { GoogleAuth } from 'google-auth-library';

const auth = new GoogleAuth({
  scopes: ['https://www.googleapis.com/auth/cloud-platform'],
});

/**
 * Generate a short-lived CES chat token for the token broker flow.
 */
export async function generateChatToken(deploymentName: string): Promise<string> {
  const client = await auth.getClient();
  const accessToken = await client.getAccessToken();

  const url = `https://ces.googleapis.com/v1beta/${deploymentName}:generateChatToken`;
  const resp = await fetch(url, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${accessToken.token}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({}),
  });

  if (!resp.ok) {
    const body = await resp.text();
    throw new Error(`generateChatToken failed: ${resp.status} ${body}`);
  }

  const data = (await resp.json()) as { token: string };
  return data.token;
}
