import { useEffect } from 'react';
import { useSound as useSoundContext } from '../context/SoundContext';

export const useSound = useSoundContext;

export function useLowTimeAlarm(secondsLeft: number | null) {
  const { setLowTimeAlarm } = useSoundContext();
  useEffect(() => {
    if (secondsLeft !== null && secondsLeft < 10) {
      setLowTimeAlarm(true);
    } else {
      setLowTimeAlarm(false);
    }
  }, [secondsLeft, setLowTimeAlarm]);
}
