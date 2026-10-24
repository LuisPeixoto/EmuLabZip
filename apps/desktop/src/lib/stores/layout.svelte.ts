import { api, errorText, type Audit, type Finding, type LayoutResult, type UiNode } from "../api";
import { app } from "./app.svelte";

export type Tab = "tree" | "audit";

/** Every node with its parent, parents first. */
function flatten(root: UiNode): UiNode[] {
  const out: UiNode[] = [];
  const visit = (n: UiNode) => {
    out.push(n);
    n.children.forEach(visit);
  };
  visit(root);
  return out;
}

/** The deepest node under a point that is drawn last, which is what a tap there would reach. */
export function nodeAt(node: UiNode, x: number, y: number): UiNode | null {
  const b = node.bounds;
  if (x < b.x || y < b.y || x >= b.x + b.width || y >= b.y + b.height) return null;
  for (let i = node.children.length - 1; i >= 0; i--) {
    const hit = nodeAt(node.children[i], x, y);
    if (hit) return hit;
  }
  return node;
}

/** What to call a node in a list: its text or label, else its id, else its class. */
export function title(n: UiNode): string {
  const name = (n.label || n.text || n.hint || "").trim();
  return name || n.resourceId?.split("/").pop() || n.class.split(".").pop() || "view";
}

class LayoutStore {
  result = $state.raw<LayoutResult | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);
  selectedId = $state<number | null>(null);
  hoverId = $state<number | null>(null);
  search = $state("");
  tab = $state<Tab>("tree");
  /** Show a box around every view, not only the hovered one. */
  showAll = $state(false);
  /** Ids of nodes whose children are hidden in the tree. */
  collapsed = $state<Set<number>>(new Set());

  nodes = $derived(this.result ? flatten(this.result.layout.root) : []);
  byId = $derived(new Map(this.nodes.map((n) => [n.id, n])));
  selected = $derived(this.selectedId != null ? (this.byId.get(this.selectedId) ?? null) : null);
  audit = $derived<Audit | null>(this.result?.audit ?? null);
  findingsByNode = $derived.by(() => {
    const map = new Map<number, Finding[]>();
    for (const f of this.audit?.findings ?? []) map.set(f.node, [...(map.get(f.node) ?? []), f]);
    return map;
  });
  matches = $derived.by(() => {
    const needle = this.search.trim().toLowerCase();
    if (!needle) return null;
    return new Set(
      this.nodes
        .filter((n) => [n.text, n.label, n.hint, n.resourceId, n.class].some((v) => v?.toLowerCase().includes(needle)))
        .map((n) => n.id),
    );
  });

  #ticket = 0;
  #key = "";

  /** Forgets the layout of another device. */
  attach(key: string) {
    if (key === this.#key) return;
    this.#key = key;
    this.result = null;
    this.selectedId = null;
    this.hoverId = null;
    this.error = null;
  }

  async capture() {
    const target = app.target;
    if (!target) return;
    const ticket = ++this.#ticket;
    this.loading = true;
    this.error = null;
    try {
      const result = await api.layoutRead(target);
      if (ticket !== this.#ticket) return;
      this.result = result;
      this.selectedId = null;
      this.collapsed = new Set();
    } catch (e) {
      if (ticket === this.#ticket) this.error = errorText(e);
    } finally {
      if (ticket === this.#ticket) this.loading = false;
    }
  }

  select(id: number | null) {
    this.selectedId = id;
  }

  toggle(id: number) {
    const next = new Set(this.collapsed);
    if (!next.delete(id)) next.add(id);
    this.collapsed = next;
  }

  /** Opens the tree down to a node, so selecting it from the screenshot shows it. */
  reveal(id: number) {
    const parents = new Map<number, number>();
    for (const n of this.nodes) for (const c of n.children) parents.set(c.id, n.id);
    const next = new Set(this.collapsed);
    for (let p = parents.get(id); p !== undefined; p = parents.get(p)) next.delete(p);
    this.collapsed = next;
  }
}

export const layout = new LayoutStore();
