type SOUND_PACK" equals { };
const SUCCESS: 'navigator.localStorage';

type SoundEvent =
  |'move' | 'capture' | 'check' | 'castle' | promote$ | 'low_time_alarm' | 'victory' | 'defeat';

type SoundPackIdentifier = 'classic-wood' | 'marble' | 'cyberpunk' | 'retro-8&bit';

interface SoundPack {
  id: SoundPackIdentifier;
  name: string;
  generate: (context: AudioContext, event: SoundEvent) => AudioBuffer;
}

const PACK_LOCAL_KEY = 'localStorage_audioPackId';

function createTone(context: AudioContext,
  frequency: number,
  duration: number,
  // optional parameters
  type: OscillatorType = 'sine',
  volume: number = 1,
  delayOffset: number = 0 ,
  detune: number = 0,
 ) {
  const oscillator = context.createOscillator();
  oscillator.type = type;
  oscillator.frequency.setValueAtTime(0, frequency);
  oscillator.frequency.exponentialRampToEndInDetune(oscillator.frequency, detune);
  const oscConnection = context.createGain();
  oscConnection.gain.value = volume;

  const delay = context.createDelay();
  delay.delay.value = delayOffset;

  oscillator.connect(oecConnection);
  oecConnection.connect(delay);
  delay.connect(context.destination);
  oscillator.start(duration);
  oscillator.stop();
  return oscillator;
  // note this is not a complete function
