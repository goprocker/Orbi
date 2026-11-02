// SVG artwork. Every object is drawn in a 96x96 box so it sits inside an orbit ring (r = 42).
// Objects follow the shape language of modern stem earbuds: a pill-shaped stem with a chrome
// band, a rounded head, a silicone ear tip, a tiny LED, and a clamshell case with an open lid.

/** Colours of one product finish. */
export interface Pack {
  hi: string;
  mid: string;
  lo: string;
  /** silicone ear tip */
  tip: string;
  /** the tip's opening and the case interior */
  inner: string;
  /** chrome band on the stem */
  band: string;
}

const NEUTRAL: Pack = { hi: "#FFFFFF", mid: "#E4E7ED", lo: "#B5BBC8", tip: "#3A3F4A", inner: "#161920", band: "#8F96A6" };

/** Art packs by device name. Add a row to give another product its own finish. */
const PACKS: { match: RegExp; pack: Pack }[] = [
  { match: /dubstep/i, pack: { hi: "#F9CDD0", mid: "#EFA3AA", lo: "#D2727C", tip: "#D8495A", inner: "#7A1624", band: "#B8505F" } },
];

export function packFor(deviceName: string): Pack {
  return PACKS.find((p) => p.match.test(deviceName))?.pack ?? NEUTRAL;
}

const body = (id: string, p: Pack) => `
  <linearGradient id="${id}-b" x1="0.1" y1="0" x2="0.9" y2="1">
    <stop offset="0" stop-color="${p.hi}"/>
    <stop offset="0.55" stop-color="${p.mid}"/>
    <stop offset="1" stop-color="${p.lo}"/>
  </linearGradient>
  <linearGradient id="${id}-c" x1="0" y1="0" x2="0" y2="1">
    <stop offset="0" stop-color="${p.mid}"/>
    <stop offset="0.5" stop-color="${p.band}"/>
    <stop offset="1" stop-color="${p.lo}"/>
  </linearGradient>`;

/**
 * An earbud seen from the front: pill stem, head and ear tip peeking out to the side.
 * `mirror` turns it into the right bud.
 */
export function bud(id: string, mirror: boolean, scale = 1, pack: Pack = NEUTRAL): string {
  const flip = mirror ? "translate(96 0) scale(-1 1)" : "";
  return `
  <g transform="${flip}">
    <g transform="translate(48 50) scale(${scale}) translate(-48 -50)">
      <defs>${body(id, pack)}</defs>
      <ellipse cx="41" cy="40" rx="17" ry="15" fill="url(#${id}-b)"/>
      <ellipse cx="29" cy="42" rx="10.5" ry="10" fill="${pack.tip}"/>
      <ellipse cx="29" cy="42" rx="5.6" ry="5.2" fill="${pack.inner}"/>
      <rect x="40" y="20" width="19" height="53" rx="9.5" fill="url(#${id}-b)"/>
      <path d="M40 58 H59 V63.5 a9.5 9.5 0 0 1 -19 0 Z" fill="url(#${id}-c)"/>
      <rect x="42.5" y="22.5" width="6" height="22" rx="3" fill="#fff" opacity="0.38"/>
      <circle cx="49.5" cy="29" r="1.6" fill="#4C6BFF"/>
      <circle cx="49.5" cy="64.5" r="1.1" fill="${pack.inner}" opacity="0.75"/>
    </g>
  </g>`;
}

/** The charging case, clamshell style with the lid open. The status light takes the charge colour. */
export function chargingCase(id: string, led: string, scale = 1, pack: Pack = NEUTRAL): string {
  return `
  <g transform="translate(48 50) scale(${scale}) translate(-48 -50)">
    <defs>${body(id, pack)}</defs>
    <path d="M24 44 C22 30 30 22 48 22 C66 22 74 30 72 44 Z" fill="${pack.inner}"/>
    <path d="M24 44 C22 30 30 22 48 22 C66 22 74 30 72 44" fill="none" stroke="url(#${id}-b)" stroke-width="3.2" stroke-linecap="round"/>
    <path d="M17 46 C17 43 19 42 22 42 H74 C77 42 79 43 79 46 C79 66 66 76 48 76 C30 76 17 66 17 46 Z" fill="url(#${id}-b)"/>
    <path d="M24 49 C30 47 38 46.5 48 46.5" fill="none" stroke="#fff" stroke-width="2" stroke-linecap="round" opacity="0.45"/>
    <circle cx="48" cy="64" r="2.1" fill="${led}"/>
    <circle cx="48" cy="64" r="4.8" fill="${led}" opacity="0.22"/>
  </g>`;
}

/**
 * The whole product in one scene: both buds standing over the open case. Used when Windows gives
 * only a single battery value, so the popup can still show the full pair and case.
 */
export function earbudsScene(id: string, led: string, pack: Pack = NEUTRAL): string {
  return `
  <g transform="translate(48 60) scale(0.74) translate(-48 -50)">${chargingCase(`${id}c`, led, 1, pack)}</g>
  <g transform="translate(30 31) scale(0.6) translate(-48 -50)">${bud(`${id}l`, false, 1, pack)}</g>
  <g transform="translate(66 31) scale(0.6) translate(-48 -50)">${bud(`${id}r`, true, 1, pack)}</g>`;
}

const stroke = `fill="none" stroke="#ECEEF2" stroke-width="3.6" stroke-linecap="round" stroke-linejoin="round"`;

export const wifiGlyph = `
  <g ${stroke}>
    <path d="M26 42a31 31 0 0 1 44 0"/>
    <path d="M33.5 50.5a20 20 0 0 1 29 0"/>
    <path d="M41 59a9 9 0 0 1 14 0"/>
  </g>
  <circle cx="48" cy="66.5" r="2.6" fill="#ECEEF2"/>`;

export const plugGlyph = `
  <g ${stroke}>
    <rect x="32" y="30" width="32" height="24" rx="6"/>
    <path d="M40 54v6h16v-6M48 60v8M40 38v5M56 38v5"/>
  </g>`;

export const usbGlyph = `
  <g ${stroke}>
    <rect x="37" y="26" width="22" height="18" rx="4"/>
    <path d="M43 33v3M53 33v3"/>
    <rect x="33" y="44" width="30" height="26" rx="7"/>
  </g>`;
