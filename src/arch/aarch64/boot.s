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
    /* Setup stack pointer */
    /* Cài đặt con trỏ ngăn xếp */
    ldr x0, =__stack_end
    mov sp, x0

    /* Check EL — QEMU virt may start at EL2 or EL1 */
    /* Kiểm tra EL — QEMU virt có thể khởi động ở EL2 hoặc EL1 */
    mrs x0, CurrentEL
    lsr x0, x0, #2
    cmp x0, #2
    b.ne at_el1

    /* === Drop from EL2 to EL1 === */
    /* Chuyển từ EL2 xuống EL1 */
    mrs x0, hcr_el2
    orr x0, x0, #(1 << 31)   /* HCR_EL2.RW = 1 (EL1 is AArch64) */
    msr hcr_el2, x0

    mov x0, #0x33FF
    msr cptr_el2, x0
    msr hstr_el2, xzr

    /* SCTLR_EL1 reset value */
    /* Giá trị khởi tạo SCTLR_EL1 */
    mov x0, #0x0800
    movk x0, #0x30D0, lsl #16
    msr sctlr_el1, x0

    /* Enable EL1 physical timer access from EL2 */
    /* Cho phép EL1 truy cập bộ đếm thời gian vật lý từ EL2 */
    mrs x0, CNTHCTL_EL2
    orr x0, x0, #3        /* EL1PCTEN + EL1PCEN */
    msr CNTHCTL_EL2, x0
    msr CNTVOFF_EL2, xzr  /* Zero virtual offset */

    /* Return to EL1h */
    /* Quay lại EL1h */
    mov x0, #0x3C5
    msr spsr_el2, x0
    adr x0, at_el1
    msr elr_el2, x0
    eret

at_el1:
    /* Re-setup SP (SP_EL1 after eret) */
    /* Cài đặt lại con trỏ ngăn xếp (SP_EL1 sau eret) */
    ldr x0, =__stack_end
    mov sp, x0

    /* Clear BSS + page tables */
    /* Xóa BSS + bảng trang */
    ldr x0, =__bss_start
    ldr x1, =__page_tables_end

2:
    cmp x0, x1
    b.eq 3f
    str xzr, [x0], #8
    b 2b

3:
    /* === MMU Setup === */

    /* Build page tables in Rust */
    /* Khởi tạo bảng trang trong Rust */
    bl  mmu_init

    /* Invalidate all TLB entries */
    /* Vô hiệu hóa tất cả các mục TLB */
    tlbi vmalle1
    dsb  ish
    isb

    /* MAIR_EL1: idx0=Device-nGnRnE(0x00), idx1=Normal-NC(0x44),
                  idx2=Normal-WB(0xFF), idx3=Device-nGnRE(0x04) */
    ldr x0, =0x04FF4400
    msr mair_el1, x0

    /* TCR_EL1: 39-bit VA, 4KB granule, TTBR0 only, 48-bit PA */
    /* TCR_EL1: 39-bit địa chỉ ảo, kích thước trang 4KB, chỉ dùng TTBR0, 48-bit địa chỉ vật lý */
    ldr x0, =0x5B5993519
    msr tcr_el1, x0

    /* TTBR0_EL1 = kernel boot L1 (page 13 in .page_tables) */
    /* TTBR0_EL1 = bảng trang cấp 1 khởi động kernel (trang 13 trong .page_tables) */
    ldr x0, =__page_tables_start
    add x0, x0, #(13 * 4096)
    msr ttbr0_el1, x0

    isb

    /* Enable MMU: set M + C + SA + I + WXN in SCTLR_EL1 */
    /* Bật MMU: đặt các bit M + C + SA + I + WXN trong SCTLR_EL1 */
    mrs x0, sctlr_el1
    ldr x1, =0x0008100D
    orr x0, x0, x1
    msr sctlr_el1, x0
    isb

    /* MMU is now active — jump to Rust */
    /* MMU đã được kích hoạt — nhảy vào Rust */
    bl  kernel_main

4:
    wfe
    b 4b
