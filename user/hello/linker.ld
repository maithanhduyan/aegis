/* AegisOS User Task Linker Script
 * Links at 0x4010_0000 — the kernel's .elf_load region.
 * All sections merged into a single loadable segment for simplicity.
 */

ENTRY(_start)

SECTIONS
{
    . = 0x40100000;

    .text : {
        KEEP(*(.text._start))
        *(.text*)
        *(.rodata*)
        *(.data*)
    }

    .bss : {
        *(.bss*)
    }

    /DISCARD/ : { *(.comment*) *(.eh_frame*) *(.gcc_except_table*) }
}
