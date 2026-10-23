<script lang="ts">
  import type { Platform } from "../api";
  import Icon from "./Icon.svelte";

  let { platform, size = 14, class: cls = "" }: { platform: Platform; size?: number; class?: string } = $props();
</script>

<span
  class="grid shrink-0 place-items-center rounded-md {platform === 'android' ? 'bg-ok/12 text-ok' : 'bg-info/12 text-info'} {cls}"
  style="width:{size + 10}px;height:{size + 10}px"
  title={platform === "android" ? "Android" : "iOS"}
>
  <Icon name={platform === "android" ? "android" : "apple"} {size} />
</span>
