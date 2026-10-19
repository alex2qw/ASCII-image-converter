import { useEffect, useRef, useState } from 'react';
import { Copy, Download, ImagePlus, Maximize2, Moon, RotateCcw, Sun, Upload, X, Check } from 'lucide-react';
import { convertImage, type ConvertOptions } from '../api/convert';

type Preset = { name: string; options: ConvertOptions };
const defaults: ConvertOptions = { mode: 'black_and_white', max_columns: 100, font_size: 16, character_width: 10, character_height: 18, ramp: '@#8&o:*. ', contrast: 0, brightness: 0, invert: false, background: '#080808' };
const builtIns: Preset[] = [
  { name: 'Classic', options: defaults },
  { name: 'Terminal green', options: { ...defaults, mode: 'color', background: '#07140d', ramp: '█▓▒░ ' } },
  { name: 'Minimal', options: { ...defaults, max_columns: 72, ramp: '#+. ' } },
];
const storageKey = 'ascii-studio-presets';

export default function App() {
  const [file, setFile] = useState<File | null>(null); const [font, setFont] = useState<File | undefined>();
  const [options, setOptions] = useState<ConvertOptions>(defaults); const [sourceUrl, setSourceUrl] = useState(''); const [resultUrl, setResultUrl] = useState('');
  const [busy, setBusy] = useState(false); const [error, setError] = useState(''); const [zoom, setZoom] = useState(100); const [copied, setCopied] = useState(false);
  const [custom, setCustom] = useState<Preset[]>(() => { try { return JSON.parse(localStorage.getItem(storageKey) ?? '[]') as Preset[]; } catch { return []; } });
  const inputRef = useRef<HTMLInputElement>(null); const fontRef = useRef<HTMLInputElement>(null); const requestRef = useRef<AbortController | undefined>(undefined);

  useEffect(() => { if (!file) { setSourceUrl(''); return; } const url = URL.createObjectURL(file); setSourceUrl(url); return () => URL.revokeObjectURL(url); }, [file]);
  useEffect(() => {
    if (!file) return; requestRef.current?.abort(); const controller = new AbortController(); requestRef.current = controller; setBusy(true); setError('');
    convertImage({ image: file, font, options }, controller.signal).then(blob => { if (controller.signal.aborted) return; setResultUrl(old => { if (old) URL.revokeObjectURL(old); return URL.createObjectURL(blob); }); }).catch(e => { if (e.name !== 'AbortError') setError(e instanceof Error ? e.message : 'Unable to convert image.'); }).finally(() => { if (!controller.signal.aborted) setBusy(false); });
    return () => controller.abort();
  }, [file, font, options]);
  const update = <K extends keyof ConvertOptions>(key: K, value: ConvertOptions[K]) => setOptions(prev => ({ ...prev, [key]: value }));
  const selectFile = (candidate?: File) => { if (!candidate || !candidate.type.startsWith('image/')) { setError('Please choose a PNG, JPG, GIF, or WebP image.'); return; } setFile(candidate); setError(''); };
  const savePreset = () => { const name = window.prompt('Name this preset'); if (!name?.trim()) return; const next = [...custom, { name: name.trim(), options }]; setCustom(next); localStorage.setItem(storageKey, JSON.stringify(next)); };
  const copy = async () => { if (!resultUrl) return; try { const blob = await fetch(resultUrl).then(r => r.blob()); await navigator.clipboard.write([new ClipboardItem({ [blob.type]: blob })]); setCopied(true); setTimeout(() => setCopied(false), 1600); } catch { setError('Copy is not supported by this browser. Try downloading instead.'); } };
  const allPresets = [...builtIns, ...custom];
  return <main className="shell">
    <header className="topbar"><div className="brand"><span className="brand-mark">Aa</span><div><strong>ASCII Studio</strong><span>Image → character art</span></div></div><span className="api-status"><i /> Rust renderer connected</span></header>
    <section className="intro"><p className="eyebrow">IMAGE CONVERTER <span>01</span></p><h1>Make images speak<br /><em>in characters.</em></h1><p className="lede">A focused, tactile workspace for turning pixels into expressive ASCII art.</p></section>
    <div className="workspace">
      <aside className="panel controls"><div className="panel-heading"><div><span className="section-number">01</span><h2>Source image</h2></div><button className="icon-button" aria-label="Reset all settings" onClick={() => setOptions(defaults)}><RotateCcw size={16} /></button></div>
        <div className={'dropzone' + (file ? ' has-file' : '')} onDragOver={e => e.preventDefault()} onDrop={e => { e.preventDefault(); selectFile(e.dataTransfer.files[0]); }} onClick={() => inputRef.current?.click()} role="button" tabIndex={0} onKeyDown={e => { if (e.key === 'Enter' || e.key === ' ') inputRef.current?.click(); }}>
          {file ? <><img src={sourceUrl} alt="Selected source" /><div><strong>{file.name}</strong><span>{(file.size / 1024 / 1024).toFixed(2)} MB · Click to replace</span></div><button className="remove" aria-label="Remove image" onClick={e => { e.stopPropagation(); setFile(null); setResultUrl(''); }}><X size={15} /></button></> : <><Upload size={20} /><strong>Drop an image here</strong><span>or click to browse · PNG, JPG, GIF, WebP</span></>}
        </div><input ref={inputRef} type="file" accept="image/*" hidden onChange={e => selectFile(e.target.files?.[0])} />
        <div className="font-row"><div><label htmlFor="font">Custom font <small>optional</small></label><span>{font?.name ?? 'Bundled server font'}</span></div><button className="outline-button" onClick={() => fontRef.current?.click()}>Choose</button><input ref={fontRef} type="file" accept=".ttf,.otf,.woff,.woff2" hidden onChange={e => setFont(e.target.files?.[0])} /></div>
        <div className="control-group"><div className="label-row"><label>Render mode</label></div><div className="segmented"><button className={options.mode === 'black_and_white' ? 'active' : ''} onClick={() => update('mode', 'black_and_white')}><Moon size={14} /> B&W</button><button className={options.mode === 'color' ? 'active' : ''} onClick={() => update('mode', 'color')}><Sun size={14} /> Color</button></div></div>
        <Range label="Quality / width" value={options.max_columns} min={24} max={240} step={4} suffix={`${options.max_columns} cols`} onChange={v => update('max_columns', v)} />
        <Range label="Contrast" value={options.contrast} min={-100} max={100} suffix={`${options.contrast > 0 ? '+' : ''}${options.contrast}`} onChange={v => update('contrast', v)} />
        <Range label="Brightness" value={options.brightness} min={-100} max={100} suffix={`${options.brightness > 0 ? '+' : ''}${options.brightness}`} onChange={v => update('brightness', v)} />
        <div className="control-group"><div className="label-row"><label htmlFor="ramp">Character ramp</label><button className="text-button" onClick={() => update('ramp', defaults.ramp)}>Reset</button></div><input id="ramp" className="ramp-input" value={options.ramp} onChange={e => update('ramp', e.target.value)} /><small className="hint">Dark → light, left to right</small></div>
        <div className="inline-controls"><label className="toggle"><input type="checkbox" checked={options.invert} onChange={e => update('invert', e.target.checked)} /><span /> Invert ramp</label><label>Background <input className="color-input" type="color" value={options.background} onChange={e => update('background', e.target.value)} /></label></div>
        <div className="preset-area"><div className="label-row"><label>Presets</label><button className="text-button" onClick={savePreset}>+ Save current</button></div><select aria-label="Choose preset" onChange={e => { const preset = allPresets[Number(e.target.value)]; if (preset) setOptions(preset.options); }} defaultValue=""><option value="" disabled>Choose a look…</option>{allPresets.map((p, i) => <option value={i} key={`${p.name}-${i}`}>{p.name}{i >= builtIns.length ? ' · custom' : ''}</option>)}</select></div>
      </aside>
      <section className="panel preview-panel"><div className="panel-heading"><div><span className="section-number">02</span><h2>Live preview</h2></div><div className="preview-actions"><button className="icon-button" aria-label="Fit preview" onClick={() => setZoom(100)}><Maximize2 size={16} /></button><select aria-label="Preview zoom" value={zoom} onChange={e => setZoom(Number(e.target.value))}><option value={50}>50%</option><option value={75}>75%</option><option value={100}>Fit</option><option value={150}>150%</option><option value={200}>200%</option></select></div></div>
        <div className="canvas-wrap"><div className="canvas" style={{ background: options.background }}>{resultUrl ? <img src={resultUrl} alt="ASCII conversion preview" style={{ width: zoom === 100 ? 'auto' : `${zoom}%` }} /> : file && sourceUrl ? <div className="empty-preview"><ImagePlus size={28} /><span>{busy ? 'Rendering your image…' : 'Preview unavailable'}</span></div> : <div className="empty-preview"><ImagePlus size={32} /><strong>Your ASCII art will appear here</strong><span>Upload an image to get started</span></div>}{busy && <div className="loading-bar" />}</div></div>
        <div className="preview-footer"><span className={error ? 'error' : ''}>{error || (busy ? 'Rendering…' : file ? 'Preview is up to date' : 'Waiting for an image')}</span><div className="export-actions"><button className="outline-button" disabled={!resultUrl} onClick={copy}>{copied ? <Check size={15} /> : <Copy size={15} />} {copied ? 'Copied' : 'Copy image'}</button><a className={'primary-button' + (!resultUrl ? ' disabled' : '')} href={resultUrl || '#'} download="ascii-art.png" onClick={e => !resultUrl && e.preventDefault()}><Download size={15} /> Download PNG</a></div></div>
      </section>
    </div><footer>ASCII Studio <span>·</span> The web client sends your image to the configured Rust API for conversion.</footer>
  </main>;
}
function Range({ label, value, min, max, step = 1, suffix, onChange }: { label: string; value: number; min: number; max: number; step?: number; suffix: string; onChange: (v: number) => void }) { return <div className="control-group"><div className="label-row"><label>{label}</label><output>{suffix}</output></div><input type="range" min={min} max={max} step={step} value={value} onChange={e => onChange(Number(e.target.value))} /></div>; }
