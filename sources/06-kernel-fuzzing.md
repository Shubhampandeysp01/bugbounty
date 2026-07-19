# Fuzzing

## Fuzzing Fundamentals

### Coverage-Guided Fuzzing
- Compile target with instrumentation (AFL's edge coverage, Clang SanitizerCoverage)
- Use coverage feedback to guide input mutation toward new code paths
- Seed corpus matters — distill from real-world data and re-minimize

### Sanitizers
- **ASan**: Heap/stack buffer overflow, use-after-free
- **MSan**: Uninitialized memory reads
- **UBSan**: Integer overflow, undefined behavior
- **TSan**: Data races between threads
- **KASAN**: Kernel Address Sanitizer
- **KCSAN**: Kernel Concurrency Sanitizer
- **KMSAN**: Kernel Memory Sanitizer

## Key Fuzzers

### AFL++ (American Fuzzy Lop Plus Plus)
- Coverage-guided binary fuzzer
- **Persistent mode**: In-process fuzzing without fork overhead
- **QEMU mode**: Fuzz binaries without source instrumentation
- **Frida mode**: Binary-only fuzzing with Frida instrumentation
- **LLVM mode**: Best performance with source-level instrumentation
- **Dictionaries**: Magic bytes for protocol/format awareness

### libFuzzer
- In-process coverage-guided fuzzer for library code
- Integrated with Clang: `-fsanitize=fuzzer`
- Write `LLVMFuzzerTestOneInput()` harness
- **FuzzerIntrospector**: Data-flow tracing and coverage analysis

### Honggfuzz
- Hardware performance counter feedback (branch misses, instructions retired)
- Multi-threaded, persistent mode
- Fast corpus management

### syzkaller (Kernel Fuzzer)
Google's coverage-guided kernel fuzzer (powers syzbot)

**Architecture:**
- **Manager**: Runs on host, spawns VMs, manages corpora
- **Fuzzer process**: Inside VM, generates and runs syscall programs
- **syzlang**: Syscall description language in `sys/linux/*.txt`
- **KCOV**: Kernel code coverage instrumentation
- **Web UI**: Manager exposes dashboard with crash stats

**Key Features:**
- Discovers ~4000 Linux kernel bugs and counting
- Found bugs: github.com/google/syzkaller/wiki/Found-Bugs
- Supports VMs (QEMU, GCE) and physical devices
- Auto-generates C reproducers from crashes

**Research based on syzkaller:**
- "SyzDirect: Directed Greybox Fuzzing for Linux Kernel"
- "Unlocking Low Frequency Syscalls with Dependency-Based RAG"
- "KIT: Testing OS-Level Virtualization for Functional Interference Bugs"
- "SyzGen: Automated Generation of Syscall Specification of Closed-Source macOS Drivers"
- "Snowboard: Finding Kernel Concurrency Bugs through Systematic Inter-thread Communication Analysis"
- "HFL: Hybrid Fuzzing on the Linux Kernel" - NDSS
- "Towards LLM Guided Kernel Direct Fuzzing" - SyzAgent, arXiv 2025

## Kernel Fuzzing

### Linux
- **syzkaller**: Primary tool (Google)
- **kAFL**: Hardware-assisted feedback using Intel PT (Processor Trace)
- **kernel-fuzzing**: AFL + KCOV bridge
- **Trinity**: Predecessor to syzkaller, still useful for quick testing

### Windows
- **IOCTL Fuzzing**: Use `DeviceIoControl` with random buffers
- **WinDbg**: Kernel debugging and crash analysis
- **OneFuzz**: Microsoft's fuzzing framework

### macOS/iOS
- **SyzGen**: Auto-generate macOS driver syscall descriptions
- **Drill the Apple Core**: Black Hat EU research

## Fuzzing Strategies
- **Seed corpus**: Start with valid real-world inputs, distill to minimal set
- **Dictionary support**: Protocol-specific tokens and keywords
- **Custom mutators**: Domain-specific mutation operators
- **Coverage tracking**: `afl-showmap`, `llvm-cov`, `gcov`
- **Corpus minimization**: `afl-cmin` + `afl-tmin` for individual cases
- **Triage pipeline**: Deduplicate crashes and prioritize by severity

## Fuzzing Papers Repository
- github.com/wcventure/FuzzingPaper (comprehensive fuzzing paper collection)
- "Fuzzing: A Survey" - Chen et al., 2018 (comprehensive technique review)

## Additional Fuzzers
- **Jazzer**: JVM fuzzer (Java/Kotlin) with libFuzzer engine
- **Atheris**: Python coverage-guided fuzzer
- **OneFuzz**: Microsoft's self-hosted fuzzing orchestration framework
- **Peach**: Grammar-based fuzzing for protocols and file formats
- **BSOD**: Windows kernel fuzzing with Bochs emulation
