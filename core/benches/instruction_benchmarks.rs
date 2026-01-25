use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use emu86_core::Computer;

/// Assemble a simple program for benchmarking
/// Returns assembled machine code
fn assemble_simple_loop() -> Vec<u8> {
    // Simple loop that counts down CX from 10000 to 0
    // MOV CX, 10000  ; B9 10 27
    // LOOP_START:
    // DEC CX         ; 49
    // JNZ LOOP_START ; 75 FD
    // HLT            ; F4
    vec![
        0xB9, 0x10, 0x27,  // MOV CX, 10000
        0x49,              // DEC CX (loop start)
        0x75, 0xFD,        // JNZ -3 (jump to DEC CX)
        0xF4,              // HLT
    ]
}

fn assemble_arithmetic_ops() -> Vec<u8> {
    // Test various arithmetic operations in a loop
    // MOV CX, 1000   ; B9 E8 03
    // MOV AX, 0      ; B8 00 00
    // LOOP_START:
    // INC AX         ; 40
    // ADD AX, 2      ; 05 02 00
    // SUB AX, 1      ; 2D 01 00
    // DEC CX         ; 49
    // JNZ LOOP_START ; 75 F7
    // HLT            ; F4
    vec![
        0xB9, 0xE8, 0x03,  // MOV CX, 1000
        0xB8, 0x00, 0x00,  // MOV AX, 0
        0x40,              // INC AX (loop start)
        0x05, 0x02, 0x00,  // ADD AX, 2
        0x2D, 0x01, 0x00,  // SUB AX, 1
        0x49,              // DEC CX
        0x75, 0xF7,        // JNZ -9 (jump to INC AX)
        0xF4,              // HLT
    ]
}

fn assemble_memory_ops() -> Vec<u8> {
    // Test memory operations
    // MOV CX, 1000      ; B9 E8 03
    // MOV BX, 1000h     ; BB 00 10
    // LOOP_START:
    // MOV [BX], AX      ; 89 07
    // MOV AX, [BX]      ; 8B 07
    // INC BX            ; 43
    // INC BX            ; 43
    // DEC CX            ; 49
    // JNZ LOOP_START    ; 75 F6
    // HLT               ; F4
    vec![
        0xB9, 0xE8, 0x03,  // MOV CX, 1000
        0xBB, 0x00, 0x10,  // MOV BX, 0x1000
        0x89, 0x07,        // MOV [BX], AX (loop start)
        0x8B, 0x07,        // MOV AX, [BX]
        0x43,              // INC BX
        0x43,              // INC BX
        0x49,              // DEC CX
        0x75, 0xF6,        // JNZ -10 (jump to MOV [BX], AX)
        0xF4,              // HLT
    ]
}

fn assemble_register_only() -> Vec<u8> {
    // Pure register operations (fastest)
    // MOV CX, 10000     ; B9 10 27
    // MOV AX, 0         ; B8 00 00
    // LOOP_START:
    // INC AX            ; 40
    // INC BX            ; 43
    // INC DX            ; 42
    // DEC CX            ; 49
    // JNZ LOOP_START    ; 75 FA
    // HLT               ; F4
    vec![
        0xB9, 0x10, 0x27,  // MOV CX, 10000
        0xB8, 0x00, 0x00,  // MOV AX, 0
        0x40,              // INC AX (loop start)
        0x43,              // INC BX
        0x42,              // INC DX
        0x49,              // DEC CX
        0x75, 0xFA,        // JNZ -6 (jump to INC AX)
        0xF4,              // HLT
    ]
}

fn assemble_nop_loop() -> Vec<u8> {
    // Minimal loop with NOPs to test pure overhead
    // MOV CX, 10000     ; B9 10 27
    // LOOP_START:
    // NOP               ; 90
    // DEC CX            ; 49
    // JNZ LOOP_START    ; 75 FC
    // HLT               ; F4
    vec![
        0xB9, 0x10, 0x27,  // MOV CX, 10000
        0x90,              // NOP (loop start)
        0x49,              // DEC CX
        0x75, 0xFC,        // JNZ -4 (jump to NOP)
        0xF4,              // HLT
    ]
}

/// Count the approximate number of instructions in a program
fn count_instructions(program: &Vec<u8>) -> u64 {
    match program {
        p if p == &assemble_simple_loop() => 10000 * 2 + 2,  // loop_iterations * (DEC + JNZ) + MOV + HLT
        p if p == &assemble_arithmetic_ops() => 1000 * 5 + 3, // loop_iterations * (INC + ADD + SUB + DEC + JNZ) + setup + HLT
        p if p == &assemble_memory_ops() => 1000 * 6 + 3,     // loop_iterations * (MOV mem + MOV from mem + 2*INC + DEC + JNZ) + setup + HLT
        p if p == &assemble_register_only() => 10000 * 5 + 3, // loop_iterations * (3*INC + DEC + JNZ) + setup + HLT
        p if p == &assemble_nop_loop() => 10000 * 3 + 2,      // loop_iterations * (NOP + DEC + JNZ) + MOV + HLT
        _ => 0,
    }
}

/// Estimate CPU cycles based on instruction count and types
/// Real 8086 cycles vary by instruction, but we use rough averages
fn estimate_cycles(program: &Vec<u8>) -> u64 {
    match program {
        p if p == &assemble_simple_loop() => {
            // MOV CX, imm16: 4 cycles
            // DEC reg: 2 cycles
            // JNZ taken: 16 cycles, not taken: 4 cycles
            // HLT: 2 cycles
            4 + (10000 * (2 + 16)) + 4 + 2
        }
        p if p == &assemble_arithmetic_ops() => {
            // MOV: 4 cycles each
            // INC: 2 cycles
            // ADD/SUB imm: 4 cycles
            // DEC: 2 cycles
            // JNZ taken: 16 cycles
            // HLT: 2 cycles
            4 + 4 + (1000 * (2 + 4 + 4 + 2 + 16)) + 4 + 2
        }
        p if p == &assemble_memory_ops() => {
            // MOV r/m to reg and reg to r/m: ~13-14 cycles (using [BX])
            // INC reg: 2 cycles
            // DEC: 2 cycles
            // JNZ taken: 16 cycles
            4 + 4 + (1000 * (14 + 14 + 2 + 2 + 2 + 16)) + 4 + 2
        }
        p if p == &assemble_register_only() => {
            // MOV: 4 cycles each
            // INC: 2 cycles each
            // DEC: 2 cycles
            // JNZ taken: 16 cycles
            4 + 4 + (10000 * (2 + 2 + 2 + 2 + 16)) + 4 + 2
        }
        p if p == &assemble_nop_loop() => {
            // MOV: 4 cycles
            // NOP: 3 cycles
            // DEC: 2 cycles
            // JNZ taken: 16 cycles
            4 + (10000 * (3 + 2 + 16)) + 4 + 2
        }
        _ => 0,
    }
}

fn run_program_benchmark(program: Vec<u8>) {
    let mut computer = Computer::new();
    computer.load_program(&program, 0x1000, 0x0000).unwrap();
    computer.run();
}

fn benchmark_programs(c: &mut Criterion) {
    let programs = vec![
        ("simple_loop", assemble_simple_loop()),
        ("arithmetic_ops", assemble_arithmetic_ops()),
        ("memory_ops", assemble_memory_ops()),
        ("register_only", assemble_register_only()),
        ("nop_loop", assemble_nop_loop()),
    ];

    let mut group = c.benchmark_group("instruction_execution");

    // Store results for final analysis
    let mut results: Vec<(&str, u64, u64, f64)> = Vec::new();

    for (name, program) in programs.iter() {
        let instruction_count = count_instructions(program);
        let estimated_cycles = estimate_cycles(program);

        let id = BenchmarkId::from_parameter(name);

        group.bench_with_input(
            id,
            program,
            |b, prog| {
                b.iter(|| {
                    run_program_benchmark(black_box(prog.clone()))
                });
            }
        );

        // After the benchmark, we'll print estimated performance in the summary
        println!("\n{} stats:", name);
        println!("  Instructions: {}", instruction_count);
        println!("  Est. 8086 cycles: {}", estimated_cycles);

        // Store for later analysis - we'll manually time a run
        let start = std::time::Instant::now();
        run_program_benchmark(program.clone());
        let elapsed = start.elapsed().as_secs_f64();

        results.push((name, instruction_count, estimated_cycles, elapsed));
    }

    group.finish();

    // Print performance summary
    println!("\n{}", "=".repeat(80));
    println!("EMU86 PERFORMANCE ANALYSIS");
    println!("{}", "=".repeat(80));
    println!();

    let mut total_weighted_mhz = 0.0;
    let mut total_weight = 0u64;

    for (name, instructions, cycles, time_s) in results.iter() {
        let instructions_per_sec = *instructions as f64 / time_s;
        let cycles_per_sec = *cycles as f64 / time_s;
        let mhz = cycles_per_sec / 1_000_000.0;
        let avg_cycles_per_inst = *cycles as f64 / *instructions as f64;

        println!("{}:", name);
        println!("  Time:                    {:.2} µs", time_s * 1_000_000.0);
        println!("  Instructions:            {}", instructions);
        println!("  Est. 8086 cycles:        {}", cycles);
        println!("  Avg cycles/instruction:  {:.1}", avg_cycles_per_inst);
        println!("  Instructions/sec:        {:.0}", instructions_per_sec);
        println!("  Cycles/sec:              {:.0}", cycles_per_sec);
        println!("  Effective MHz:           {:.2}", mhz);
        println!();

        total_weighted_mhz += mhz * (*instructions as f64);
        total_weight += instructions;
    }

    let average_mhz = total_weighted_mhz / total_weight as f64;

    println!("{}", "=".repeat(80));
    println!("WEIGHTED AVERAGE EFFECTIVE SPEED: {:.2} MHz", average_mhz);
    println!("{}", "=".repeat(80));
    println!();

    // Compare to real 8086
    let real_8086_mhz = 4.77;  // Original IBM PC
    let speedup = average_mhz / real_8086_mhz;

    println!("Real 8086 (IBM PC):           {} MHz", real_8086_mhz);
    println!("Emulator effective speed:     {:.2} MHz", average_mhz);
    println!("Speedup vs real hardware:     {:.1}x", speedup);
    println!();

    println!("NOTES:");
    println!("- These estimates are based on 8086 cycle counts for instructions");
    println!("- Real 8086 had variable timing based on memory access patterns");
    println!("- Emulator overhead includes instruction decoding and Rust function calls");
    println!("- The emulator is running interpreted code, not JIT compiled");
    println!("- Performance will vary based on CPU, compiler optimizations, etc.");
    println!();
}

fn single_instruction_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_instruction");

    // Benchmark individual instruction types
    let instructions = vec![
        ("nop", vec![0x90, 0xF4]),           // NOP, HLT
        ("inc_ax", vec![0x40, 0xF4]),        // INC AX, HLT
        ("mov_imm", vec![0xB8, 0x34, 0x12, 0xF4]), // MOV AX, 0x1234, HLT
    ];

    for (name, program) in instructions.iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            program,
            |b, prog| {
                b.iter(|| {
                    let mut computer = Computer::new();
                    computer.load_program(black_box(prog), 0x1000, 0x0000).unwrap();
                    computer.run();
                });
            }
        );
    }

    group.finish();
}

criterion_group!(benches, benchmark_programs, single_instruction_overhead);
criterion_main!(benches);
