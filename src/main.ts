import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface Part {
  key: "left" | "right" | "case" | "main";
  level: number;
  charging: boolean;
}

interface DeviceEvent {
  id: string;
  type: "bluetooth" | "wifi" | "ethernet" | "usb";
  name: string;
  status: "connected" | "disconnected" | "updated";
  parts: Part[];
  metrics: {
    ssid?: string;
    signal?: number;
    speed?: string;
    used_pct?: number;
    free_space?: string;
  };
}

const card = document.getElementById("card") as HTMLDivElement;

const BUD = "M24 2C14 2 8 9 8 18c0 7 4 12 9 14v22a7 7 0 0 0 14 0V32c5-2 9-7 9-14C40 9 34 2 24 2z";
const SHAPES: Record<Part["key"], string> = {
  left: `<g transform="translate(8 0)"><path d="${BUD}"/></g>`,
  right: `<g transform="translate(56 0) scale(-1 1)"><path d="${BUD}"/></g>`,
  case: `<rect x="3" y="14" width="58" height="38" rx="14"/>`,
  main: `<g transform="translate(8 0)"><path d="${BUD}"/></g>`,
};
// vertical extent of each silhouette inside the 64x64 viewBox, so the fill spans only the shape
const EXTENT: Record<Part["key"], [number, number]> = { left: [2, 62], right: [2, 62], main: [2, 62], case: [14, 52] };
const LABELS: Record<Part["key"], string> = { left: "Left", right: "Right", case: "Case", main: "" };

const levelColor = (level: number) => (level <= 20 ? "var(--low)" : level <= 50 ? "var(--warn)" : "var(--ok)");

function partHtml(p: Part, single: boolean): string {
  const [top, bottom] = EXTENT[p.key];
  const y = bottom - (bottom - top) * (p.level / 100);
  const id = `clip-${p.key}`;
  return `
    <div class="part${single ? " single" : ""}">
      <svg viewBox="0 0 64 64" aria-hidden="true">
        <defs><clipPath id="${id}"><rect class="fill" data-y="${y}" x="0" y="0" width="64" height="64" style="transform:translateY(64px)"/></clipPath></defs>
        <g class="shape">${SHAPES[p.key]}</g>
        <g fill="${levelColor(p.level)}" clip-path="url(#${id})">${SHAPES[p.key]}</g>
      </svg>
      <div>${p.level}%${p.charging ? " ⚡" : ""}</div>
      ${LABELS[p.key] ? `<div class="label">${LABELS[p.key]}</div>` : ""}
    </div>`;
}

function bluetoothHtml(e: DeviceEvent): string {
  const single = e.parts.length <= 1;
  const low = e.parts.some((p) => p.level <= 20);
  return `
    <div class="title">${e.name}</div>
    <div class="parts">${e.parts.map((p) => partHtml(p, single)).join("")}</div>
    <div class="sub">${low ? "Low battery" : "Connected"}</div>`;
}

const ICON_PLUG = `<svg viewBox="0 0 64 64" fill="none" stroke="currentColor" stroke-width="5" stroke-linecap="round" stroke-linejoin="round"><rect x="14" y="10" width="36" height="28" rx="6"/><path d="M24 38v8h16v-8M32 46v10M24 20v6M40 20v6"/></svg>`;
const ICON_USB = `<svg viewBox="0 0 64 64" fill="none" stroke="currentColor" stroke-width="5" stroke-linecap="round" stroke-linejoin="round"><rect x="20" y="6" width="24" height="22" rx="4"/><path d="M26 14v4M38 14v4"/><rect x="16" y="28" width="32" height="30" rx="6"/></svg>`;

function wifiHtml(e: DeviceEvent): string {
  const lit = Math.max(1, Math.round(((e.metrics.signal ?? 0) / 100) * 4));
  const bars = [0, 1, 2, 3].map((i) => `<i class="${i < lit ? "on" : ""}" style="height:${16 + i * 16}px"></i>`).join("");
  return `
    <div class="title">${e.metrics.ssid ?? e.name}</div>
    <div class="bars">${bars}</div>
    <div class="sub">Connected${e.metrics.speed ? ` · ${e.metrics.speed}` : ""}</div>`;
}

function ethernetHtml(e: DeviceEvent): string {
  return `
    <div class="title">${e.name}</div>
    <div class="hero">${ICON_PLUG}</div>
    <div class="sub">Connected${e.metrics.speed ? ` · ${e.metrics.speed}` : ""}</div>`;
}

function usbHtml(e: DeviceEvent): string {
  const used = e.metrics.used_pct;
  const meter = used === undefined ? "" : `<div class="meter"><b style="width:${used}%"></b></div>`;
  return `
    <div class="title">${e.name}</div>
    <div class="hero">${ICON_USB}</div>
    ${meter}
    <div class="sub">${e.metrics.free_space ?? "Connected"}</div>`;
}

function render(e: DeviceEvent) {
  const view = { bluetooth: bluetoothHtml, wifi: wifiHtml, ethernet: ethernetHtml, usb: usbHtml }[e.type];
  card.innerHTML = view(e);
  card.hidden = false;
  // two frames so the browser paints the start state before the fill transitions to its level
  requestAnimationFrame(() =>
    requestAnimationFrame(() => {
      card.querySelectorAll<SVGRectElement>("rect.fill").forEach((r) => {
        r.style.transform = `translateY(${r.dataset.y}px)`;
      });
    }),
  );
}

card.addEventListener("mouseenter", () => invoke("popup_hover", { hovered: true }));
card.addEventListener("mouseleave", () => invoke("popup_hover", { hovered: false }));

listen<DeviceEvent>("device", (ev) => render(ev.payload));
invoke<DeviceEvent | null>("popup_ready").then((ev) => ev && render(ev));
