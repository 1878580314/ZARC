<script lang="ts" module>
  /**
   * 内联 SVG 图标集（Lucide 风格，24×24 线条）。
   * Inline SVG icon set (Lucide style, 24×24 strokes).
   *
   * 整个 UI 此前用 emoji 作图标：字形随系统而异、无法着色、垂直对齐总差半像素。
   * 换成线条路径后颜色跟随 `currentColor`，线宽统一。
   * The whole UI previously used emoji as icons: glyphs varied with the system,
   * couldn't be tinted, and vertical alignment was always off by half a pixel.
   * With stroke paths, colors follow `currentColor` and stroke width is uniform.
   */
  export const ICONS = {
    // 导航与操作 / Navigation & actions
    compress:
      'M21 8v8a2 2 0 0 1-1 1.73l-7 4a2 2 0 0 1-2 0l-7-4A2 2 0 0 1 3 16V8a2 2 0 0 1 1-1.73l7-4a2 2 0 0 1 2 0l7 4A2 2 0 0 1 21 8Z|M3.3 7 12 12l8.7-5|M12 22V12',
    decompress:
      'M6 14.5 7.5 11.6A2 2 0 0 1 9.3 10.5H20a2 2 0 0 1 1.94 2.5l-1.55 6a2 2 0 0 1-1.94 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2',
    benchmark: 'M13 2 3 14h9l-1 8 10-12h-9l1-8Z',
    play: 'm7 4 13 8-13 8V4Z',
    stop: 'M7 7h10v10H7z',
    search: 'M11 19a8 8 0 1 0 0-16 8 8 0 0 0 0 16Z|m21 21-4.35-4.35',
    copy: 'M9 9h11a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2h-9a2 2 0 0 1-2-2V9Z|M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1',
    trash:
      'M3 6h18|M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2|M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6',
    refresh: 'M3 12a9 9 0 0 1 15.5-6.2L21 8|M21 3v5h-5|M21 12a9 9 0 0 1-15.5 6.2L3 16|M3 21v-5h5',
    folder:
      'M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z',
    dropIn: 'M12 3v12|m7 10 5 5 5-5|M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4',

    // 主题 / Theme
    sun: 'M12 16a4 4 0 1 0 0-8 4 4 0 0 0 0 8Z|M12 2v2|M12 20v2|m4.9 4.9 1.4 1.4|m17.7 17.7 1.4 1.4|M2 12h2|M20 12h2|m6.3 17.7-1.4 1.4|m19.1 4.9-1.4 1.4',
    moon: 'M20.4 14.9A8.5 8.5 0 0 1 9.1 3.6a8.5 8.5 0 1 0 11.3 11.3Z',

    // 状态 / Status
    check: 'M20 6 9 17l-5-5',
    checkCircle: 'M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20Z|m8.5 12 2.5 2.5 4.5-5',
    close: 'M18 6 6 18|m6 6 12 12',
    info: 'M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20Z|M12 16v-4.5|M12 8h.01',
    warn: 'M10.3 3.6 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3l-8.5-14.4a2 2 0 0 0-3.4 0Z|M12 9v4|M12 17h.01',
    error: 'M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20Z|M12 7.5V13|M12 16.5h.01',

    // 表单 / Forms
    eye: 'M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7-10-7-10-7Z|M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z',
    eyeOff:
      'M9.9 9.9a3 3 0 0 0 4.2 4.2|M10.7 5.1A10.5 10.5 0 0 1 12 5c6.4 0 10 7 10 7a13.4 13.4 0 0 1-2.2 3.1|M6.6 6.6A13.6 13.6 0 0 0 2 12s3.6 7 10 7a9.8 9.8 0 0 0 4.6-1.1|m2 2 20 20',
    shield:
      'M20 12.5c0 5-3.5 7.5-7.7 9a1 1 0 0 1-.66 0C7.5 20 4 17.5 4 12.5V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.2-2.7a1.2 1.2 0 0 1 1.5 0C14.5 3.8 17 5 19 5a1 1 0 0 1 1 1Z',
    layers:
      'M12.8 2.2a2 2 0 0 0-1.7 0L2.6 6.1a1 1 0 0 0 0 1.8l8.6 3.9a2 2 0 0 0 1.7 0l8.6-3.9a1 1 0 0 0 0-1.8Z|m22 12.7-9.2 4.1a2 2 0 0 1-1.7 0L2 12.7|m22 17.7-9.2 4.1a2 2 0 0 1-1.7 0L2 17.7',
    cpu: 'M6 6h12v12H6z|M9 2v3|M15 2v3|M9 19v3|M15 19v3|M2 9h3|M2 15h3|M19 9h3|M19 15h3|M10 10h4v4h-4z',
    sliders: 'M20 7h-8|M8 7H4|M20 17h-4|M12 17H4|M10 4.5v5|M16 14.5v5',
    chevronDown: 'm6 9 6 6 6-6',
    chevronRight: 'm9 18 6-6-6-6',
    chevronLeft: 'm15 18-6-6 6-6',

    // 文件类型 / File types
    file: 'M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8Z|M14 2v6h6',
    text: 'M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8Z|M14 2v6h6|M9 13h6|M9 17h6',
    code: 'M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8Z|M14 2v6h6|m10 12-2 2 2 2|m14 12 2 2-2 2',
    archive: 'M3 4h18v4H3z|M5 8v11a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8|M10 12h4',
    app: 'M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z|M12 2v3|M12 19v3|M22 12h-3|M5 12H2|m18.4 5.6-2.1 2.1|m7.7 7.7-2.1 2.1|m18.4 18.4-2.1-2.1|m7.7 16.3-2.1-2.1',
    image: 'M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2Z|M9.5 10a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3Z|m21 15-5-5L5 21',
    video: 'M3 6h11a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V6Z|m16 10 5-3v10l-5-3',
    audio: 'M9 18V5l12-2v13|M6 21a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z|M18 19a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z'
  } as const;

  export type IconName = keyof typeof ICONS;
</script>

<script lang="ts">
  interface Props {
    name: IconName;
    /** 像素尺寸；与相邻文本行高对齐即可。 / Pixel size; just match the line height of adjacent text. */
    size?: number;
    stroke?: number;
    class?: string;
  }

  let { name, size = 18, stroke = 1.75, class: klass = '' }: Props = $props();

  let paths = $derived(ICONS[name].split('|'));
</script>

<svg
  class="shrink-0 {klass}"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={stroke}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
  focusable="false"
>
  {#each paths as d (d)}
    <path {d} />
  {/each}
</svg>
