use crate::{
    cpu::CpuType,
    disk::{BackedDisk, DiskGeometry, DriveNumber, MemBackend},
    tests::run_test,
};

/// DMA2 channel 5 counter-advance test (286 system).
/// Programs DMA2 channel 5 with a software-asserted DREQ and verifies the
/// current-count register advances — proving DMA2 is alive on a 286 AT.
/// Requires BIOS DMA2 initialisation and CASCADE mode handling in tick().
#[test_log::test]
pub(crate) fn counter_advances_286() {
    run_test(
        "devices/dma/counter_advances_286",
        make_computer!(cpu_type: CpuType::I80286),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

#[test_log::test]
pub(crate) fn counter_advances() {
    run_test(
        "devices/dma/counter_advances",
        make_computer!(),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

/// DMA verify mode: channel 1 programmed with transfer type 00 (verify).
/// The DMA must advance its counters but must NOT write any bytes to memory.
/// Buffer is pre-filled with 0xCC; if any byte changes the test fails.
#[test_log::test]
pub(crate) fn dma_verify_mode() {
    run_test(
        "devices/dma/dma_verify_mode",
        make_computer!(),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

/// DMA auto-init: channel 1 programmed with count=0 and auto-init enabled.
/// After many TC events the channel must remain unmasked (auto-init reloaded
/// the base registers).  A non-auto-init channel would be masked on TC.
#[test_log::test]
pub(crate) fn dma_auto_init() {
    run_test(
        "devices/dma/dma_auto_init",
        make_computer!(),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

/// DMA floppy read: programs DMA channel 2 and FDC directly (no BIOS),
/// issues READ DATA without the NDM bit (DMA mode), then verifies the
/// sector data landed in the target buffer.
///
/// This test is RED until Phase 4 (FDC DMA transfers) is implemented.
#[test_log::test]
pub(crate) fn floppy_read_dma() {
    run_test(
        "devices/dma/floppy_read_dma",
        make_computer!(cpu_type: CpuType::I80286),
        |computer, _video_buffer| {
            let backend = MemBackend::zeroed(DiskGeometry::FLOPPY_1440K.total_size);
            let disk = BackedDisk::new(backend).unwrap();
            computer.set_floppy_disk(DriveNumber::floppy_a(), Some(Box::new(disk)));
            computer.run();
        },
    );
}
