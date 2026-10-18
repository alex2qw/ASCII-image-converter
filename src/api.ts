/** Contract expected from the Rust HTTP adapter (see README). */
export type ConvertOptions = { mode: 'black_and_white' | 'color'; max_columns: number; font_size: number; character_width: number; character_height: number; ramp: string; contrast: number; brightness: number; invert: boolean; background: string };
export type ConvertRequest = { image: File; font?: File; options: ConvertOptions };
const endpoint = import.meta.env.VITE_API_URL ?? '/convert';
const maxUploadBytes = 10 * 1024 * 1024;
export async function convertImage({ image, font, options }: ConvertRequest, signal?: AbortSignal): Promise<Blob> {
  if (image.size > maxUploadBytes) {
    throw new Error(`Image is too large. Please choose an image smaller than ${Math.round(maxUploadBytes / 1024 / 1024)} MiB.`);
  }
  // The bundled Axum adapter accepts `image` and a JSON `options` part. It
  // supplies the configured server font; custom font is reserved for adapters
  // that support a `font` part.
  const body = new FormData(); body.append('image', image); body.append('options', JSON.stringify(options)); if (font) body.append('font', font);
  const response = await fetch(endpoint, { method: 'POST', body, signal, headers: { Accept: 'image/png' } });
  if (!response.ok) { let message = `Conversion failed (${response.status})`; try { const json = await response.json() as { error?: string | { message?: string } }; message = typeof json.error === 'string' ? json.error : json.error?.message ?? message; } catch { /* non-JSON errors are fine */ } throw new Error(message); }
  return response.blob();
}
