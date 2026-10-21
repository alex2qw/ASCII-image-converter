// @vitest-environment jsdom
import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { convertImage, type ConvertOptions } from '../../api/convert';
import { useConversion } from './useConversion';

vi.mock('../../api/convert', () => ({ convertImage: vi.fn() }));
const convert = vi.mocked(convertImage);
const options: ConvertOptions = {
  mode: 'black_and_white', max_columns: 100, font_size: 16,
  character_width: 10, character_height: 18, ramp: '@# ',
  contrast: 0, brightness: 0, invert: false, background: '#000000',
};
const image = new File(['source'], 'source.png', { type: 'image/png' });
const png = new Blob(['rendered'], { type: 'image/png' });

function deferred() {
  let resolve!: (blob: Blob) => void;
  const promise = new Promise<Blob>(done => { resolve = done; });
  return { promise, resolve };
}

beforeEach(() => {
  vi.useFakeTimers();
  convert.mockReset();
  let nextUrl = 0;
  Object.defineProperty(URL, 'createObjectURL', { configurable: true, value: vi.fn(() => `blob:preview-${++nextUrl}`) });
  Object.defineProperty(URL, 'revokeObjectURL', { configurable: true, value: vi.fn() });
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

async function settleEditing() {
  await act(async () => { await vi.advanceTimersByTimeAsync(300); });
}

describe('conversion lifecycle', () => {
  it('does not claim success or contact the API without an image', async () => {
    const { result } = renderHook(() => useConversion(null, undefined, options));
    await settleEditing();
    expect(result.current.status).toBe('idle');
    expect(result.current.resultUrl).toBe('');
    expect(convert).not.toHaveBeenCalled();
  });

  it('debounces edits and submits only the latest settings', async () => {
    const pending = deferred();
    convert.mockReturnValue(pending.promise);
    const { result, rerender } = renderHook(
      ({ settings }) => useConversion(image, undefined, settings),
      { initialProps: { settings: options } },
    );
    expect(result.current.status).toBe('waiting');
    await act(async () => { await vi.advanceTimersByTimeAsync(200); });
    const latest = { ...options, brightness: 40 };
    rerender({ settings: latest });
    await act(async () => { await vi.advanceTimersByTimeAsync(299); });
    expect(convert).not.toHaveBeenCalled();
    await act(async () => { await vi.advanceTimersByTimeAsync(1); });
    expect(convert).toHaveBeenCalledTimes(1);
    expect(convert.mock.calls[0][0].options).toBe(latest);
    expect(result.current.status).toBe('rendering');
    await act(async () => { pending.resolve(png); });
    expect(result.current.status).toBe('success');
    expect(result.current.resultUrl).toBe('blob:preview-1');
  });

  it('hides outdated results immediately and revokes URLs when replaced or removed', async () => {
    convert.mockResolvedValue(png);
    const { result, rerender } = renderHook(
      ({ file, settings }: { file: File | null; settings: ConvertOptions }) => useConversion(file, undefined, settings),
      { initialProps: { file: image as File | null, settings: options } },
    );
    await settleEditing();
    expect(result.current.resultUrl).toBe('blob:preview-1');
    rerender({ file: image, settings: { ...options, contrast: 20 } });
    expect(result.current.status).toBe('waiting');
    expect(result.current.resultUrl).toBe('');
    await settleEditing();
    expect(result.current.resultUrl).toBe('blob:preview-2');
    expect(URL.revokeObjectURL).toHaveBeenCalledWith('blob:preview-1');
    rerender({ file: null, settings: options });
    expect(result.current.status).toBe('idle');
    expect(result.current.resultUrl).toBe('');
    expect(URL.revokeObjectURL).toHaveBeenCalledWith('blob:preview-2');
  });

  it('ignores an old response even when its server work finishes after cancellation', async () => {
    const old = deferred();
    convert.mockReturnValueOnce(old.promise).mockResolvedValue(png);
    const { result, rerender } = renderHook(
      ({ settings }) => useConversion(image, undefined, settings),
      { initialProps: { settings: options } },
    );
    await settleEditing();
    const oldSignal = convert.mock.calls[0][1];
    rerender({ settings: { ...options, brightness: 20 } });
    expect(oldSignal?.aborted).toBe(true);
    await settleEditing();
    expect(result.current.resultUrl).toBe('blob:preview-1');
    await act(async () => { old.resolve(png); });
    expect(result.current.resultUrl).toBe('blob:preview-1');
    expect(URL.createObjectURL).toHaveBeenCalledTimes(1);
  });

  it('reports failures without enabling exports and allows retrying the same settings', async () => {
    convert.mockRejectedValueOnce(new Error('All conversion slots are in use.')).mockResolvedValue(png);
    const { result } = renderHook(() => useConversion(image, undefined, options));
    await settleEditing();
    expect(result.current.status).toBe('error');
    expect(result.current.error).toBe('All conversion slots are in use.');
    expect(result.current.resultUrl).toBe('');
    act(() => result.current.retry());
    expect(result.current.status).toBe('waiting');
    expect(result.current.error).toBe('');
    await settleEditing();
    expect(result.current.status).toBe('success');
    expect(convert).toHaveBeenCalledTimes(2);
  });

  it('cancels pending timers and releases the final preview on unmount', async () => {
    convert.mockResolvedValue(png);
    const first = renderHook(() => useConversion(image, undefined, options));
    first.unmount();
    await settleEditing();
    expect(convert).not.toHaveBeenCalled();
    const second = renderHook(() => useConversion(image, undefined, options));
    await settleEditing();
    second.unmount();
    expect(URL.revokeObjectURL).toHaveBeenCalledWith('blob:preview-1');
  });
});
