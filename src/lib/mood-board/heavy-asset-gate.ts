export type MoodBoardHeavyAssetPermit = {
  release: () => void;
};

export type MoodBoardHeavyAssetGate = {
  isBusy: () => boolean;
  tryAcquire: () => MoodBoardHeavyAssetPermit | null;
};

export function createMoodBoardHeavyAssetGate(): MoodBoardHeavyAssetGate {
  let active = false;

  return {
    isBusy: () => active,
    tryAcquire() {
      if (active) return null;
      active = true;
      let released = false;
      return {
        release() {
          if (released) return;
          released = true;
          active = false;
        },
      };
    },
  };
}
