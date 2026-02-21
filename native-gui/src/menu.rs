use emu86_core::DriveNumber;

/// Menu actions that can be triggered
#[derive(Debug, Clone, Copy)]
pub enum MenuAction {
    InsertFloppyA,
    EjectFloppyA,
    InsertFloppyB,
    EjectFloppyB,
    InsertCdRom,
    EjectCdRom,
    Reset,
    SaveScreenshot,
    SaveRam,
    SaveVideoRam,
    ToggleExecutionLogging,
    ToggleInterruptLogging,
    TogglePause,
    TogglePerformanceOverlay,
}

impl MenuAction {
    /// Get the drive number for this action (only valid for floppy actions)
    pub fn drive_number(&self) -> DriveNumber {
        match self {
            MenuAction::InsertFloppyA | MenuAction::EjectFloppyA => DriveNumber::floppy_a(),
            MenuAction::InsertFloppyB | MenuAction::EjectFloppyB => DriveNumber::floppy_b(),
            _ => unreachable!("drive_number() called on non-floppy action"),
        }
    }

    /// Check if this is an insert action
    pub fn is_insert(&self) -> bool {
        matches!(self, MenuAction::InsertFloppyA | MenuAction::InsertFloppyB)
    }

    /// Check if this is a debug action
    pub fn is_debug_action(&self) -> bool {
        matches!(
            self,
            MenuAction::ToggleExecutionLogging
                | MenuAction::ToggleInterruptLogging
                | MenuAction::TogglePause
                | MenuAction::TogglePerformanceOverlay
                | MenuAction::Reset
                | MenuAction::SaveScreenshot
                | MenuAction::SaveRam
                | MenuAction::SaveVideoRam
        )
    }
}

/// Application menu structure
pub struct AppMenu {
    floppy_a_present: bool,
    floppy_b_present: bool,
    cdrom_present: bool,
    exec_logging_enabled: bool,
    interrupt_logging_enabled: bool,
    is_paused: bool,
    show_performance_overlay: bool,
}

impl AppMenu {
    /// Create a new menu
    pub fn new() -> Self {
        Self {
            floppy_a_present: false,
            floppy_b_present: false,
            cdrom_present: false,
            exec_logging_enabled: false,
            interrupt_logging_enabled: false,
            is_paused: false,
            show_performance_overlay: false,
        }
    }

    /// Update menu item states based on disk presence
    pub fn update_menu_states(&mut self, floppy_a_present: bool, floppy_b_present: bool) {
        self.floppy_a_present = floppy_a_present;
        self.floppy_b_present = floppy_b_present;
    }

    /// Update CD-ROM present state
    pub fn update_cdrom_state(&mut self, present: bool) {
        self.cdrom_present = present;
    }

    /// Update debug menu states
    pub fn update_debug_states(
        &mut self,
        exec_logging: bool,
        interrupt_logging: bool,
        paused: bool,
        show_overlay: bool,
    ) {
        self.exec_logging_enabled = exec_logging;
        self.interrupt_logging_enabled = interrupt_logging;
        self.is_paused = paused;
        self.show_performance_overlay = show_overlay;
    }

    /// Render the menu bar using egui and return any triggered action
    pub fn render(&mut self, ctx: &egui::Context) -> Option<MenuAction> {
        let mut action = None;

        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("Floppy", |ui| {
                    ui.menu_button("Floppy A:", |ui| {
                        if ui
                            .add_enabled(
                                true,
                                egui::Button::new("Insert Disk...").shortcut_text("Ctrl+Shift+A"),
                            )
                            .clicked()
                        {
                            action = Some(MenuAction::InsertFloppyA);
                            ui.close_menu();
                        }

                        if ui
                            .add_enabled(
                                self.floppy_a_present,
                                egui::Button::new("Eject Disk").shortcut_text("Ctrl+Alt+A"),
                            )
                            .clicked()
                        {
                            action = Some(MenuAction::EjectFloppyA);
                            ui.close_menu();
                        }
                    });

                    ui.menu_button("Floppy B:", |ui| {
                        if ui
                            .add_enabled(
                                true,
                                egui::Button::new("Insert Disk...").shortcut_text("Ctrl+Shift+B"),
                            )
                            .clicked()
                        {
                            action = Some(MenuAction::InsertFloppyB);
                            ui.close_menu();
                        }

                        if ui
                            .add_enabled(
                                self.floppy_b_present,
                                egui::Button::new("Eject Disk").shortcut_text("Ctrl+Alt+B"),
                            )
                            .clicked()
                        {
                            action = Some(MenuAction::EjectFloppyB);
                            ui.close_menu();
                        }
                    });
                });

                ui.menu_button("CD-ROM", |ui| {
                    if ui.button("Insert ISO...").clicked() {
                        action = Some(MenuAction::InsertCdRom);
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(self.cdrom_present, egui::Button::new("Eject CD-ROM"))
                        .clicked()
                    {
                        action = Some(MenuAction::EjectCdRom);
                        ui.close_menu();
                    }
                });

                ui.menu_button("System", |ui| {
                    if ui
                        .button("🔄 Reset")
                        .on_hover_text("Reset and reboot the computer")
                        .clicked()
                    {
                        action = Some(MenuAction::Reset);
                        ui.close_menu();
                    }

                    ui.separator();

                    if ui
                        .button("📷 Save Screenshot...")
                        .on_hover_text("Save screen as PNG image")
                        .clicked()
                    {
                        action = Some(MenuAction::SaveScreenshot);
                        ui.close_menu();
                    }

                    if ui
                        .button("💾 Save RAM...")
                        .on_hover_text("Save entire RAM to file")
                        .clicked()
                    {
                        action = Some(MenuAction::SaveRam);
                        ui.close_menu();
                    }

                    if ui
                        .button("🖥️ Save Video RAM...")
                        .on_hover_text("Save video RAM to file")
                        .clicked()
                    {
                        action = Some(MenuAction::SaveVideoRam);
                        ui.close_menu();
                    }
                });

                ui.menu_button("Debug", |ui| {
                    // Execution Logging with checkbox
                    let mut b = self.exec_logging_enabled;
                    if ui.checkbox(&mut b, "Execution Logging").clicked() {
                        action = Some(MenuAction::ToggleExecutionLogging);
                        ui.close_menu();
                    }

                    // Interrupt Logging with checkbox
                    let mut b = self.interrupt_logging_enabled;
                    if ui.checkbox(&mut b, "Interrupt Logging").clicked() {
                        action = Some(MenuAction::ToggleInterruptLogging);
                        ui.close_menu();
                    }

                    ui.separator();

                    // Performance overlay with checkbox
                    let mut b = self.show_performance_overlay;
                    if ui.checkbox(&mut b, "Performance Overlay").clicked() {
                        action = Some(MenuAction::TogglePerformanceOverlay);
                        ui.close_menu();
                    }

                    ui.separator();

                    // Pause/Run with dynamic label
                    let pause_label = if self.is_paused {
                        "▶ Run"
                    } else {
                        "⏸ Pause"
                    };
                    if ui.button(pause_label).clicked() {
                        action = Some(MenuAction::TogglePause);
                        ui.close_menu();
                    }
                });
            });
        });

        action
    }
}
