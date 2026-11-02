const { contextBridge, ipcRenderer } = require('electron');
contextBridge.exposeInMainWorld('orbi', { onDevice: (cb) => ipcRenderer.on('device', (_e, d) => cb(d)) });
