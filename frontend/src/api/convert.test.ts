// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { convertImage, type ConvertOptions } from './convert';

const request = {
  image: new File(['image'], 'image.png', { type: 'image/png' }),
  options: {} as ConvertOptions,
};
afterEach(() => vi.unstubAllGlobals());

describe('conversion responses', () => {
  it('rejects a successful HTML response instead of presenting it as a preview', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response('<html>wrong server</html>', {
      headers: { 'Content-Type': 'text/html' },
    })));
    await expect(convertImage(request)).rejects.toThrow('unexpected response');
  });

  it('rejects empty PNG responses', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response('', {
      headers: { 'Content-Type': 'image/png' },
    })));
    await expect(convertImage(request)).rejects.toThrow('empty image');
  });

  it('preserves the server explanation when conversion capacity is full', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(JSON.stringify({
      error: { code: 'server_busy', message: 'All conversion slots are in use.' },
    }), { status: 503, headers: { 'Content-Type': 'application/json' } })));
    await expect(convertImage(request)).rejects.toThrow('All conversion slots are in use.');
  });
});
