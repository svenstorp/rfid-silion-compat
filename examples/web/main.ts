import init, { SilionReader, bytesToHex } from "./pkg/rfidlibrs.js";

let reader: SilionReader | null = null;
let inventoryLoopRunning = false;
let stopInProgress = false;

const logEl = document.getElementById("log");
if (!logEl) throw new Error("missing #log element");

function log(line: string): void {
  const now = new Date().toISOString().slice(11, 19);
  (logEl as HTMLElement).textContent += `[${now}] ${line}\n`;
  (logEl as HTMLElement).scrollTop = (logEl as HTMLElement).scrollHeight;
}

function req(id: string): HTMLElement {
  const el = document.getElementById(id);
  if (!el) throw new Error(`missing element #${id}`);
  return el;
}

function setButtonsDisabled(disabled: boolean): void {
  document.querySelectorAll("button").forEach((el) => {
    (el as HTMLButtonElement).disabled = disabled;
  });
}

async function runWithStopLock(task: () => Promise<void>): Promise<void> {
  if (stopInProgress) {
    log("stop already in progress");
    return;
  }

  stopInProgress = true;
  setButtonsDisabled(true);
  try {
    await task();
  } finally {
    stopInProgress = false;
    setButtonsDisabled(false);
  }
}

async function pumpInventory(): Promise<void> {
  if (!reader || !reader.isInventoryRunning()) return;
  inventoryLoopRunning = true;
  try {
    while (reader && reader.isInventoryRunning()) {
      const msg = await reader.recvInventoryMessage();
      switch (msg.kind) {
        case "tagInformation": {
          const epc = bytesToHex(msg.tag.epcId);
          const rssi = msg.tag.rssiDbm ?? "n/a";
          const ant = msg.tag.antennaId ?? "n/a";
          log(`TAG epc=${epc} rssi=${rssi} ant=${ant}`);
          break;
        }
        case "heartbeat": {
          log(`HEARTBEAT flags=0x${Number(msg.searchFlags).toString(16)}`);
          break;
        }
        case "startAck":
          log("START ACK");
          break;
        case "stopAck":
          log("STOP ACK");
          break;
        case "subcommand":
          log(`SUBCOMMAND 0x${Number(msg.subcommand).toString(16)}`);
          break;
        default:
          log(`UNKNOWN ${JSON.stringify(msg)}`);
      }
    }
  } catch (err) {
    if (String(err).includes("inventory receive aborted") && reader && !reader.isInventoryRunning()) {
      return;
    }
    log(`inventory loop error: ${err}`);
  } finally {
    inventoryLoopRunning = false;
  }
}

async function main(): Promise<void> {
  await init();
  log("WASM initialized.");

  req("connect").addEventListener("click", async () => {
    try {
      const baud = Number((req("baud") as HTMLInputElement).value || 115200);
      reader = await SilionReader.connect(baud);
      log(`connected @ ${baud}`);
    } catch (err) {
      log(`connect failed: ${err}`);
    }
  });

  req("version").addEventListener("click", async () => {
    if (!reader) return log("not connected");
    try {
      const v = await reader.getVersion();
      log(
        `version fw=${bytesToHex(v.firmwareVersion)} bl=${bytesToHex(v.bootloaderVersion)} hw=${bytesToHex(v.hardwareVersion)}`
      );
    } catch (err) {
      log(`getVersion failed: ${err}`);
    }
  });

  req("transact").addEventListener("click", async () => {
    if (!reader) return log("not connected");
    try {
      const resp = await reader.transact(0x03, new Uint8Array());
      log(`transact 0x03 payload=${bytesToHex(resp)}`);
    } catch (err) {
      log(`transact failed: ${err}`);
    }
  });

  req("start").addEventListener("click", async () => {
    if (!reader) return log("not connected");
    try {
      await reader.startInventory();
      log("inventory started");
      if (!inventoryLoopRunning) {
        void pumpInventory();
      }
    } catch (err) {
      log(`startInventory failed: ${err}`);
    }
  });

  req("stop").addEventListener("click", async () => {
    if (!reader) return log("not connected");
    log("stopping inventory...");
    await runWithStopLock(async () => {
      try {
        await reader!.stopInventory();
        log("inventory stopped");
      } catch (err) {
        log(`stopInventory failed: ${err}`);
      }
    });
  });

  req("close").addEventListener("click", async () => {
    if (!reader) return log("not connected");
    try {
      await reader.close();
      reader = null;
      log("closed");
    } catch (err) {
      log(`close failed: ${err}`);
    }
  });

  // Reader Info
  req("getSerialNumber").addEventListener("click", async () => {
    if (!reader) return log("not connected");
    try {
      const sn = await reader.getSerialNumber(0, 0);
      log(`serial number: ${JSON.stringify(sn)}`);
    } catch (err) {
      log(`getSerialNumber failed: ${err}`);
    }
  });

  req("getCurrentTagProtocol").addEventListener("click", async () => {
    if (!reader) return log("not connected");
    try {
      const proto = await reader.getCurrentTagProtocol();
      log(`current tag protocol: 0x${Number(proto).toString(16)}`);
    } catch (err) {
      log(`getCurrentTagProtocol failed: ${err}`);
    }
  });

  req("getRunPhase").addEventListener("click", async () => {
    if (!reader) return log("not connected");
    try {
      const phase = await reader.getRunPhase();
      log(`run phase: ${JSON.stringify(phase)}`);
    } catch (err) {
      log(`getRunPhase failed: ${err}`);
    }
  });

  req("getCurrentTemperature").addEventListener("click", async () => {
    if (!reader) return log("not connected");
    try {
      const temp = await reader.getCurrentTemperature();
      log(`current temperature: ${Number(temp)}°C`);
    } catch (err) {
      log(`getCurrentTemperature failed: ${err}`);
    }
  });

  // Region
  req("getCurrentRegion").addEventListener("click", async () => {
    if (!reader) return log("not connected");
    try {
      const region = await reader.getCurrentRegion();
      log(`current region: ${JSON.stringify(region)}`);
    } catch (err) {
      log(`getCurrentRegion failed: ${err}`);
    }
  });

  req("setCurrentRegion").addEventListener("click", async () => {
    if (!reader) return log("not connected");
    try {
      const codeStr = prompt("Enter region code (0=NorthAmerica, 1=Europe, ...):");
      if (codeStr === null) return;
      const code = Number(codeStr);
      await reader.setCurrentRegion(code);
      log(`set region to code=${code}`);
    } catch (err) {
      log(`setCurrentRegion failed: ${err}`);
    }
  });

  req("getAvailableRegions").addEventListener("click", async () => {
    if (!reader) return log("not connected");
    try {
      const regions = await reader.getAvailableRegions();
      log(`available regions: ${JSON.stringify(regions)}`);
    } catch (err) {
      log(`getAvailableRegions failed: ${err}`);
    }
  });

  // GPIO
  req("getGpi").addEventListener("click", async () => {
    if (!reader) return log("not connected");
    try {
      const gpi = await reader.getGpi();
      log(`GPI: 0x${Number(gpi).toString(16)}`);
    } catch (err) {
      log(`getGpi failed: ${err}`);
    }
  });

  req("getGpoStates").addEventListener("click", async () => {
    if (!reader) return log("not connected");
    try {
      const gpo = await reader.getGpoStates();
      log(`GPO states: ${JSON.stringify(gpo)}`);
    } catch (err) {
      log(`getGpoStates failed: ${err}`);
    }
  });

  // Antenna
  req("getAntennaPorts").addEventListener("click", async () => {
    if (!reader) return log("not connected");
    try {
      const ports = await reader.getAntennaPorts(0);
      log(`antenna ports (option=0): ${JSON.stringify(ports)}`);
    } catch (err) {
      log(`getAntennaPorts failed: ${err}`);
    }
  });

  req("setAntennaAccessPair").addEventListener("click", async () => {
    if (!reader) return log("not connected");
    try {
      const port1Str = prompt("Port 1 (1-4):");
      if (port1Str === null) return;
      const port2Str = prompt("Port 2 (1-4):");
      if (port2Str === null) return;
      const port1 = Number(port1Str);
      const port2 = Number(port2Str);
      await reader.setAntennaAccessPair(port1, port2);
      log(`set access pair: port1=${port1}, port2=${port2}`);
    } catch (err) {
      log(`setAntennaAccessPair failed: ${err}`);
    }
  });

  req("setAntennaInventoryPairs").addEventListener("click", async () => {
    if (!reader) return log("not connected");
    try {
      const pairsStr = prompt("Inventory pairs (e.g., '1,2,3,4' for [1,2],[3,4]):");
      if (pairsStr === null) return;
      const ports = pairsStr.split(",").map((p) => Number(p.trim()));
      if (ports.length % 2 !== 0) return log("must provide even number of ports");
      const pairsData = new Uint8Array(ports);
      await reader.setAntennaInventoryPairs(pairsData);
      log(`set inventory pairs: ${Array.from(pairsData).join(",")}`);
    } catch (err) {
      log(`setAntennaInventoryPairs failed: ${err}`);
    }
  });

  req("setAntennaPower").addEventListener("click", async () => {
    if (!reader) return log("not connected");
    try {
      const txStr = prompt("TX antenna (port):");
      if (txStr === null) return;
      const readPowerStr = prompt("Read power (0.01 dBm units, e.g., 3000 = 30 dBm):");
      if (readPowerStr === null) return;
      const writePowerStr = prompt("Write power (0.01 dBm units, e.g., 3000 = 30 dBm):");
      if (writePowerStr === null) return;
      const tx = Number(txStr);
      const readPower = Number(readPowerStr);
      const writePower = Number(writePowerStr);
      await reader.setAntennaPower(tx, readPower, writePower);
      log(`set antenna power: tx=${tx}, readPower=${readPower}, writePower=${writePower}`);
    } catch (err) {
      log(`setAntennaPower failed: ${err}`);
    }
  });

  // RF / Config
  req("getFrequencyHoppingTable").addEventListener("click", async () => {
    if (!reader) return log("not connected");
    try {
      const table = await reader.getFrequencyHoppingTable();
      log(`frequency hopping table: ${JSON.stringify(table)}`);
    } catch (err) {
      log(`getFrequencyHoppingTable failed: ${err}`);
    }
  });

  req("getRegulatoryHopTime").addEventListener("click", async () => {
    if (!reader) return log("not connected");
    try {
      const hopTime = await reader.getRegulatoryHopTime();
      log(`regulatory hop time: ${JSON.stringify(hopTime)}`);
    } catch (err) {
      log(`getRegulatoryHopTime failed: ${err}`);
    }
  });

  req("getReaderConfiguration").addEventListener("click", async () => {
    if (!reader) return log("not connected");
    try {
      const optionStr = prompt("Option (0-255):");
      if (optionStr === null) return;
      const keyStr = prompt("Key (0-255):");
      if (keyStr === null) return;
      const config = await reader.getReaderConfiguration(Number(optionStr), Number(keyStr));
      log(`reader configuration: ${JSON.stringify(config)}`);
    } catch (err) {
      log(`getReaderConfiguration failed: ${err}`);
    }
  });

  req("getProtocolConfiguration").addEventListener("click", async () => {
    if (!reader) return log("not connected");
    try {
      const protocolStr = prompt("Protocol value (0-255):");
      if (protocolStr === null) return;
      const paramStr = prompt("Parameter (0-255):");
      if (paramStr === null) return;
      const config = await reader.getProtocolConfiguration(Number(protocolStr), Number(paramStr));
      log(`protocol configuration: ${JSON.stringify(config)}`);
    } catch (err) {
      log(`getProtocolConfiguration failed: ${err}`);
    }
  });

  // Firmware
  req("getRunPhaseForBoot").addEventListener("click", async () => {
    if (!reader) return log("not connected");
    try {
      const phase = await reader.getRunPhase();
      log(`run phase (for boot decision): ${JSON.stringify(phase)}`);
    } catch (err) {
      log(`getRunPhase failed: ${err}`);
    }
  });

  req("bootFirmware").addEventListener("click", async () => {
    if (!reader) return log("not connected");
    try {
      await reader.bootFirmware();
      log("boot firmware: switched to app firmware");
    } catch (err) {
      log(`bootFirmware failed: ${err}`);
    }
  });

  req("bootBootloader").addEventListener("click", async () => {
    if (!reader) return log("not connected");
    try {
      const confirmed = confirm("Boot bootloader? Device will need re-flashing.");
      if (!confirmed) return;
      await reader.bootBootloader();
      log("bootloader mode activated");
    } catch (err) {
      log(`bootBootloader failed: ${err}`);
    }
  });
}

main().catch((err) => {
  log(`fatal: ${err}`);
});
