ENTRY(_start)

SECTIONS
{
    . = 0x40080000;

    .text : {
        KEEP(*(.text._start))
        *(.text*)
    }

    .rodata : { *(.rodata*) }
    .data   : { *(.data*)   }

    .bss : {
        . = ALIGN(16);
        __bss_start = .;
        *(.bss*);
        . = ALIGN(16);
        __bss_end = .;
    }

    . = ALIGN(16);
    __stack_start = .;
    . += 0x4000;        /* 16 KB stack */
    __stack_end = .;

    /DISCARD/ : { *(.comment*) *(.eh_frame*) *(.gcc_except_table*) }
}
