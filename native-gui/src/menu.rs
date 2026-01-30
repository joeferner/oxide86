use anyhow::{Context, Result};
use emu86_core::DriveNumber;
use muda::{Menu, MenuEvent, MenuItem, Submenu, accelerator::Accelerator};

/// Custom event types for the application
#[derive(Debug)]
pub enum AppEvent {
    MenuEvent(MenuEvent),
    #[allow(dead_code)]
    DiskInserted {
        slot: DriveNumber,
        result: Result<(), String>,
    },
}

/// Menu item identifiers
const MENU_INSERT_FLOPPY_A: &str = "insert_floppy_a";
const MENU_EJECT_FLOPPY_A: &str = "eject_floppy_a";
const MENU_INSERT_FLOPPY_B: &str = "insert_floppy_b";
const MENU_EJECT_FLOPPY_B: &str = "eject_floppy_b";

/// Application menu structure with references to menu items
pub struct AppMenu {
    #[allow(dead_code)]
    pub menu: Menu,
    insert_floppy_a: MenuItem,
    eject_floppy_a: MenuItem,
    insert_floppy_b: MenuItem,
    eject_floppy_b: MenuItem,
}

impl AppMenu {
    /// Update menu item states based on disk presence
    pub fn update_menu_states(&self, floppy_a_present: bool, floppy_b_present: bool) {
        // Eject items are only enabled when disk is present
        self.eject_floppy_a.set_enabled(floppy_a_present);
        self.eject_floppy_b.set_enabled(floppy_b_present);

        // Insert items are always enabled (allows disk swapping)
        self.insert_floppy_a.set_enabled(true);
        self.insert_floppy_b.set_enabled(true);
    }

    /// Get the menu item ID from a menu event
    pub fn get_menu_action(&self, event: &MenuEvent) -> Option<MenuAction> {
        let id = event.id();
        if id == self.insert_floppy_a.id() {
            Some(MenuAction::InsertFloppyA)
        } else if id == self.eject_floppy_a.id() {
            Some(MenuAction::EjectFloppyA)
        } else if id == self.insert_floppy_b.id() {
            Some(MenuAction::InsertFloppyB)
        } else if id == self.eject_floppy_b.id() {
            Some(MenuAction::EjectFloppyB)
        } else {
            None
        }
    }
}

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

/// Create the application menu structure
pub fn create_menu() -> Result<AppMenu> {
    let menu = Menu::new();

    // Create "Floppy" submenu
    let floppy_menu = Submenu::new("Floppy", true);

    // Create "Floppy A:" submenu
    let floppy_a_menu = Submenu::new("Floppy A:", true);

    let insert_floppy_a = MenuItem::with_id(
        MENU_INSERT_FLOPPY_A,
        "Insert Disk...",
        true,
        Some(Accelerator::new(
            Some(muda::accelerator::Modifiers::CONTROL | muda::accelerator::Modifiers::SHIFT),
            muda::accelerator::Code::KeyA,
        )),
    );

    let eject_floppy_a = MenuItem::with_id(
        MENU_EJECT_FLOPPY_A,
        "Eject Disk",
        false, // Initially disabled
        Some(Accelerator::new(
            Some(muda::accelerator::Modifiers::CONTROL | muda::accelerator::Modifiers::ALT),
            muda::accelerator::Code::KeyA,
        )),
    );

    floppy_a_menu
        .append(&insert_floppy_a)
        .context("Failed to append insert floppy A menu item")?;
    floppy_a_menu
        .append(&eject_floppy_a)
        .context("Failed to append eject floppy A menu item")?;

    // Create "Floppy B:" submenu
    let floppy_b_menu = Submenu::new("Floppy B:", true);

    let insert_floppy_b = MenuItem::with_id(
        MENU_INSERT_FLOPPY_B,
        "Insert Disk...",
        true,
        Some(Accelerator::new(
            Some(muda::accelerator::Modifiers::CONTROL | muda::accelerator::Modifiers::SHIFT),
            muda::accelerator::Code::KeyB,
        )),
    );

    let eject_floppy_b = MenuItem::with_id(
        MENU_EJECT_FLOPPY_B,
        "Eject Disk",
        false, // Initially disabled
        Some(Accelerator::new(
            Some(muda::accelerator::Modifiers::CONTROL | muda::accelerator::Modifiers::ALT),
            muda::accelerator::Code::KeyB,
        )),
    );

    floppy_b_menu
        .append(&insert_floppy_b)
        .context("Failed to append insert floppy B menu item")?;
    floppy_b_menu
        .append(&eject_floppy_b)
        .context("Failed to append eject floppy B menu item")?;

    // Add submenus to main floppy menu
    floppy_menu
        .append(&floppy_a_menu)
        .context("Failed to append floppy A submenu")?;
    floppy_menu
        .append(&floppy_b_menu)
        .context("Failed to append floppy B submenu")?;

    // Add floppy menu to main menu
    menu.append(&floppy_menu)
        .context("Failed to append floppy menu")?;

    Ok(AppMenu {
        menu,
        insert_floppy_a,
        eject_floppy_a,
        insert_floppy_b,
        eject_floppy_b,
    })
}
