use emu86_core::DriveNumber;

/// Menu actions that can be triggered
#[derive(Debug, Clone, Copy)]
pub enum MenuAction {
    InsertFloppyA,
    EjectFloppyA,
    InsertFloppyB,
    EjectFloppyB,
}

impl MenuAction {
    /// Get the drive number for this action
    pub fn drive_number(&self) -> DriveNumber {
        match self {
            MenuAction::InsertFloppyA | MenuAction::EjectFloppyA => DriveNumber::floppy_a(),
            MenuAction::InsertFloppyB | MenuAction::EjectFloppyB => DriveNumber::floppy_b(),
        }
    }

    /// Check if this is an insert action
    pub fn is_insert(&self) -> bool {
        matches!(self, MenuAction::InsertFloppyA | MenuAction::InsertFloppyB)
    }
}

/// Application menu structure
pub struct AppMenu {
    floppy_a_present: bool,
    floppy_b_present: bool,
}

impl AppMenu {
    /// Create a new menu
    pub fn new() -> Self {
        Self {
            floppy_a_present: false,
            floppy_b_present: false,
        }
    }

    /// Update menu item states based on disk presence
    pub fn update_menu_states(&mut self, floppy_a_present: bool, floppy_b_present: bool) {
        self.floppy_a_present = floppy_a_present;
        self.floppy_b_present = floppy_b_present;
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
            });
        });

        action
    }
}
