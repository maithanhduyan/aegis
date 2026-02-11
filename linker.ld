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

    /* === Page Tables (13 × 4096 = 52 KiB, 4KB-aligned) === */
    /* Layout: [0]=L2_device(shared), [1..3]=L1 per task, [4..6]=L2_ram per task,
       [7..9]=L3 per task, [10]=L1 kernel boot, [11]=L2_ram kernel, [12]=L3 kernel */
    . = ALIGN(4096);
    __page_tables_start = .;
    .page_tables (NOLOAD) : {
        . += 13 * 4096;
    }
    __page_tables_end = .;

    /* === Kernel Stacks per task (3 × 4KB = 12 KiB, 4KB-aligned) === */
    /* Used as SP_EL1 when handling exceptions from EL0 tasks */
    . = ALIGN(4096);
    __task_stacks_start = .;
    .task_stacks (NOLOAD) : {
        . += 3 * 4096;
    }
    __task_stacks_end = .;

    /* === User Stacks per task (3 × 4KB = 12 KiB, 4KB-aligned) === */
    /* Used as SP_EL0 when tasks run in user mode (EL0) */
    . = ALIGN(4096);
    __user_stacks_start = .;
    .user_stacks (NOLOAD) : {
        . += 3 * 4096;
    }
    __user_stacks_end = .;

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
