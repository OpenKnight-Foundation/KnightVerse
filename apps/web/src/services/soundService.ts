export type SoundPack = 'classic-wood' | 'marble' | 'cyberpunk' | 'retro-8bit';
export type SoundEvent = 'move' | 'capture' | 'check' | 'castle' | 'promote' | 'low_time' | 'victory' | 'defeat';

interface TonePart {
  type: OscillatorType;
  freq: number;
  start: number;
  duration: number;
  volume?: number;
}

const PACK_DEFINITIONS: Record<SoundPack, Record<SoundEvent, TonePart[]]> = {
  'classic-wood': {
    move: [{ type: 'sine', freq: 440, start: 0, duration: 0.08, volume: 0.8 }],
    capture: [{ type: 'sine', freq: 320, start: 0, duration: 0.12, volume: 0.8 }],
    check: [{ type: 'sine', freq: 660, start: 0, duration: 0.1, volume: 0.7 }],
    castle: [{ type: 'sine', freq: 550, start: 0, duration: 0.15, volume: 0.7 }],
    promote: [
      { type: 'sine', freq: 660, start: 0, duration: 0.1, volume: 0.7 },
      { type: 'sine', freq: 880, start: 0.1, duration: 0.2, volume: 0.7 },
    ],
    low_time: [{ type: 'sine', freq: 1200, start: 0, duration: 0.03, volume: 0.5 }],
    victory: [
      { type: 'sine', freq: 523, start: 0, duration: 0.15, volume: 0.7 },
      { type: 'sine', freq: 659, start: 0.15, duration: 0.15, volume: 0.7 },
      { type: 'sine', freq: 784, start: 0.3, duration: 0.3, volume: 0.8 },
    ],
    defeat: [{ type: 'sine', freq: 220, start: 0, duration: 0.6, volume: 0.8 }],
  },
  'marble': {
    move: [{ type: 'triangle', freq: 600, start: 0, duration: 0.06, volume: 0.8 }],
    capture: [{ type: 'triangle', freq: 450, start: 0, duration: 0.1, volume: 0.8 }],
    check: [{ type: 'triangle', freq: 900, start: 0, duration: 0.08, volume: 0.7 }],
    castle: [{ type: 'triangle', freq: 700, start: 0, duration: 0.12, volume: 0.7 }],
    promote: [
      { type: 'triangle', freq: 900, start: 0, duration: 0.1, volume: 0.7 },
      { type: 'triangle', freq: 1200, start: 0.1, duration: 0.2, volume: 0.7 },
    ],
    low_time: [{ type: 'triangle', freq: 1500, start: 0, duration: 0.02, volume: 0.5 }],
    victory: [
      { type: 'triangle', freq: 784, start: 0, duration: 0.12, volume: 0.7 },
      { type: 'triangle', freq: 988, start: 0.12, duration: 0.12, volume: 0.7 },
      { type: 'triangle', freq: 1175, start: 0.24, duration: 0.3, volume: 0.8 },
    ],
    defeat: [{ type: 'triangle', freq: 300, start: 0, duration: 0.5, volume: 0.8 }],
  },
  'cyberpunk': {
    move: [{ type: 'square', freq: 200, start: 0, duration: 0.08, volume: 0.5 }],
    capture: [{ type: 'square', freq: 150, start: 0, duration: 0.1, volume: 0.5 }],
    check: [{ type: 'square', freq: 500, start: 0, duration: 0.1, volume: 0.5 }],
    castle: [{ type: 'square', freq: 300, start: 0, duration: 0.12, volume: 0.5 }],
    promote: [
      { type: 'square', freq: 400, start: 0, duration: 0.08, volume: 0.5 },
      { type: 'square', freq: 600, start: 0.08, duration: 0.15, volume: 0.5 },
    ],
    low_time: [{ type: 'square', freq: 800, start: 0, duration: 0.02, volume: 0.4 }],
    victory: [
      { type: 'square', freq: 600, start: 0, duration: 0.1, volume: 0.5 },
      { type: 'square', freq: 800, start: 0.1, duration: 0.1, volume: 0.5 },
      { type: 'square', freq: 1000, start: 0.2, duration: 0.2, volume: 0.6 },
    ],
    defeat: [{ type: 'sawtooth', freq: 100, start: 0, duration: 0.5, volume: 0.5 }],
  },
  'retro-8bit': {
    move: [{ type: 'square', freq: 440, start: 0, duration: 0.05, volume: 0.6 }],
    capture: [{ type: 'square', freq: 330, start: 0, duration: 0.08, volume: 0.6 }],
    check: [{ type: 'square', freq: 660, start: 0, duration: 0.06, volume: 0.6 }],
    castle: [{ type: 'square', freq: 550, start: 0, duration: 0.1, volume: 0.6 }],
    promote: [
      { type: 'square', freq: 660, start: 0, duration: 0.05, volume: 0.6 },
      { type: 'square', freq: 880, start: 0.05, duration: 0.1, volume: 0.6 },
    ],
    low_time: [{ type: 'square', freq: 1000, start: 0, duration: 0.02, volume: 0.4 }],
    victory: [
      { type: 'square', freq: 523, start: 0, duration: 0.08, volume: 0.6 },
      { type: 'square', freq: 659, start: 0.08, duration: 0.08, volume: 0.6 },
      { type: 'square', freq: 784, start: 0.16, duration: 0.16, volume: 0.7 },
    ],
    defeat: [{ type: 'square', freq: 220, start: 0, duration: 0.4, volume: 0.6 }],
  },
};

export const SOUND_PACKS: { id: SoundPack; name: string }[] = [
  { id: 'classic-wood', name: 'Classic Wood' },
  { id: 'marble', name: 'Crisp Marble' },
  { id: 'cyberpunk', name: 'Cyberpunk / Sci-Fi' },
  { id: 'retro-8bit', name: 'Retro 8-Bit' },
];

export const SOUND_EVENTS: SoundEvent[] = ['move', 'capture', 'check', 'castle', 'promote', 'low_time', 'victory', 'defeat'];

function clamp01(v: number): number {
  return Math.min(1, Math.max(0, v));
}

class SoundService {
  private audioContext: AudioContext | null = null;
  private cachedBuffers: Map<string, AudioBuffer> = new Map();
  private currentPack: SoundPack = 'classic-wood';
  private masterVolume = 0.7;
  private eventVolumes: Record<SoundEvent, number> = {
    move: 0.8, capture: 0.8, check: 0.8, castle: 0.8, promote: 0.8, low_time: 0.6, victory: 1.0, defeat: 1.0,
  };
  private lowTimeTimer: number | null = null;

  setPack(pack: SoundPack): void {
    if (this.currentPack === pack) return;
    this.currentPack = pack;
  }

  setMasterVolume(v: number): void {
    this.masterVolume = clamp01(v);
  }

  setEventVolume(event: SoundEvent, v: number): void {
    this.eventVolumes[event] = clamp01(v);
  }

  private ensureAudioContext(): AudioContext {
    if (!this.audioContext) {
      const AudioCtx = window.AudioContext || (window as any).webkitAudioContext;
      this.audioContext = new AudioCtx();
    }
    if (this.audioContext.state === 'suspended') {
      this.audioContext.resume();
    }
    return this.audioContext;
  }

  private createBuffer(pack: SoundPack, event: SoundEvent): AudioBuffer {
    const ctx = this.ensureAudioContext();
    const tones = PACK_DEFINITIONS[pack][event];
    const totalDuration = Math.max(...tones.map(t => t.start + t.duration)) + 0.05;
    const buffer = ctx.createBuffer(1, Math.ceil(ctx.sampleRate * totalDuration), ctx.sampleRate);
    const data = buffer.getChannelData(0);
    for (let i = 0; i < data.length; i++) data[i] = 0;

    for (const tone of tones) {
      const startSample = Math.floor(tone.start * ctx.sampleRate);
      const durationSamples = Math.floor(tone.duration * ctx.sampleRate);
      const volume = tone.volume ?? 1;
      const attack = Math.min(0.005, tone.duration / 4);
      const release = Math.min(0.01, tone.duration / 4);

      for (let i = 0; i < durationSamples; i++) {
        const sampleIndex = startSample + i;
        if (sampleIndex >=data.length) break;
        const t = i / ctx.sampleRate;
        let sample: number;
        const phase = 2 * Math.PI * tone.freq * t;
        switch (tone.type) {
          case 'sine': sample = Math.sin(phase); break;
          case 'square': sample = Math.sin(phase) >= 0 ? 1 : -1; break;
          case 'triangle': sample = 2 / Math.PI * Math.asin(Math.sin(phase)); break;
          case 'sawtooth': sample = 2 * (t * tone.freq - Math.floor(t * tone.freq + 0.5)); break;
          default: sample = 0;
        }
        let envelope = 1;
        if (t < attack) envelope = t / attack;
        if (t > tone.duration - release) envelope = Math.max(0, (tone.duration - t) / release);
        data[sampleIndex] += sample * volume * envelope;
      }
    }
    return buffer;
  }

  private getBuffer(pack: SoundPack, event: SoundEvent): AudioBuffer {
    const key = `${pack}:${event}`;
    if (!this.cachedBuffers.has(key)) {
      this.cachedBuffers.set(key, this.createBuffer(pack, event));
    }
    return this.cachedBuffers.get(key)!;
  }

  play(event: SoundEvent, pack?: SoundPack): void {
    if (this.masterVolume === 0) return;
    const p = pack ?? this.currentPack;
    const buffer = this.getBuffer(p, event);
    const ctx = this.ensureAudioContext();
    const source = ctx.createBufferSource();
    source.buffer = buffer;
    const gainNode = ctx.createGain();
    const effective = this.masterVolume * (this.eventVolumes[event] ?? 1);
    gainNode.gain.setValueAtTime(Math.max(effective, 0.001), ctx.currentTime);
    source.connect(gainNode).connect(ctx.destination);
    source.start(ctx.currentTime);
  }

  preview(pack: SoundPack, event: SoundEvent): void {
    this.play(event, pack);
  }

  preloadPack(pack?: SoundPack): void {
    const p = pack ?? this.currentPack;
    SOUND_EVENTS.forEach(event => this.getBuffer(p, event));
  }

  setLowTimeAlarm(active: boolean): void {
    if (active && this.lowTimeTimer === null) {
      const tick = () => this.play('low_time');
      tick();
      this.lowTimeTimer = window.setInterval(tick, 1000);
    } else if (!active && this.lowTimeTimer !== null) {
      window.clearInterval(this.lowTimeTimer);
      this.lowTimeTimer = null;
    }
  }
}

export const soundService = new SoundService();
