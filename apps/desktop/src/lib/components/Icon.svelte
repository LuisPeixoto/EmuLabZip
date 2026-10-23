<script lang="ts">
  // Lucide-style 24px stroke icons (ISC licensed shapes), inlined so the app works fully offline.
  const paths: Record<string, string> = {
    pause: "M8 5v14M16 5v14",
    trash: "M3 6h18M8 6V4h8v2M19 6l-1 14H6L5 6M10 11v6M14 11v6",
    search: "M11 11m-7 0a7 7 0 1 0 14 0a7 7 0 1 0 -14 0M21 21l-4.3-4.3",
    globe: "M12 12m-9 0a9 9 0 1 0 18 0a9 9 0 1 0 -18 0M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18",
    settings: "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z",
    sun: "M12 12m-4 0a4 4 0 1 0 8 0a4 4 0 1 0 -8 0M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4",
    moon: "M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z",
    download: "M12 3v12M7 10l5 5 5-5M5 21h14",
    alert: "M12 9v4M12 17h.01M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z",
    layers: "M12 2 2 7l10 5 10-5zM2 17l10 5 10-5M2 12l10 5 10-5",
    monitor: "M3 4h18v12H3zM8 20h8M12 16v4",
    laptop: "M4 5h16v11H4zM2 19h20",
    copy: "M9 9h11v11H9zM5 15H4V4h11v1",
    x: "M18 6 6 18M6 6l12 12",
    check: "M20 6 9 17l-5-5",
    chevronRight: "M9 18l6-6-6-6",
    chevronDown: "M6 9l6 6 6-6",
    terminal: "M4 17l6-6-6-6M12 19h8",
    sidebar: "M3 4h18v16H3zM9 4v16",
    filter: "M22 3H2l8 9.5V19l4 2v-8.5z",
    smartphone: "M7 2h10v20H7zM12 18h.01",
    folder: "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
    info: "M12 12m-9 0a9 9 0 1 0 18 0a9 9 0 1 0 -18 0M12 16v-4M12 8h.01",
    code: "M16 18l6-6-6-6M8 6l-6 6 6 6",
    clock: "M12 12m-9 0a9 9 0 1 0 18 0a9 9 0 1 0 -18 0M12 7v5l3 2",
    play: "M6 4l14 8-14 8z",
    sliders: "M4 21v-7M4 10V3M12 21v-9M12 8V3M20 21v-5M20 12V3M1 14h6M9 8h6M17 16h6",
    gauge: "M12 14l4-4M3.3 17a10 10 0 1 1 17.4 0",
    list: "M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01",
    folderOpen: "M6 14l1.5-2.5A2 2 0 0 1 9.2 10.5H21l-2.4 7a2 2 0 0 1-1.9 1.5H4a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v2.5",
    file: "M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9zM14 3v6h6",
    book: "M4 19.5A2.5 2.5 0 0 1 6.5 17H20M4 19.5A2.5 2.5 0 0 0 6.5 22H20V2H6.5A2.5 2.5 0 0 0 4 4.5z",
    eye: "M2 12s4-8 10-8 10 8 10 8-4 8-10 8-10-8-10-8zM12 12m-3 0a3 3 0 1 0 6 0a3 3 0 1 0 -6 0",
    power: "M12 2v10M18.4 6.6a9 9 0 1 1-12.8 0",
    zap: "M13 2 3 14h9l-1 8 10-12h-9z",
    plus: "M12 5v14M5 12h14",
    minus: "M5 12h14",
    history: "M3 12a9 9 0 1 0 3-6.7L3 8M3 3v5h5M12 7v5l3 2",
    android: "M5 16V12a7 7 0 0 1 14 0v4zM8 5.5 6.5 3M16 5.5 17.5 3M9.5 10h.01M14.5 10h.01M5 19h14",
    apple: "M12 20.94c1.5 0 2.75 1.06 4 1.06 3 0 6-8 6-12.22A4.91 4.91 0 0 0 17 5c-2.22 0-4 1.44-5 2-1-.56-2.78-2-5-2a4.9 4.9 0 0 0-5 4.78C2 14 5 22 8 22c1.25 0 2.5-1.06 4-1.06zM10 2c1 .5 2 2 2 5",
    camera: "M14.5 4h-5L7 7H4a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-3zM12 17a4 4 0 1 0 0-8 4 4 0 0 0 0 8z",
    activity: "M22 12h-4l-3 9L9 3l-3 9H2",
    cpu: "M6 6h12v12H6zM9 9h6v6H9zM9 2v4M15 2v4M9 18v4M15 18v4M2 9h4M2 15h4M18 9h4M18 15h4",
    database: "M12 3c4.4 0 8 1.3 8 3s-3.6 3-8 3-8-1.3-8-3 3.6-3 8-3zM4 6v12c0 1.7 3.6 3 8 3s8-1.3 8-3V6M4 12c0 1.7 3.6 3 8 3s8-1.3 8-3",
    refresh: "M21 12a9 9 0 1 1-2.6-6.4M21 3v6h-6",
    external: "M15 3h6v6M10 14 21 3M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6",
    arrowDownToLine: "M12 3v12M7 10l5 5 5-5M5 21h14",
    battery: "M3 7h15a1 1 0 0 1 1 1v8a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V8a1 1 0 0 1 1-1zM22 11v2M6 10v4M10 10v4",
    regex: "M17 3v10M12.7 5.5l8.6 5M12.7 10.5l8.6-5M3 21h5v-5H3z",
    route: "M6 19a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM18 11a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM9 16h6a3 3 0 0 0 0-6H9a3 3 0 0 1 0-6h3",
    github: "M15 22v-4a4.8 4.8 0 0 0-1-3.5c3 0 6-2 6-5.5.08-1.25-.27-2.48-1-3.5.28-1.15.28-2.35 0-3.5 0 0-1 0-3 1.5-2.64-.5-5.36-.5-8 0C6 2 5 2 5 2c-.3 1.15-.3 2.35 0 3.5A5.4 5.4 0 0 0 4 9c0 3.5 3 5.5 6 5.5-.39.49-.68 1.05-.85 1.65S8.93 17.38 9 18v4M9 18c-4.51 2-5-2-7-2",
  };
  let { name, size = 16, class: cls = "", stroke = 1.8 }: { name: string; size?: number; class?: string; stroke?: number } = $props();
</script>

<svg
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={stroke}
  stroke-linecap="round"
  stroke-linejoin="round"
  class={cls}
  aria-hidden="true"><path d={paths[name] ?? paths.info} /></svg>
