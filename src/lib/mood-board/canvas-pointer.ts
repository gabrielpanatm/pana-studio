import type { MoodBoardMarqueeBox } from "$lib/mood-board/drag";
import type { MoodBoardSnapGuide } from "$lib/mood-board/snap";

export type MoodBoardItemDragSelection = {
  selectedItemIds: string[];
  dragItemIds: string[];
  shouldStartDrag: boolean;
};

export type MoodBoardMarqueeSelectionStart = {
  additive: boolean;
  initialSelection: string[];
  selectedItemIds: string[];
};

export type MoodBoardDragOverlayState = {
  marqueeBox: MoodBoardMarqueeBox | null;
  snapGuides: MoodBoardSnapGuide[];
  attachFrameId: string | null;
};

export function moodBoardItemDragSelection(
  selectedItemIds: string[],
  visibleItemIds: string[],
  itemId: string,
  additive: boolean,
): MoodBoardItemDragSelection {
  const isSelected = selectedItemIds.includes(itemId);
  if (additive && isSelected) {
    return {
      selectedItemIds: selectedItemIds.filter((selectedId) => selectedId !== itemId),
      dragItemIds: [],
      shouldStartDrag: false,
    };
  }

  const nextSelectedItemIds = additive
    ? [...selectedItemIds, itemId]
    : isSelected ? selectedItemIds : [itemId];
  const visibleIds = new Set(visibleItemIds);

  return {
    selectedItemIds: nextSelectedItemIds,
    dragItemIds: nextSelectedItemIds.includes(itemId)
      ? nextSelectedItemIds.filter((selectedId) => visibleIds.has(selectedId))
      : [itemId],
    shouldStartDrag: true,
  };
}

export function moodBoardMarqueeSelectionStart(
  selectedItemIds: string[],
  additive: boolean,
): MoodBoardMarqueeSelectionStart {
  return {
    additive,
    initialSelection: additive ? [...selectedItemIds] : [],
    selectedItemIds: additive ? selectedItemIds : [],
  };
}

export function clearMoodBoardDragOverlays(): MoodBoardDragOverlayState {
  return {
    marqueeBox: null,
    snapGuides: [],
    attachFrameId: null,
  };
}
