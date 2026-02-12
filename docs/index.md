---
layout: home

hero:
  name: AegisOS
  text: Microkernel AArch64
  tagline: Hệ điều hành bare-metal cho hệ thống an toàn tới hạn — tên lửa, y tế, xe tự lái. Viết bằng Rust, không heap, không phụ thuộc ngoài.
  actions:
    - theme: brand
      text: 📝 Đọc Blog
      link: /blog/01-tai-sao-chung-ta-can-mot-he-dieu-hanh
    - theme: alt
      text: 📋 Kế hoạch
      link: /plan/01-plan-first-heartbeat_2026-02-10_00-00

features:
  - icon: 🛡️
    title: An toàn tới hạn
    details: Thiết kế theo DO-178C (hàng không), IEC 62304 (y tế), ISO 26262 (ô tô). Zero heap, zero external dependencies.
    link: /standard/01-DO-178C-hang-khong
    linkText: Xem tiêu chuẩn

  - icon: 🦀
    title: 100% Rust + ASM
    details: Kernel viết bằng no_std Rust và AArch64 assembly. Không dùng thư viện ngoài, không floating-point.

  - icon: 🔬
    title: Microkernel
    details: Chỉ giữ tối thiểu trong kernel — scheduler, IPC, capability. Driver chạy ở user-mode (EL0).

  - icon: 🚀
    title: QEMU Ready
    details: Chạy trên QEMU virt machine với Cortex-A53. Có 241 unit test, 32 QEMU checkpoint, và 10 Kani formal proof.

  - icon: 📖
    title: Blog cho học sinh lớp 5
    details: Mỗi phase đều có bài blog giải thích bằng tiếng Việt, dành cho các bạn nhỏ có ước mơ lớn.
    link: /blog/01-tai-sao-chung-ta-can-mot-he-dieu-hanh
    linkText: Đọc ngay

  - icon: 🧩
    title: 15 Phase phát triển
    details: Từ First Heartbeat → MMU → Scheduler → IPC → Fault Isolation → Capability → Address Space → User-Mode Driver → ELF Loader → Safety Assurance → Scale & Verify → Multi-ELF User Ecosystem.
    link: /plan/01-plan-first-heartbeat_2026-02-10_00-00
    linkText: Xem kế hoạch
---
