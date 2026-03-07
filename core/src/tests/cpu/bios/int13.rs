use crate::cpu::CpuType;
use crate::disk::{BackedDisk, Disk, DiskGeometry, DriveNumber, MemBackend};
use crate::tests::run_test;

#[test_log::test]
pub(crate) fn floppy_read() {
    run_test(
        "cpu/bios/int13/floppy_read",
        make_computer!(cpu_type: CpuType::I80286),
        |computer, _video_buffer| {
            let backend = MemBackend::zeroed(DiskGeometry::FLOPPY_1440K.total_size);
            let disk = BackedDisk::new(backend).unwrap();
            computer.set_floppy_disk(DriveNumber::floppy_a(), Some(Box::new(disk)));
            computer.run()
        },
    );
}

#[test_log::test]
pub(crate) fn floppy_write() {
    run_test(
        "cpu/bios/int13/floppy_write",
        make_computer!(cpu_type: CpuType::I80286),
        |computer, _video_buffer| {
            let backend = MemBackend::zeroed(DiskGeometry::FLOPPY_1440K.total_size);
            let disk = BackedDisk::new(backend).unwrap();
            computer.set_floppy_disk(DriveNumber::floppy_a(), Some(Box::new(disk)));
            computer.run();
            let disk = computer
                .set_floppy_disk(DriveNumber::floppy_a(), None)
                .unwrap();
            let data = disk.read_sectors(0, 0, 1, 1).expect("read sector failed");
            assert_eq!(data.len(), 512);
            assert!(data.iter().all(|&b| b == 0xA5), "sector data mismatch");
        },
    );
}

#[test_log::test]
pub(crate) fn hard_disk_read() {
    run_test(
        "cpu/bios/int13/hard_disk_read",
        make_computer!(
            cpu_type: CpuType::I80286,
            hard_disks: {
                let backend = MemBackend::zeroed(10 * 1024 * 1024);
                let disk = BackedDisk::new(backend).unwrap();
                vec![Box::new(disk) as Box<dyn Disk>]
            }
        ),
        |computer, _video_buffer| computer.run(),
    );
}

#[test_log::test]
pub(crate) fn hard_disk_write() {
    run_test(
        "cpu/bios/int13/hard_disk_write",
        make_computer!(
            cpu_type: CpuType::I80286,
            hard_disks: {
                let backend = MemBackend::zeroed(10 * 1024 * 1024);
                let disk = BackedDisk::new(backend).unwrap();
                vec![Box::new(disk) as Box<dyn Disk>]
            }
        ),
        |computer, _video_buffer| {
            computer.run();
            let sector = computer
                .read_hard_disk_sectors(DriveNumber::hard_drive_c(), 0, 0, 1, 1)
                .expect("read sector failed");
            assert_eq!(sector.len(), 512);
            assert!(sector.iter().all(|&b| b == 0xA5), "sector data mismatch");
        },
    );
}

#[test_log::test]
pub(crate) fn floppy_write_protected() {
    run_test(
        "cpu/bios/int13/floppy_write_protected",
        make_computer!(cpu_type: CpuType::I80286),
        |computer, _video_buffer| {
            let backend = MemBackend::zeroed(DiskGeometry::FLOPPY_1440K.total_size);
            let mut disk = BackedDisk::new(backend).unwrap();
            disk.set_read_only(true);
            computer.set_floppy_disk(DriveNumber::floppy_a(), Some(Box::new(disk)));
            computer.run()
        },
    );
}

#[test_log::test]
pub(crate) fn floppy_verify() {
    run_test(
        "cpu/bios/int13/floppy_verify",
        make_computer!(cpu_type: CpuType::I80286),
        |computer, _video_buffer| {
            let backend = MemBackend::zeroed(DiskGeometry::FLOPPY_1440K.total_size);
            let disk = BackedDisk::new(backend).unwrap();
            computer.set_floppy_disk(DriveNumber::floppy_a(), Some(Box::new(disk)));
            computer.run()
        },
    );
}

#[test_log::test]
pub(crate) fn hard_disk_verify() {
    run_test(
        "cpu/bios/int13/hard_disk_verify",
        make_computer!(
            cpu_type: CpuType::I80286,
            hard_disks: {
                let backend = MemBackend::zeroed(10 * 1024 * 1024);
                let disk = BackedDisk::new(backend).unwrap();
                vec![Box::new(disk) as Box<dyn Disk>]
            }
        ),
        |computer, _video_buffer| computer.run(),
    );
}
