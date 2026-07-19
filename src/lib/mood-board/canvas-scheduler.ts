import type { MoodBoard } from "$lib/mood-board/model";

export type MoodBoardBoardScheduler = {
  schedule: (board: MoodBoard) => void;
  flush: () => MoodBoard | null;
  cancel: () => void;
  current: () => MoodBoard | null;
};

export function createMoodBoardBoardScheduler(applyBoard: (board: MoodBoard) => void): MoodBoardBoardScheduler {
  let frame: number | null = null;
  let pendingBoard: MoodBoard | null = null;

  function schedule(board: MoodBoard) {
    pendingBoard = board;
    if (frame !== null) return;

    frame = requestAnimationFrame(() => {
      frame = null;
      const next = pendingBoard;
      pendingBoard = null;
      if (next) applyBoard(next);
    });
  }

  function flush() {
    const next = pendingBoard;
    if (frame !== null) {
      cancelAnimationFrame(frame);
      frame = null;
    }
    pendingBoard = null;
    if (next) applyBoard(next);
    return next;
  }

  function cancel() {
    if (frame !== null) cancelAnimationFrame(frame);
    frame = null;
    pendingBoard = null;
  }

  return {
    schedule,
    flush,
    cancel,
    current: () => pendingBoard,
  };
}
