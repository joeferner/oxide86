use crate::{
    bus::Bus,
    cpu::{
        Cpu,
        bios::{BIOS_CODE_SEGMENT, bda::bda_get_ps2_mouse_handler},
    },
    devices::pic::{PIC_COMMAND_EOI, PIC_IO_PORT_COMMAND, PIC2_IO_PORT_COMMAND},
};

/// IP within the BIOS code segment used as the RETF trampoline after the PS/2
/// mouse callback handler returns.  Must be ≤ 0xFF (BIOS dispatch guard) and
/// must not collide with any real INT vector handled in step_bios_int.
pub(in crate::cpu) const PS2_MOUSE_RETURN_IP: u16 = 0xF4;

impl Cpu {
    /// INT 74h — PS/2 mouse hardware interrupt (IRQ12).
    ///
    /// Reads a 3-byte PS/2 packet from the aux port, sends EOIs to both PICs,
    /// then invokes the application handler registered via INT 15h AH=C2h AL=07h
    /// by doing a virtual FAR CALL: pushing a RETF trampoline address then
    /// jumping to the handler.  step() detects the CS change and skips the
    /// normal IRET, letting the trampoline's subsequent invocation clean up.
    pub(in crate::cpu) fn handle_int74_ps2_mouse_interrupt(&mut self, bus: &mut Bus) {
        bus.increment_cycle_count(200);
        // Acknowledge both PICs so IRQ12 can fire again.
        bus.io_write_u8(PIC2_IO_PORT_COMMAND, PIC_COMMAND_EOI);
        bus.io_write_u8(PIC_IO_PORT_COMMAND, PIC_COMMAND_EOI);

        // Read the 3-byte PS/2 mouse packet.
        let b0 = bus.keyboard_controller_mut().aux_read();
        let b1 = bus.keyboard_controller_mut().aux_read();
        let b2 = bus.keyboard_controller_mut().aux_read();

        let (Some(b0), Some(b1), Some(b2)) = (b0, b1, b2) else {
            log::warn!("INT 74h: PS/2 mouse interrupt with < 3 bytes in aux buffer");
            return;
        };

        // Look up the registered handler.
        let (handler_seg, handler_off, mask) = bda_get_ps2_mouse_handler(bus);

        if handler_seg == 0 && handler_off == 0 {
            return; // No handler registered — discard packet.
        }
        let _ = mask; // stored in BDA but not used for per-event filtering

        // Save registers before overwriting them for the callback.
        // A real BIOS uses PUSHA/POPA around the FAR CALL; we emulate that here so
        // that code interrupted between (e.g.) "mov ah, 0x01" and "int 0x16" sees
        // the original AH on return rather than the 0 the callback setup would leave.
        self.saved_for_int74 = Some((self.ax, self.bx, self.cx, self.dx));

        // Set up registers for the handler:
        //   AL = PS/2 status byte (buttons, sign bits, overflow)
        //   BL = X movement (signed byte)
        //   CL = Y movement (signed byte)
        //   DL = Z movement (0 — standard mouse has no wheel)
        //   AH = 0
        self.ax = b0 as u16; // AH=0, AL=status
        self.bx = b1 as u16; // BH=0, BL=X
        self.cx = b2 as u16; // CH=0, CL=Y
        self.dx = 0; // DH=0, DL=Z

        // FAR CALL to the handler: push return address (trampoline), jump.
        self.push(BIOS_CODE_SEGMENT, bus);
        self.push(PS2_MOUSE_RETURN_IP, bus);
        self.set_cs_real(handler_seg);
        self.ip = handler_off;
        // step() will see CS != BIOS_CODE_SEGMENT and skip patch_flags_and_iret.
    }
}
