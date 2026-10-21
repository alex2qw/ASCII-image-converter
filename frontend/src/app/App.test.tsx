// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { convertImage } from '../api/convert';
import App from './App';

vi.mock('../api/convert', () => ({ convertImage: vi.fn() }));
const convert = vi.mocked(convertImage);

beforeEach(() => {
  vi.useFakeTimers();
  convert.mockReset();
  localStorage.clear();
  Object.defineProperty(URL, 'createObjectURL', { configurable: true, value: vi.fn(() => 'blob:test') });
  Object.defineProperty(URL, 'revokeObjectURL', { configurable: true, value: vi.fn() });
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

function upload(container: HTMLElement) {
  fireEvent.change(container.querySelector('input[type="file"]')!, {
    target: { files: [new File(['source'], 'source.png', { type: 'image/png' })] },
  });
}
async function settleEditing() {
  await act(async () => { await vi.advanceTimersByTimeAsync(300); });
}

it('shows honest initial status and disables stale exports when settings change', async () => {
  convert.mockResolvedValue(new Blob(['rendered'], { type: 'image/png' }));
  const { container } = render(<App />);
  expect(screen.getByText('No conversion yet')).toBeTruthy();
  expect(screen.queryByText('Rust renderer connected')).toBeNull();
  expect((screen.getByRole('button', { name: 'Copy image' }) as HTMLButtonElement).disabled).toBe(true);
  upload(container);
  expect(screen.getByText('Waiting to render')).toBeTruthy();
  await settleEditing();
  expect(screen.getByText('Last conversion succeeded')).toBeTruthy();
  expect(screen.getByAltText('ASCII conversion preview')).toBeTruthy();
  expect(screen.getByText('Download PNG').getAttribute('aria-disabled')).toBe('false');
  fireEvent.change(screen.getAllByRole('slider')[1], { target: { value: '20' } });
  expect(screen.getByText('Waiting to render')).toBeTruthy();
  expect(screen.queryByAltText('ASCII conversion preview')).toBeNull();
  expect(screen.getByText('Download PNG').getAttribute('aria-disabled')).toBe('true');
  expect((screen.getByRole('button', { name: 'Copy image' }) as HTMLButtonElement).disabled).toBe(true);
});

it('shows conversion errors and recovers through the retry button', async () => {
  convert.mockRejectedValueOnce(new Error('All conversion slots are in use.'))
    .mockResolvedValue(new Blob(['rendered'], { type: 'image/png' }));
  const { container } = render(<App />);
  upload(container);
  await settleEditing();
  expect(screen.getByText('Last conversion failed')).toBeTruthy();
  expect(screen.getByText('All conversion slots are in use.')).toBeTruthy();
  expect(screen.getByText('Download PNG').getAttribute('aria-disabled')).toBe('true');
  fireEvent.click(screen.getByRole('button', { name: 'Retry conversion' }));
  expect(screen.getByText('Waiting to render')).toBeTruthy();
  await settleEditing();
  expect(screen.getByText('Last conversion succeeded')).toBeTruthy();
  expect(screen.queryByRole('button', { name: 'Retry conversion' })).toBeNull();
  fireEvent.click(screen.getByRole('button', { name: 'Remove image' }));
  expect(screen.getByText('No conversion yet')).toBeTruthy();
  expect(screen.queryByAltText('ASCII conversion preview')).toBeNull();
});
