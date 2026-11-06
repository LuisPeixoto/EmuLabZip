<script lang="ts" module>
  export interface TreeNode {
    label: string;
    /** Smaller text after the label: a file and line, or props. */
    detail?: string;
    /** Made by the app's own code (or a component it wrote) rather than by the framework. */
    own?: boolean;
    children: TreeNode[];
  }

  /** Whether the node or anything under it says `needle`. */
  export function matches(node: TreeNode, needle: string): boolean {
    return (
      node.label.toLowerCase().includes(needle) ||
      (node.detail ?? "").toLowerCase().includes(needle) ||
      node.children.some((c) => matches(c, needle))
    );
  }
</script>

<script lang="ts">
  import Icon from "../Icon.svelte";
  import Tree from "./Tree.svelte";

  let { node, depth = 0, needle = "" }: { node: TreeNode; depth?: number; needle?: string } = $props();

  let open = $state<boolean | null>(null);
  const expanded = $derived(needle ? true : (open ?? depth < 5));
  const shown = $derived(needle ? node.children.filter((c) => matches(c, needle)) : node.children);
</script>

<div>
  <div class="flex items-center hover:bg-hover" style="padding-left:{6 + depth * 14}px">
    {#if node.children.length}
      <button onclick={() => (open = !expanded)} class="grid size-5 shrink-0 place-items-center text-faint hover:text-fg" aria-label="Expand or collapse"><Icon name={expanded ? "chevronDown" : "chevronRight"} size={11} /></button>
    {:else}<span class="size-5 shrink-0"></span>{/if}
    <span class="mr-1.5 size-1.5 shrink-0 rounded-full {node.own ? 'bg-accent' : 'bg-line-strong'}" title={node.own ? "Made by the app's own code" : "Made by the framework or a package"}></span>
    <span class="mono min-w-0 truncate py-0.5 text-[12px] {node.own ? 'font-medium' : 'text-muted'}">{node.label}</span>
    {#if node.detail}<span class="mono ml-2 min-w-0 truncate text-[11px] text-faint">{node.detail}</span>{/if}
    {#if !expanded && node.children.length}<span class="mono ml-2 text-[10.5px] text-faint">{node.children.length}</span>{/if}
  </div>
  {#if expanded}
    {#each shown as child, i (i)}
      <Tree node={child} depth={depth + 1} {needle} />
    {/each}
  {/if}
</div>
