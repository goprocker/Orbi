const { app, BrowserWindow, Tray, Menu, nativeImage, screen } = require('electron');
const path = require('path');
const { readDevices, mockDevices } = require('./battery');

const MOCK = process.argv.includes('--mock');
const POLL_MS = 15000;
const HIDE_MS = 6000;

let win, tray, hideTimer;
const known = new Map();

function trayIcon() {
  const size = 16;
  const buf = Buffer.alloc(size * size * 4);
  for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) {
    const d = Math.hypot(x - 7.5, y - 7.5);
    const i = (y * size + x) * 4;
    if (d < 7) { buf[i] = 0xf5; buf[i + 1] = 0x82; buf[i + 2] = 0x3b; buf[i + 3] = 255; } // BGRA
  }
  return nativeImage.createFromBitmap(buf, { width: size, height: size });
}

function createWindow() {
  win = new BrowserWindow({
    width: 340, height: 220, show: false, frame: false, transparent: true,
    resizable: false, skipTaskbar: true, alwaysOnTop: true, focusable: false,
    webPreferences: { preload: path.join(__dirname, 'preload.js') },
  });
  win.loadFile(path.join(__dirname, 'index.html'));
}

function popup(device) {
  const { workArea } = screen.getPrimaryDisplay();
  win.setPosition(Math.round(workArea.x + workArea.width / 2 - 170), workArea.y + workArea.height - 240);
  win.webContents.send('device', device);
  win.showInactive();
  clearTimeout(hideTimer);
  hideTimer = setTimeout(() => win.hide(), HIDE_MS);
}

async function poll() {
  const devices = MOCK ? mockDevices() : await readDevices();
  const seen = new Set();
  for (const d of devices) {
    seen.add(d.id);
    if (!known.has(d.id)) popup(d); // newly connected
    known.set(d.id, d.level);
  }
  for (const id of known.keys()) if (!seen.has(id)) known.delete(id);
  tray.setToolTip(devices.length ? devices.map((d) => `${d.name}: ${d.level}%`).join('\n') : 'Orbi: no earbuds');
}

app.whenReady().then(() => {
  createWindow();
  tray = new Tray(trayIcon());
  tray.setContextMenu(Menu.buildFromTemplate([
    { label: 'Show battery now', click: () => { known.clear(); poll(); } },
    { label: 'Quit Orbi', click: () => app.quit() },
  ]));
  poll();
  setInterval(poll, POLL_MS);
});

app.on('window-all-closed', (e) => e.preventDefault());
