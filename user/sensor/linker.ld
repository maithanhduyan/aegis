/* AegisOS User Task Linker Script — sensor (slot 1)
 * Links at 0x4010_4000 — ELF load slot 1 (task 3).
 */

ENTRY(_start)

SECTIONS
{
    . = 0x40104000;

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
