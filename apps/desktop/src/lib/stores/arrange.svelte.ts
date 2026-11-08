import { loadValue, save } from "../prefs";
import { sanitizeSizes } from "../layout/size.ts";

const KEY = "arrangement";

export type ContentWidth = "narrow" | "wide" | "full";

interface Saved {
  sizes: Record<string, number>;
  contentWidth: ContentWidth;
}

function read(): Saved {
  const raw = loadValue<Partial<Saved> | null>(KEY, null);
  return {
    sizes: sanitizeSizes(raw?.sizes),
    contentWidth: raw?.contentWidth === "narrow" || raw?.contentWidth === "full" ? raw.contentWidth : "wide",
  };
}

/** How the person arranged the window: the size of each pane, and how wide the centred pages are. */
class ArrangeStore {
  sizes = $state<Record<string, number>>(read().sizes);
  contentWidth = $state<ContentWidth>(read().contentWidth);

  #save() {
    save(KEY, { sizes: $state.snapshot(this.sizes), contentWidth: this.contentWidth });
  }

  size(id: string): number | undefined {
    return this.sizes[id];
  }

  setSize(id: string, value: number) {
    this.sizes[id] = Math.round(value);
    this.#save();
  }

  /** Puts one pane back to its default, or all of them. */
  reset(id?: string) {
    if (id) delete this.sizes[id];
    else this.sizes = {};
    this.#save();
  }

  setContentWidth(width: ContentWidth) {
    this.contentWidth = width;
    this.#save();
  }

  /** The same for the pages that were a little narrower to begin with. */
  get pageMid(): string {
    return this.contentWidth === "narrow" ? "max-w-[860px]" : this.contentWidth === "full" ? "max-w-none" : "max-w-[1040px]";
  }

  /** The class of the box that centres a page: its widest size. */
  get pageMax(): string {
    return this.contentWidth === "narrow" ? "max-w-[900px]" : this.contentWidth === "full" ? "max-w-none" : "max-w-[1180px]";
  }
}

export const arrange = new ArrangeStore();
