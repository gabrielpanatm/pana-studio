import {
  cloneMoodBoard,
  findMoodBoardItem,
  type MoodBoard,
  type MoodBoardItem,
  type MoodBoardVectorHandleMode,
  type MoodBoardVectorNode,
  type MoodBoardVectorNodeEditState,
} from "$lib/mood-board/model";
import { editableSvgElementNodes, svgElementWithVectorNodes } from "$lib/mood-board/svg";
import { setSelectedVectorNodesHandleMode } from "$lib/mood-board/vector";

export type MoodBoardVectorEditTarget = {
  itemId: string;
  svgElementId: string | null;
};

export type MoodBoardSelectionState = {
  selectedItemIds: string[];
  selectedSvgElementId: string | null;
  vectorEditTarget: MoodBoardVectorEditTarget | null;
  vectorNodeEditState: MoodBoardVectorNodeEditState | null;
};

export type MoodBoardVectorNodeHandleModeEdit = {
  beforeItem: MoodBoardItem;
  nextItem: MoodBoardItem;
  vectorNodeEditState: MoodBoardVectorNodeEditState;
};

export function moodBoardSelectionState(
  selectedItemIds: string[],
  selectedSvgElementId: string | null,
  vectorEditTarget: MoodBoardVectorEditTarget | null,
  vectorNodeEditState: MoodBoardVectorNodeEditState | null,
): MoodBoardSelectionState {
  return {
    selectedItemIds,
    selectedSvgElementId,
    vectorEditTarget,
    vectorNodeEditState,
  };
}

export function moodBoardSelectedItems(board: MoodBoard, selectedItemIds: string[]) {
  return selectedItemIds
    .map((itemId) => findMoodBoardItem(board.items, itemId))
    .filter((item): item is MoodBoardItem => Boolean(item));
}

export function moodBoardVisibleSelectedIds(selectedItemIds: string[], visibleItemIds: string[]) {
  const visibleIds = new Set(visibleItemIds);
  return selectedItemIds.filter((itemId) => visibleIds.has(itemId));
}

export function moodBoardSelectOnly(itemId: string | null): MoodBoardSelectionState {
  return {
    selectedItemIds: itemId ? [itemId] : [],
    selectedSvgElementId: null,
    vectorEditTarget: null,
    vectorNodeEditState: null,
  };
}

export function moodBoardSetSelection(itemIds: string[], allItems: MoodBoardItem[]): MoodBoardSelectionState {
  const knownIds = new Set(allItems.map((item) => item.id));
  const selectedItemIds = itemIds.filter((itemId, index) => (
    knownIds.has(itemId) && itemIds.indexOf(itemId) === index
  ));

  return {
    selectedItemIds,
    selectedSvgElementId: null,
    vectorEditTarget: null,
    vectorNodeEditState: null,
  };
}

export function moodBoardToggleSelection(
  current: MoodBoardSelectionState,
  itemId: string,
): MoodBoardSelectionState {
  const selectedItemIds = current.selectedItemIds.includes(itemId)
    ? current.selectedItemIds.filter((selectedId) => selectedId !== itemId)
    : [...current.selectedItemIds, itemId];

  return {
    selectedItemIds,
    selectedSvgElementId: null,
    vectorEditTarget: null,
    vectorNodeEditState: null,
  };
}

export function moodBoardSetSvgElementSelection(
  board: MoodBoard,
  current: MoodBoardSelectionState,
  itemId: string,
  elementId: string | null,
): MoodBoardSelectionState {
  const item = findMoodBoardItem(board.items, itemId);
  const selectedSvgElementId = item?.type === "vectorGroup" ? elementId : null;
  const vectorTargetStillMatches = (
    current.vectorEditTarget
    && current.vectorEditTarget.itemId === itemId
    && current.vectorEditTarget.svgElementId === selectedSvgElementId
  );

  return {
    selectedItemIds: current.selectedItemIds,
    selectedSvgElementId,
    vectorEditTarget: vectorTargetStillMatches ? current.vectorEditTarget : null,
    vectorNodeEditState: vectorTargetStillMatches ? current.vectorNodeEditState : null,
  };
}

export function moodBoardEnterVectorEditMode(
  board: MoodBoard,
  itemId: string,
  svgElementId: string | null = null,
): MoodBoardSelectionState | null {
  const item = findMoodBoardItem(board.items, itemId);
  if (!item || (item.type !== "vectorPath" && item.type !== "vectorGroup")) return null;
  const selectedSvgElementId = item.type === "vectorGroup" ? svgElementId : null;

  return {
    selectedItemIds: [itemId],
    selectedSvgElementId,
    vectorEditTarget: { itemId, svgElementId: selectedSvgElementId },
    vectorNodeEditState: null,
  };
}

export function moodBoardActiveVectorNodeEditState(
  vectorNodeEditState: MoodBoardVectorNodeEditState | null,
  vectorEditTarget: MoodBoardVectorEditTarget | null,
) {
  if (
    vectorNodeEditState
    && vectorEditTarget
    && vectorNodeEditState.itemId === vectorEditTarget.itemId
    && vectorNodeEditState.svgElementId === vectorEditTarget.svgElementId
  ) return vectorNodeEditState;

  return null;
}

export function moodBoardAcceptedVectorNodeEditState(
  state: MoodBoardVectorNodeEditState | null,
  vectorEditTarget: MoodBoardVectorEditTarget | null,
) {
  return moodBoardActiveVectorNodeEditState(state, vectorEditTarget);
}

export function moodBoardVectorNodeHandleMode(
  node: MoodBoardVectorNode | undefined,
): MoodBoardVectorHandleMode {
  if (!node) return "corner";
  return node.handleMode ?? (node.in || node.out ? "independent" : "corner");
}

export function moodBoardApplyVectorNodeHandleMode(
  board: MoodBoard,
  state: MoodBoardVectorNodeEditState | null,
  mode: MoodBoardVectorHandleMode,
): MoodBoardVectorNodeHandleModeEdit | null {
  if (!state || !state.indexes.length) return null;
  const item = findMoodBoardItem(board.items, state.itemId);
  const selectedIndexes = new Set(state.indexes);

  if (item?.type === "vectorPath" && state.svgElementId === null) {
    const nodes = setSelectedVectorNodesHandleMode(item.nodes, selectedIndexes, item.closed, mode);
    return {
      beforeItem: cloneMoodBoardItem(board, item),
      nextItem: { ...item, nodes },
      vectorNodeEditState: {
        ...state,
        modes: state.indexes.map((index) => moodBoardVectorNodeHandleMode(nodes[index])),
      },
    };
  }

  if (item?.type !== "vectorGroup" || !state.svgElementId) return null;
  const element = item.elements.find((entry) => entry.id === state.svgElementId);
  const parsed = element?.type === "path" ? editableSvgElementNodes(element) : null;
  if (!element || element.type !== "path" || !parsed) return null;
  const nodes = setSelectedVectorNodesHandleMode(parsed.nodes, selectedIndexes, parsed.closed, mode);

  return {
    beforeItem: cloneMoodBoardItem(board, item),
    nextItem: {
      ...item,
      elements: item.elements.map((entry) => (
        entry.id === element.id && entry.type === "path" ? svgElementWithVectorNodes(entry, nodes, parsed.closed) : entry
      )),
    },
    vectorNodeEditState: {
      ...state,
      modes: state.indexes.map((index) => moodBoardVectorNodeHandleMode(nodes[index])),
    },
  };
}

function cloneMoodBoardItem(board: MoodBoard, item: MoodBoardItem): MoodBoardItem {
  return cloneMoodBoard({ ...board, items: [item] }).items[0];
}
