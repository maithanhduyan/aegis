.section .text._start
.global _start

_start:
    /* Chỉ core 0 chạy, các core khác park */
    mrs x0, mpidr_el1
    and x0, x0, #3
    cbz x0, 1f

0:
    wfe
    b 0b

1:
    /* Setup stack pointer từ vùng stack riêng */
    ldr x0, =__stack_end
    mov sp, x0

    /* Clear BSS */
    ldr x0, =__bss_start
    ldr x1, =__bss_end

2:
    cmp x0, x1
    b.eq 3f
    str xzr, [x0], #8
    b 2b

3:
    bl kernel_main

4:
    wfe
    b 4b
