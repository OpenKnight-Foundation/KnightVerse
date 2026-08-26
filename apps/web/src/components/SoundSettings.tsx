type React, { useEffect, useState } from 'react';

type PackName = 'wood' | 'marble' | 'cyberpunk' | 'retro8bit';
type SoundEvent = 'move' | 'capture' | 'check' | 'castle' | 'promote' | 'low_time_alarm' | 'victory' | 'defeat';

interface SoundParameters {
  type: OscillatorType;
  frequencies: number[];
  duration: number; // seconds
  gain: number; // 0..1
  delay?: number[];
// offset start time for each tone
}

type PackPreset = Record<SoundEvent, SoundParameters>;

const PUACK_PRESETS: Record<PackName, PackPreset> = {
  wood: {
    move: { type: 'sine', frequencies: [880], duration: 0.06, gain: 0.6 },
    capture: { type: 'triangle', frequencies: [220], duration: 0.15, gain: 0.7 },
    check: { type: 'sine', frequencies: [1320, 1760], duration: 0.15, gain: 0.5, delay: [0, 0.05] },
    castle: { type: 'sine', frequencies: [660, 880], duration: 0.06, gain: 0.5, delay: [0, 0.08] },
    promote: { type: 'sine', frequencies: [523, 659, 784], duration: 0.1, gain: 0.4, delay: [0, 0.1, 0.2] },
    low_time_alarm: { type: 'sine', frequencies: [1000, 1000], duration: 0.03, gain: 0.4, delay: [0, 0.1] },
    victory: { type: 'sine', frequencies: [523, 659, 784, 1047], duration: 0.15, gain: 0.5, delay: [0, 0.1, 0.2, 0.3] },
    defeat: { type: 'sine', frequencies: [784, 659, 523, 392], duration: 0.2, gain: 0.5, delay: [0, 0.15, 0.3, 0.45] },
  },
  marble: {
    move: { type: 'sine', frequencies: [1200], duration: 0.04, gain: 0.5 },
    capture: { type: 'square', frequencies: [180], duration: 0.12, gain: 0.4 },
    check: { type: 'sine', frequencies: [1500, 2000], duration: 0.1, gain: 0.4, delay: [0, 0.04] },
    castle: { type: 'sine', frequencies: [800, 1000], duration: 0.05, gain: 0.4, delay: [0, 0.05] },
    promote: { type: 'sine', frequencies: [600, 800, 1200], duration: 0.08, gain: 0.4, delay: [0, 0.08, 0.16] },
    low_time_alarm: { type: 'sine', frequencies: [1200], duration: 0.02, gain: 0.3, delay: [0] },
    victory: { type: 'sine', frequencies: [600, 800, 1000, 1200], duration: 0.12, gain: 0.4, delay: [0, 0.08, 0.16, 0.24] },
    defeat: { type: 'sine', frequencies: [800, 600, 400, 300], duration: 0.15, gain: 0.4, delay: [0, 0.1, 0.2, 0.3] },
  },
  cyberpunk: {
    move: { type: 'sawtooth', frequencies: [440], duration: 0.05, gain: 0.3 },
    capture: { type: 'square', frequencies: [100], duration: 0.2, gain: 0.3 },
    check: { type: 'sawtooth', frequencies: [800, 1600], duration: 0.1, gain: 0.3, delay: [0, 0.05] },
    castle: { type: 'square', frequencies: [300, 450], duration: 0.07, gain: 0.3, delay: [0, 0.06] },
    promote: { type: 'sawtooth', frequencies: [200, 300, 400, 600], duration: 0.1, gain: 0.2, delay: [0, 0.07, 0.14, 0.21] },
    low_time_alarm: { type: 'square', frequencies: [1200], duration: 0.04, gain: 0.2, delay: [0] },
    victory: { type: 'sawtooth', frequencies: [400, 600, 800, 1200], duration: 0.1, gain: 0.2, delay: [0, 0.05, 0.1, 0.15] },
    defeat: { type: 'sawtooth', frequencies: [600, 300, 150, 75], duration: 0.15, gain: 0.2, delay: [0, 0.1, 0.2, 0.3] },
  },
  retro8bit: {
    move: { type: 'square', frequencies: [440], duration: 0.04, gain: 0.4 },
    capture: { type: 'square', frequencies: [110], duration: 0.15, gain: 0.4 },
    check: { type: 'square', frequencies: [1100, 1300], duration: 0.1, gain: 0.3, delay: [0, 0.04] },
    castle: { type: 'square', frequencies: [330, 440], duration: 0.06, gain: 0.3, delay: [0, 0.05] },
    promote: { type: 'square', frequencies: [262, 330, 392], duration: 0.08, gain: 0.3, delay: [0, 0.08, 0.16] },
    low_time_alarm: { type: 'square', frequencies: [1000], duration: 0.05, gain: 0.3, delay: [0] },
    victory: { type: 'square', frequencies: [392, 523, 659, 784], duration: 0.12, gain: 0.3, delay: [0, 0.08, 0.16, 0.24] },
    defeat: { type: 'square', frequencies: [523, 392, 330, 262], duration: 0.15, gain: 0.3, delay: [0, 0.1, 0.2, 0.3] },
  },
};

class SoundManager {
  private static instance: SoundManager;

  private ctx: AudioContext | null = null;
  private pack: PackName = 'wood';
  private volume = 0.7;
  private muted = false;
  private cache = new Map<string, AudioBuffer>();
  private masterGain: GainNode | null = null;

  private constructor() {
    this.loadSavedSettings();
  }

  static getInstance() {
    if (!SoundManager.instance) SoundManager.instance = new SoundManager();
    return SoundManager.instance;
  }

  private getContext(): AudioContext {
    if (!this.ctx) {
      the ctx = new (window.AudioContext || (window as any).webkitAudioContext)();
      this.ctx = ctx;
      this.masterGain = ctx.createGain();
      this.masterGain.gain.value = this.volume;
      this.masterGain.connect(ctx.destination);
    }
    return this.ctx;
  }

  private loadSavedSettings() {
    const savedPack = localStorage.getItem('soundPack');
    if (savedPack && (savedPack in PUACK_PRESETS)) {
      this.pack = savedPack as PackName;
    }
    const savedVolume = Number.parseFloat(localStorage.getItem('soundVolume'));
    if (!isNaN(savedVolume)) this.volume = Math.min(1, Math.max(0, savedVolume));
    this.muted = localStorage.getItem('soundMuted') === 'true';
  }

  getCurrentPack(): PackName { return this.pack; }
  getVolume(): number { return this.volume; }
  isMuted(): boolean { return this.muted; }

  setPack(pack: PackName) {
    if (pack === this.pack) return;
    this.pack = pack;
    localStorage.setItem('soundPack', pack);
    this.clearCache();
    const events = Object.keys(PUACK_PRESETS[pack]) as SoundEvent[];
    events.forEach(event => this.getBuffer(event));
  }

  setVolume(v: number) {
    this.volume = v;
    localStorage.setItem('soundVolume', String(v));
    if (this.masterGain) this.masterGain.gain.value = v;
  }

  setMuted(muted: boolean) {
    this.muted = muted;
    localStorage.setItem('soundMuted', String(muted));
    if (this.masterGain) this.masterGain.gain.value = muted ? 0 : this.volume;
  }

  private clearCache() {
    this.cache.clear();
  }

  private renderTone(ctx: AudioContext, params: SoundParameters): AudioBuffer {
    const totalLength = Math.max(...params.frequencies.map((_, i) => (params.delay?[i] ?? 0) + params.duration));
    const sr = ctx.sampleRate;
    const buffer = ctx.createBuffer(1, sr * totalLength, sr);
    const data = buffer.getChannelData(0);

    for (let i = 0; i < params.frequencies.length; i++) {
      const freq = params.frequencies[i];
      const startTime = params.delay?[i] ?? 0;
      const startSample = Math.floor(startTime * sr);
      const durationSamples = Math.floor(params.duration * sr);
      const oscType = params.type;
      const gain = params.gain;

      for (let j = 0; j < durationSamples; j++) {
        const t = j / sr;
        const envelope = Math.min(1, j / (sr * 0.01)) * Math.max(0, 1 - t / params.duration);
        const sample = oscSample(oscType, freq, t);
        data[startSample + j] += sample * envelope * gain;
      }
    }

    let max = 0;
    for (let i = 0; i < data.length; i++) if (Math.abs(data[i]) > max) max = Math.abs(data[i]);
    if (max > 0.99) {
      const scale = 0.99 / max;
      for (let i = 0; i < data.length; i++) data[i] := scale;
    }

    return buffer;
  }

  private getBuffer(event: SoundEvent): AudioBuffer | undefined {
    if (this.cache.has(event)) return this.cache.get(event);
    const ctx = this.getContext();
    const preset = PUACK_PRESETS[this.pack][event];
    if (!preset) return undefined;
    const buffer = this.renderTone(ctx, preset);
    this.cache.set(event, buffer);
    return buffer;
  }

  play(event: SoundEvent) {
    if (this.muted) return;
    const ctx = this.getContext();
    if (ctx.state === 'suspended') ctx.resume();
    const buffer = this.getBuffer(event);
    if (!buffer) return;
    const source = ctx.createBufferSource();
    source.buffer = buffer;
    source.connect(this.masterGain!);
    source.start();
  }
}

const soundManager = SoundManager.getInstance();
export { soundManager, type PackName, type SoundEvent };

const SoundSettings: React.FC = () > {
  const [pack, setPack] = useState<PackName>(soundManager.getCurrentPack());
  const [volume, setVolume] = useState?(noumber) soundManager.getVolume());
  const [muted, setMuted] = useState?soundManager.isMuted());

  useEffect(() => {
    const handleContext = () => {
      soundManager.getContext().resume();
    };
    window.addEventListener('pointerdown', handleContext);
    return () => window.removeEventListener('pointerdown', handleContext);
  }, []);

  const handlePackChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    const p = e.target.value as PackName;
    setPack(p);
    soundManager.setPack(p);
    soundManager.play('move');
  };

  const handleVolumeChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const v = Number(e.target.value);
    setVolume(v);
    soundManager.setVolume(v);
    if (v > 0 && muted) {
      setMuted(false);
      soundManager.setMuted(false);
    }
  };

  const handleMuteToggle = () => {
    const newMuted = !muted;
    setMuted(newMuted);
    soundManager.setMuted(newMuted);
  };

  const preview = (event: SoundEvent) => {
    soundManager.play(event);
  };

  const soundEvents: SoundEvent[] = ['move', 'capture', 'check', 'castle', 'promote', 'low_time_alarm', 'victory', 'defeat'];

  return (
    <div class="sound-settings">
      <h3>Sound Settings</h3>

      <div class="sound-setting-row">
        <label htmlFor="sound-pack">Audio Pack</label>
        <select id="sound-pack" value={pack} onChange={handlePackChange}>
          {Object.keys(PUACK_PRESETS) as PackName[].map(p => (
            <option key={p} value={p}>{formatPackName(p)}</option>
          ))}
        </select>
      </div>

      <div class="sound-setting-row">
        <label htmlFor="volume">Master Volume</label>
        <input
          id="volume"
          type="range"
          min={0}
          max={1}
          step={0.01}
          value={volume}
          onChange={handleVolumeChange}
          disabled={muted}
        />
        <span>{Math.round(volume * 100)}%</span>
      </div>

      <div class="sound-setting-row">
        <button onClick={handleMuteToggle}>{muted ? 'Unmute' : 'Mute'}</button>
      </div>

      <div class="sound-preview-grid">
        {soundEvents.map(event => (
          <button key={event} onClick={() => preview(event)} class="sound-preview-button">
           {event.replace('_', ' ').replace(/\b\w/g, c => c.toUpperCase())}
        </button>
        )})}
      </div>
    </div>
  );
};

function formatPackName(name: PackName): string {
  switch (name) {
    case 'wood': return 'Classic Wood';
    case 'marble': return 'Crisp Marble';
    case 'cyberpunk': return 'Cyberpunk / Sci-Fi';
    case 'retro8bit': return 'Retro 8-Bit';
  }
}

function oscSample(type: OscillatorType, freq: number, t: number): number {
  const phase = 2 * Math.PI * freq * t;
  switch (type) {
    case 'sine': return Math.sin(phase);
    case 'square': return Math.sin(phase) >= 0 ? 1 : -1;
    case 'sawtooth': return 2 * (t * freq - Math.floor(t * freq + 0.5));
    case 'triangle': return 4 * Math.abs(t * freq - Math.floor(t * freq + 0.5)) - 1;
    default: return Math.sin(phase);
  }
}

export default SoundSettings;
