/* AegisOS User Task Linker Script — logger (slot 2)
 * Links at 0x4010_8000 — ELF load slot 2 (task 4).
 */

ENTRY(_start)

SECTIONS
{
    . = 0x40108000;

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
