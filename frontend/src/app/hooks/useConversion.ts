import { useEffect, useState } from 'react';
import { convertImage, type ConvertOptions } from '../../api/convert';

type Request = {
  image: File;
  font: File | undefined;
  options: ConvertOptions;
  attempt: number;
};
type Progress = {
  request: Request;
  status: 'rendering' | 'success' | 'error';
  error?: string;
};
type Result = { request: Request; url: string };
export type ConversionStatus = 'idle' | 'waiting' | Progress['status'];

const debounceMs = 300;

export function useConversion(image: File | null, font: File | undefined, options: ConvertOptions) {
  const [attempt, setAttempt] = useState(0);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [result, setResult] = useState<Result | null>(null);

  // Keep ownership of each preview URL in one place, including on unmount.
  useEffect(() => {
    return () => {
      if (result) URL.revokeObjectURL(result.url);
    };
  }, [result]);

  useEffect(() => {
    if (!image) {
      setResult(null);
      setProgress(null);
      return;
    }
    const request: Request = { image, font, options, attempt };
    const controller = new AbortController();
    const timer = window.setTimeout(async () => {
      setProgress({ request, status: 'rendering' });
      try {
        const blob = await convertImage(request, controller.signal);
        // An aborted request may still finish on the server. Never publish it.
        if (controller.signal.aborted) return;
        setResult({ request, url: URL.createObjectURL(blob) });
        setProgress({ request, status: 'success' });
      } catch (error) {
        if (controller.signal.aborted) return;
        setProgress({
          request,
          status: 'error',
          error: error instanceof Error ? error.message : 'Unable to convert image.',
        });
      }
    }, debounceMs);

    return () => {
      window.clearTimeout(timer);
      controller.abort();
    };
  }, [image, font, options, attempt]);

  // Derive freshness during rendering, before effects run. Otherwise a new
  // setting could briefly leave the previous result enabled for export.
  const matchesCurrent = (request: Request) =>
    request.image === image && request.font === font &&
    request.options === options && request.attempt === attempt;
  const status: ConversionStatus = !image ? 'idle'
    : progress && matchesCurrent(progress.request) ? progress.status : 'waiting';
  const resultUrl = status === 'success' && result && matchesCurrent(result.request)
    ? result.url : '';

  return {
    status,
    resultUrl,
    error: status === 'error' ? progress?.error ?? 'Unable to convert image.' : '',
    retry: () => setAttempt(value => value + 1),
  };
}
