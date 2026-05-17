# rfid-silion-compat (JavaScript/TypeScript)

WebAssembly bindings for the Silion RFID reader API (Web Serial).

This package exposes a high-level class, `SilionReader`, for browser apps.

## Install

```bash
npm install rfid-silion-compat
```

## Runtime Requirements

- A browser with Web Serial support
- A user gesture (for example, button click) before calling `SilionReader.connect(...)`

## Quick Start

```ts
import init, { SilionReader, bytesToHex } from "rfid-silion-compat";

await init();

// Must be called from a user gesture handler in browsers.
const reader = await SilionReader.connect(115200);

const version = await reader.getVersion();
console.log("Firmware:", version.firmwareVersion);

const tag = await reader.singleTagInventory(
  1000,
  { type: "disabled" },
  { rssi: true, antennaId: true }
);

console.log("EPC:", bytesToHex(tag.epcId));
```

## Main API

- `SilionReader.connect(baudRate: number): Promise<SilionReader>`
- `reader.getVersion(): Promise<any>`
- `reader.getCurrentRegion(): Promise<{ name: string }>`
- `reader.setCurrentRegion({ name }): Promise<void>`
- `reader.singleTagInventory(timeoutMs, selectOption, metadataFlags?): Promise<any>`
- `reader.readTagData(timeoutMs, selectOption, metadataFlags, memBank, readAddressWords, wordCount): Promise<any>`
- `reader.startInventory(embeddedCommand?): Promise<void>`
- `reader.recvInventoryMessage(): Promise<any>`
- `reader.stopInventory(): Promise<void>`
- `reader.close(): Promise<void>`

Helper exports:

- `bytesToHex(data: Uint8Array | number[]): string`
- `arrayToBytes(values: number[]): Uint8Array`

## Typed Input Shapes

The generated typings include these input types:

- `MetadataFlagsInput`
- `RegionCodeInput` (`{ name: "NorthAmerica" | "China1" | "Europe" | "China2" | "FullFrequencyBand" }`)
- `MemBankInput` (`{ name: "Reserved" | "Epc" | "Tid" | "User" }`)
- `SelectOption`

`SelectOption` is a discriminated union using `type`:

```ts
type SelectOption =
  | { type: "disabled" }
  | { type: "passwordOnly" }
  | { type: "epc"; selectLengthBits: number; selectData: Uint8Array | number[]; invert: boolean }
  | { type: "tid"; selectAddress: number; selectLengthBits: number; selectData: Uint8Array | number[]; invert: boolean }
  | { type: "userMemory"; selectAddress: number; selectLengthBits: number; selectData: Uint8Array | number[]; invert: boolean }
  | { type: "epcBank"; selectAddress: number; selectLengthBits: number; selectData: Uint8Array | number[]; invert: boolean };
```

## Notes

- `startInventory()` switches the reader into async inventory mode.
- While inventory is running, use `recvInventoryMessage()` to consume pushed messages.
- Call `stopInventory()` before command-mode operations that require normal transact flow.
- `close()` will stop inventory first if needed.

## Source

See the Rust bindings implementation in `src/web_bindings.rs` in the main repository.
