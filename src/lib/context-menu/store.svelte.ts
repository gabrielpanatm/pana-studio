export type ContextMenuSource = "preview" | "layers" | "code" | "generic";

export type ContextMenuItem = {
  id: string;
  label: string;
  shortcut?: string;
  disabled?: boolean;
  hidden?: boolean;
  separatorBefore?: boolean;
  tone?: "default" | "danger";
  action?: () => void | Promise<void>;
};

export type ContextMenuRequest = {
  source: ContextMenuSource;
  x: number;
  y: number;
  title?: string;
  subtitle?: string;
  items: ContextMenuItem[];
};

export class ContextMenuState {
  current = $state<ContextMenuRequest | null>(null);

  open(request: ContextMenuRequest) {
    const items = request.items.filter((item) => !item.hidden);
    if (items.length === 0) {
      this.close();
      return;
    }
    this.current = {
      ...request,
      items,
    };
  }

  close() {
    this.current = null;
  }

  async run(item: ContextMenuItem) {
    if (item.disabled) return;
    this.close();
    await item.action?.();
  }
}

export const contextMenu = new ContextMenuState();
