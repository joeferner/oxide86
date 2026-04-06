use crate::cpu::CpuType;
use crate::tests::{create_computer, run_test};

mod bios;

#[test_log::test]
pub(crate) fn op8086() {
    run_test(
        "cpu/op8086",
        create_computer(),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

#[test_log::test]
pub(crate) fn op8087_load_store() {
    run_test(
        "cpu/op8087_load_store",
        make_computer!(math_coprocessor: true),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

#[test_log::test]
pub(crate) fn op8087_control() {
    run_test(
        "cpu/op8087_control",
        make_computer!(math_coprocessor: true),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

#[test_log::test]
pub(crate) fn op8087_arith() {
    run_test(
        "cpu/op8087_arith",
        make_computer!(math_coprocessor: true),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

#[test_log::test]
pub(crate) fn op8087_int64() {
    run_test(
        "cpu/op8087_int64",
        make_computer!(math_coprocessor: true),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

#[test_log::test]
#[ignore] // TODO checkit trig test
pub(crate) fn op8087_trig() {
    run_test(
        "cpu/op8087_trig",
        make_computer!(math_coprocessor: true),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

#[test_log::test]
pub(crate) fn op8087_compare() {
    run_test(
        "cpu/op8087_compare",
        make_computer!(math_coprocessor: true),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

#[test_log::test]
pub(crate) fn op286() {
    run_test(
        "cpu/op286",
        make_computer!(cpu_type: CpuType::I80286),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

#[test_log::test]
pub(crate) fn irq_chain() {
    run_test(
        "cpu/irq_chain",
        create_computer(),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

/// Run the CPU detection program as an 8086.
/// Expected: SP push quirk detected, bits 12-15 confirmed 0xF000 → exit 0x00.
#[test_log::test]
pub(crate) fn cpu_detect_8086() {
    let program_data = super::load_program_data("cpu/cpu_detect");
    let (mut computer, _video_buffer) = make_computer!();
    computer
        .load_program(&program_data, super::TEST_SEGMENT, super::TEST_OFFSET)
        .unwrap();
    computer.run();
    assert_eq!(
        Some(0x00),
        computer.get_exit_code(),
        "expected 8086 detection (exit 0x00)"
    );
}

/// Verify 8086 shift-by-CL behaviour: counts ≥ 16 (word) or ≥ 8 (byte) must
/// shift all bits out and produce 0, not wrap mod 16 as on 286+/modern x86.
/// This is the root cause of oxide86's EGALATCH.CK1 decoding bug.
#[test_log::test]
pub(crate) fn shift_cl() {
    run_test(
        "cpu/shift_cl",
        create_computer(),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

/// Run with no math coprocessor configured (default).
/// Both INT 11h and FNINIT/FNSTSW must agree that no 8087 is present → exit 0x00.
#[test_log::test]
pub(crate) fn fpu_not_present() {
    let program_data = super::load_program_data("cpu/fpu_not_present");
    let (mut computer, _video_buffer) = make_computer!();
    computer
        .load_program(&program_data, super::TEST_SEGMENT, super::TEST_OFFSET)
        .unwrap();
    computer.run();
    assert_eq!(
        Some(0x00),
        computer.get_exit_code(),
        "expected no coprocessor detected (exit 0x00)"
    );
}

/// Run with a math coprocessor configured.
/// Both INT 11h and FNINIT/FNSTSW must agree that an 8087 is present → exit 0x00.
#[test_log::test]
pub(crate) fn fpu_present() {
    let program_data = super::load_program_data("cpu/fpu_present");
    let (mut computer, _video_buffer) = make_computer!(math_coprocessor: true);
    computer
        .load_program(&program_data, super::TEST_SEGMENT, super::TEST_OFFSET)
        .unwrap();
    computer.run();
    assert_eq!(
        Some(0x00),
        computer.get_exit_code(),
        "expected coprocessor detected (exit 0x00)"
    );
}

/// 286 Protected Mode Step 1: SMSW/LMSW with real CR0 state tracking.
/// Tests that SMSW returns the actual MSW, LMSW can set PE/MP/EM/TS bits,
/// and LMSW cannot clear PE once set (286 behavior).
#[test_log::test]
pub(crate) fn pm_step1_msw() {
    run_test(
        "cpu/pm_step1_msw",
        make_computer!(cpu_type: CpuType::I80286),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

/// 286 Protected Mode Step 3: Segment loading in protected mode.
/// Tests that MOV to segment registers uses GDT descriptor bases for
/// address resolution, not (segment << 4).
#[test_log::test]
pub(crate) fn pm_step3_segments() {
    run_test(
        "cpu/pm_step3_segments",
        make_computer!(cpu_type: CpuType::I80286),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

/// 286 Protected Mode Step 10: Real mode → PM transition and reset path.
/// Tests PM entry, keyboard controller reset (0xFE), and PE=0 after reset.
/// Uses CMOS shutdown byte (register 0x0F) to detect post-reset run.
#[test_log::test]
pub(crate) fn pm_step10_mode_switch() {
    run_test(
        "cpu/pm_step10_mode_switch",
        make_computer!(cpu_type: CpuType::I80286),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

/// 286 Protected Mode Step 9: Privilege level transitions (ring 0 ↔ ring 3).
/// Tests IRET to ring 3, CPL verification, DPL-checked data access,
/// call gate from ring 3 to ring 0 with stack switch, and return.
#[test_log::test]
pub(crate) fn pm_step9_rings() {
    run_test(
        "cpu/pm_step9_rings",
        make_computer!(cpu_type: CpuType::I80286),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

/// 286 Protected Mode Step 8: Far CALL/JMP through call gates.
/// Tests call gate dispatch, far RET, return values, far JMP through gate,
/// and direct far CALL to code segments.
#[test_log::test]
pub(crate) fn pm_step8_call_gate() {
    run_test(
        "cpu/pm_step8_call_gate",
        make_computer!(cpu_type: CpuType::I80286),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

/// 286 Protected Mode Step 7: LLDT/SLDT/LTR/STR.
/// Tests LDT and Task Register load/store, and loading a segment from the LDT.
#[test_log::test]
pub(crate) fn pm_step7_lldt() {
    run_test(
        "cpu/pm_step7_lldt",
        make_computer!(cpu_type: CpuType::I80286),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

/// 286 Protected Mode Step 6: Exception handling (#GP, #NP).
/// Tests that #GP fires on limit violations and bad selectors,
/// #NP fires on loading a not-present segment, and error codes
/// are pushed onto the stack.
#[test_log::test]
pub(crate) fn pm_step6_exceptions() {
    run_test(
        "cpu/pm_step6_exceptions",
        make_computer!(cpu_type: CpuType::I80286),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

/// 286 Protected Mode Step 5: IDT-based interrupt dispatch.
/// Tests that INT in protected mode dispatches through the IDT,
/// interrupt gates clear IF, trap gates preserve IF, and IRET works.
#[test_log::test]
pub(crate) fn pm_step5_idt() {
    run_test(
        "cpu/pm_step5_idt",
        make_computer!(cpu_type: CpuType::I80286),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

/// 286 Protected Mode Step 4: Segment limit checking.
/// Tests that accesses within a segment's limit succeed, and accesses
/// beyond the limit are blocked (#GP).
#[test_log::test]
pub(crate) fn pm_step4_limits() {
    run_test(
        "cpu/pm_step4_limits",
        make_computer!(cpu_type: CpuType::I80286),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

/// 286 Protected Mode Step 2: LGDT/LIDT/SGDT/SIDT.
/// Tests that descriptor table registers can be loaded and stored correctly.
#[test_log::test]
pub(crate) fn pm_step2_gdt_idt() {
    run_test(
        "cpu/pm_step2_gdt_idt",
        make_computer!(cpu_type: CpuType::I80286),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

/// Run the same program as a 286.
/// Expected: no SP quirk, IOPL not settable, bits 12-15 confirmed 0x0000 → exit 0x01.
#[test_log::test]
pub(crate) fn cpu_detect_286() {
    let program_data = super::load_program_data("cpu/cpu_detect");
    let (mut computer, _video_buffer) = make_computer!(cpu_type: CpuType::I80286);
    computer
        .load_program(&program_data, super::TEST_SEGMENT, super::TEST_OFFSET)
        .unwrap();
    computer.run();
    assert_eq!(
        Some(0x01),
        computer.get_exit_code(),
        "expected 286 detection (exit 0x01)"
    );
}
