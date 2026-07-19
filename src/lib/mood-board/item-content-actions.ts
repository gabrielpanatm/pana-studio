import type {
  MoodBoardColorItem,
  MoodBoardFrameItem,
  MoodBoardGroupItem,
  MoodBoardNoteItem,
  MoodBoardReferenceItem,
  MoodBoardTextItem,
  MoodBoardVectorGroupItem,
} from "$lib/mood-board/model";

export function moodBoardNoteWithText(item: MoodBoardNoteItem, text: string): MoodBoardNoteItem {
  return { ...item, text };
}

export function moodBoardTextItemWithText(item: MoodBoardTextItem, text: string): MoodBoardTextItem {
  return { ...item, text };
}

export function moodBoardColorWithLabel(item: MoodBoardColorItem, label: string): MoodBoardColorItem {
  return { ...item, label };
}

export function moodBoardReferenceWithTitle(item: MoodBoardReferenceItem, title: string): MoodBoardReferenceItem {
  return { ...item, title };
}

export function moodBoardReferenceWithUrl(item: MoodBoardReferenceItem, url: string): MoodBoardReferenceItem {
  return { ...item, url };
}

export function moodBoardReferenceWithNote(item: MoodBoardReferenceItem, note: string): MoodBoardReferenceItem {
  return { ...item, note };
}

export function moodBoardFrameWithTitle(item: MoodBoardFrameItem, title: string): MoodBoardFrameItem {
  return { ...item, title };
}

export function moodBoardGroupWithTitle(item: MoodBoardGroupItem, title: string): MoodBoardGroupItem {
  return { ...item, title };
}

export function moodBoardVectorGroupWithTextElement(
  item: MoodBoardVectorGroupItem,
  elementId: string,
  text: string,
): MoodBoardVectorGroupItem {
  return {
    ...item,
    elements: item.elements.map((element) => (
      element.id === elementId && element.type === "text" ? { ...element, text } : element
    )),
  };
}
