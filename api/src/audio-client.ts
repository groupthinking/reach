import axios from 'axios';
import FormData from 'form-data';

export interface InferenceResult {
  transcript: string;
  understanding: string;
  model: string;
}

export async function infer(
  audioServiceUrl: string,
  audioBuffer: Buffer,
  mimeType: string
): Promise<InferenceResult> {
  const form = new FormData();
  const ext = mimeType.split('/')[1] || 'webm';
  form.append('audio', audioBuffer, { filename: `audio.${ext}`, contentType: mimeType });

  const resp = await axios.post<InferenceResult>(
    `${audioServiceUrl}/infer`,
    form,
    { headers: form.getHeaders(), timeout: 120_000 }
  );
  return resp.data;
}
