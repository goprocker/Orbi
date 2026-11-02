const { execFile } = require('child_process');

// DEVPKEY_Bluetooth_Battery: percent reported by Windows for paired BT devices
const SCRIPT = `
$key = '{104EA319-6EE2-4701-BD47-8DDBF425BBE5} 2'
Get-PnpDevice -Class Bluetooth -PresentOnly -ErrorAction SilentlyContinue | ForEach-Object {
  $p = Get-PnpDeviceProperty -InstanceId $_.InstanceId -KeyName $key -ErrorAction SilentlyContinue
  if ($p -and $null -ne $p.Data) { [pscustomobject]@{ id = $_.InstanceId; name = $_.FriendlyName; level = [int]$p.Data } }
} | ConvertTo-Json -Compress
`;

function readDevices() {
  return new Promise((resolve) => {
    execFile('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', SCRIPT], { timeout: 15000 }, (err, out) => {
      if (err || !out.trim()) return resolve([]);
      try {
        const data = JSON.parse(out);
        resolve(Array.isArray(data) ? data : [data]);
      } catch {
        resolve([]);
      }
    });
  });
}

let tick = 0;
function mockDevices() {
  tick++;
  return [{ id: 'mock-1', name: 'Orbi Buds', level: Math.max(5, 82 - tick * 3) }];
}

module.exports = { readDevices, mockDevices };
