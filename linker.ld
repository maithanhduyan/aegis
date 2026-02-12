ENTRY(_start)

SECTIONS
{
    . = 0x40080000;

    /* === Code (RX) === */
    __text_start = .;
    .text : {
        KEEP(*(.text._start))
        *(.text*)
    }
    __text_end = .;

    /* === Read-Only Data (RO-NX) === */
    . = ALIGN(4096);
    __rodata_start = .;
    .rodata : { *(.rodata*) }
    __rodata_end = .;

    /* === Mutable Data (RW-NX) === */
    . = ALIGN(4096);
    __data_start = .;
    .data : { *(.data*) }
    __data_end = .;

    /* === BSS (RW-NX, zeroed) === */
    .bss : {
        . = ALIGN(16);
        __bss_start = .;
        *(.bss*);
        . = ALIGN(16);
        __bss_end = .;
    }

    /* === Page Tables ((4*NUM_TASKS+4) × 4096, 4KB-aligned) === */
    /* Layout: [L2Device×N | L1×N | L2Ram×N | L3×N | kernel×4]  (N=NUM_TASKS) */
    /* SYNC: size must equal mmu::NUM_PAGE_TABLE_PAGES × 4096  (NUM_TASKS=8 → 36) */
    . = ALIGN(4096);
    __page_tables_start = .;
    .page_tables (NOLOAD) : {
        . += 36 * 4096;
    }
    __page_tables_end = .;

    /* === Kernel Stacks per task (NUM_TASKS × 4KB, 4KB-aligned) === */
    /* Used as SP_EL1 when handling exceptions from EL0 tasks */
    /* SYNC: count must equal sched::NUM_TASKS  (NUM_TASKS=8 → 8) */
    . = ALIGN(4096);
    __task_stacks_start = .;
    .task_stacks (NOLOAD) : {
        . += 8 * 4096;
    }
    __task_stacks_end = .;

    /* === User Stacks per task (NUM_TASKS × 4KB, 4KB-aligned) === */
    /* Used as SP_EL0 when tasks run in user mode (EL0) */
    /* SYNC: count must equal sched::NUM_TASKS  (NUM_TASKS=8 → 8) */
    . = ALIGN(4096);
    __user_stacks_start = .;
    .user_stacks (NOLOAD) : {
        . += 8 * 4096;
    }
    __user_stacks_end = .;

    /* === Grant Pages (2 × 4KB = 8 KiB, 4KB-aligned) === */
    /* Shared memory regions for inter-task data sharing (Phase J) */
    . = ALIGN(4096);
    __grant_pages_start = .;
    .grant_pages (NOLOAD) : {
        . += 2 * 4096;
    }
    __grant_pages_end = .;

    /* === ELF Load Region (3 × 4KB = 12 KiB, fixed address for user binary linkage) === */
    /* Writable pages for loading ELF binary segments (Phase L4/L5) */
    /* Fixed at 0x4010_0000 so user binaries can link at a known address */
    . = 0x40100000;
    __elf_load_start = .;
    .elf_load (NOLOAD) : {
        . += 3 * 4096;
    }
    __elf_load_end = .;

    /* === Stack Guard Page (4KB invalid — catches stack overflow) === */
    . = ALIGN(4096);
    __stack_guard = .;
    . += 4096;

    /* === Stack (16 KB) === */
    . = ALIGN(16);
    __stack_start = .;
    . += 0x4000;
    __stack_end = .;

    /* === End of kernel image === */
    . = ALIGN(4096);
    __kernel_end = .;

    /DISCARD/ : { *(.comment*) *(.eh_frame*) *(.gcc_except_table*) }
}
