// SVG artwork. Every object is drawn in a 96x96 box so it sits inside an orbit ring (r = 42).
// Gradients are declared once per object with a unique id prefix so several can share a page.


const porcelain = (id: string) => `
  <linearGradient id="${id}-p" x1="0.15" y1="0" x2="0.85" y2="1">
    <stop offset="0" stop-color="#FFFFFF"/>
    <stop offset="0.55" stop-color="#E4E7ED"/>
    <stop offset="1" stop-color="#B5BBC8"/>
  </linearGradient>`;

/** An earbud. `mirror` flips it into the right bud. `led` colours the tiny status light. */
export function bud(id: string, mirror: boolean, scale = 1): string {
  const flip = mirror ? "translate(96 0) scale(-1 1)" : "";
  return `
  <g transform="${flip}">
    <g transform="translate(48 50) scale(${scale}) translate(-48 -50)">
      <defs>${porcelain(id)}</defs>
      <rect x="49" y="42" width="9" height="30" rx="4.5" fill="url(#${id}-p)"/>
      <ellipse cx="45" cy="38" rx="15" ry="14" fill="url(#${id}-p)"/>
      <ellipse cx="37.5" cy="41" rx="6.5" ry="6" fill="#2B2F38"/>
      <ellipse cx="37.5" cy="41" rx="4" ry="3.6" fill="#161920"/>
      <ellipse cx="42" cy="29.5" rx="7" ry="3.2" fill="#fff" opacity="0.7" transform="rotate(-18 42 29.5)"/>
      <rect x="51.2" y="48" width="1.6" height="14" rx="0.8" fill="#9AA1B0" opacity="0.55"/>
    </g>
  </g>`;
}

/** The charging case, with a status light that takes the charge colour. */
export function chargingCase(id: string, led: string, scale = 1): string {
  return `
  <g transform="translate(48 50) scale(${scale}) translate(-48 -50)">
    <defs>${porcelain(id)}</defs>
    <rect x="24" y="34" width="48" height="34" rx="15" fill="url(#${id}-p)"/>
    <path d="M26 49.5 H70" stroke="#9AA1B0" stroke-width="1.2" opacity="0.6"/>
    <rect x="42" y="38.5" width="12" height="2.6" rx="1.3" fill="#fff" opacity="0.8"/>
    <circle cx="48" cy="58.5" r="2.1" fill="${led}"/>
    <circle cx="48" cy="58.5" r="4.6" fill="${led}" opacity="0.22"/>
  </g>`;
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
