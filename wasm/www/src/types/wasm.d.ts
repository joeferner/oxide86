// Type definitions for the WASM module

export interface Emu86Computer {
  // Initialization
  attach_serial_mouse_com1(): void;

  // Boot control
  boot(driveNumber: number): void;
  reset(): void;

  // Execution
  run_for_ms(ms: number, timestamp: number): boolean;
  step(): boolean;

  // Performance monitoring
  get_target_mhz(): number;
  get_actual_mhz(): number;

  // Drive management
  load_floppy(slot: number, data: Uint8Array): void;
  eject_floppy(slot: number): void;
  add_hard_drive(data: Uint8Array): void;

  // Input handling
  handle_key_event(
    code: string,
    key: string,
    shiftKey: boolean,
    ctrlKey: boolean,
    altKey: boolean
  ): void;
  handle_mouse_move(x: number, y: number): void;
  handle_mouse_delta(dx: number, dy: number): void;
  handle_mouse_button(button: number, pressed: boolean): void;
}

export interface WasmModule {
  Emu86Computer: new (canvasId: string) => Emu86Computer;
  default(): Promise<void>;
}

declare module '../../pkg/emu86_wasm.js' {
  const wasmModule: WasmModule;
  export = wasmModule;
}
