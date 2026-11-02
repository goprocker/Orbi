import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { bud, chargingCase, plugGlyph, usbGlyph, wifiGlyph } from "./art";

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

/** Health colour for "more is better" values (battery, signal). */
const health = (v: number) => (v <= 20 ? "var(--flare)" : v <= 50 ? "var(--amber)" : "var(--ion)");
/** Colour for "more is worse" values (disk used). */
const pressure = (v: number) => (v >= 90 ? "var(--flare)" : v >= 75 ? "var(--amber)" : "var(--ion)");

const BOLT = `<svg class="bolt" viewBox="0 0 10 14" aria-label="Charging"><path d="M6 0 0 8h4l-1 6 7-9H6z" fill="currentColor"/></svg>`;

/**
 * An object inside its orbit ring. The arc length is `value` (0-100) and a glowing dot rides its
 * end. Everything animates from zero through the CSS custom property --p, set after first paint.
 */
function orbit(object: string, value: number, color: string, size: number, index: number): string {
  return `
    <svg class="orbit" style="--c:${color};--d:${index * 110}ms;width:${size}px;height:${size}px" data-p="${value / 100}" viewBox="0 0 96 96" role="img">
      <circle class="track" cx="48" cy="48" r="42" pathLength="100"/>
      <circle class="arc" cx="48" cy="48" r="42" pathLength="100" transform="rotate(-90 48 48)"/>
      <g class="rider"><circle class="halo" cx="48" cy="6" r="6"/><circle class="dot" cx="48" cy="6" r="3.2"/></g>
      ${object}
    </svg>`;
}

const percent = (n: number) => `<span class="num" data-count="${n}">0</span><span class="unit">%</span>`;

function lowest(parts: Part[]): number {
  return Math.min(...parts.map((p) => p.level));
}

function partsView(e: DeviceEvent): string {
  const cells = e.parts
    .map((p, i) => {
      const color = health(p.level);
      const id = `${p.key}${i}`;
      const object =
        p.key === "case"
          ? chargingCase(id, color)
          : bud(id, p.key === "right");
      const name = { left: "Left", right: "Right", case: "Case", main: "" }[p.key];
      return `
        <div class="cell">
          ${orbit(object, p.level, color, 92, i)}
          <div class="reading">${percent(p.level)}${p.charging ? BOLT : ""}</div>
          <div class="cap">${name}</div>
        </div>`;
    })
    .join("");
  const low = lowest(e.parts) <= 20;
  return `
    <div class="head"><span class="name">${e.name}</span><span class="state ${low ? "warn" : ""}">${low ? "Low battery" : "Connected"}</span></div>
    <div class="row">${cells}</div>`;
}

function singleView(e: DeviceEvent, ring: string, value: string, status: string, warn = false): string {
  return `
    <div class="solo">
      ${ring}
      <div class="info">
        <div class="name">${e.name}</div>
        <div class="reading big">${value}</div>
        <div class="state ${warn ? "warn" : ""}">${status}</div>
      </div>
    </div>`;
}

function bluetoothView(e: DeviceEvent): string {
  if (e.parts.length > 1) return partsView(e);
  const p = e.parts[0] ?? { key: "main", level: 0, charging: false };
  const ring = orbit(bud("main", false, 1.15), p.level, health(p.level), 112, 0);
  const status = p.charging ? "Charging" : p.level <= 20 ? "Low battery" : "Connected";
  return singleView(e, ring, percent(p.level) + (p.charging ? BOLT : ""), status, p.level <= 20 && !p.charging);
}

function wifiView(e: DeviceEvent): string {
  const signal = e.metrics.signal ?? 0;
  const ring = orbit(wifiGlyph, signal, health(signal), 112, 0);
  const strength = signal > 75 ? "Excellent signal" : signal > 50 ? "Good signal" : signal > 25 ? "Fair signal" : "Weak signal";
  return singleView(e, ring, `<span class="num txt">${e.metrics.speed ?? "Connected"}</span>`, strength, signal <= 25);
}

function ethernetView(e: DeviceEvent): string {
  const ring = orbit(plugGlyph, 100, "var(--ion)", 112, 0);
  return singleView(e, ring, `<span class="num txt">${e.metrics.speed ?? "Connected"}</span>`, "Cable connected");
}

function usbView(e: DeviceEvent): string {
  const used = e.metrics.used_pct ?? 0;
  const ring = orbit(usbGlyph, used, pressure(used), 112, 0);
  return singleView(e, ring, `<span class="num txt">${e.metrics.free_space ?? "Connected"}</span>`, used >= 90 ? "Almost full" : `${used}% used`, used >= 90);
}

const views = { bluetooth: bluetoothView, wifi: wifiView, ethernet: ethernetView, usb: usbView };

function render(e: DeviceEvent) {
  card.innerHTML = `<button class="close" aria-label="Dismiss">&#10005;</button>${views[e.type](e)}`;
  card.hidden = false;
  card.querySelector(".close")?.addEventListener("click", () => invoke("popup_close"));

  // two frames so the browser paints the empty state first, then the orbits sweep in
  requestAnimationFrame(() =>
    requestAnimationFrame(() => {
      card.querySelectorAll<SVGSVGElement>("svg.orbit").forEach((svg) => svg.style.setProperty("--p", svg.dataset.p ?? "0"));
      countUp();
    }),
  );
}

/** Numbers count up while the orbit sweeps, so the arc and the digits arrive together. */
function countUp() {
  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;
  card.querySelectorAll<HTMLElement>(".num[data-count]").forEach((el) => {
    const target = Number(el.dataset.count);
    if (reduce) {
      el.textContent = String(target);
      return;
    }
    const start = performance.now();
    const duration = 1100;
    const step = (now: number) => {
      const t = Math.min(1, (now - start) / duration);
      el.textContent = String(Math.round(target * (1 - Math.pow(1 - t, 3))));
      if (t < 1) requestAnimationFrame(step);
    };
    requestAnimationFrame(step);
  });
}

card.addEventListener("mouseenter", () => invoke("popup_hover", { hovered: true }));
card.addEventListener("mouseleave", () => invoke("popup_hover", { hovered: false }));

listen<DeviceEvent>("device", (ev) => render(ev.payload));
invoke<DeviceEvent | null>("popup_ready").then((ev) => ev && render(ev));
