use crate::{
    cpu::CpuType,
    disk::{BackedDisk, DiskGeometry, DriveNumber, MemBackend},
    tests::run_test,
};

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
