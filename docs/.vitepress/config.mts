import { defineConfig } from 'vitepress'

export default defineConfig({
  lang: 'vi-VN',
  title: 'AegisOS',
  description: 'Tài liệu dự án AegisOS — Microkernel AArch64 cho hệ thống an toàn tới hạn',

  cleanUrls: true,
  lastUpdated: true,
  ignoreDeadLinks: true,

  head: [
    ['link', { rel: 'icon', href: '/favicon.svg' }],
  ],

  themeConfig: {
    siteTitle: '🛡️ AegisOS',

    nav: [
      { text: 'Trang chủ', link: '/' },
      { text: 'Blog', link: '/blog/01-tai-sao-chung-ta-can-mot-he-dieu-hanh' },
      { text: 'Kế hoạch', link: '/plan/01-plan-first-heartbeat_2026-02-10_00-00' },
      { text: 'Tiêu chuẩn', link: '/standard/01-DO-178C-hang-khong' },
    ],

    sidebar: {
      '/blog/': [
        {
          text: '📝 Blog',
          items: [
            { text: '#01 — Tại Sao Cần HĐH', link: '/blog/01-tai-sao-chung-ta-can-mot-he-dieu-hanh' },
            { text: '#02 — Bộ Nhớ & Bảo Vệ', link: '/blog/02-bo-nho-la-gi-va-tai-sao-phai-bao-ve-no' },
            { text: '#03 — Đa Nhiệm', link: '/blog/03-day-may-tinh-lam-nhieu-viec-cung-luc' },
            { text: '#04 — Chìa Khóa & Cánh Cửa', link: '/blog/04-chia-khoa-va-canh-cua-bao-ve-kernel' },
            { text: '#05 — Fault Isolation', link: '/blog/05-khi-mot-task-nga-ca-he-thong-khong-duoc-nga-theo' },
            { text: '#06 — Hệ Thống An Toàn', link: '/blog/06-lam-sao-biet-he-thong-an-toan-that' },
            { text: '#07 — Giấy Phép Phần Mềm', link: '/blog/07-giay-phep-cho-phan-mem-ai-duoc-lam-gi' },
            { text: '#08 — Bản Đồ Riêng', link: '/blog/08-moi-chuong-trinh-mot-ban-do-rieng' },
            { text: '#09 — Chuông Cửa & Hàng Đợi', link: '/blog/09-chuong-cua-va-hang-doi-noi-chuyen-khong-can-cho' },
            { text: '#10 — User-Mode Driver', link: '/blog/10-khi-chuong-trinh-tu-noi-chuyen-voi-phan-cung' },
          ],
        },
      ],

      '/plan/': [
        {
          text: '📋 Kế hoạch phát triển',
          items: [
            { text: 'A — First Heartbeat', link: '/plan/01-plan-first-heartbeat_2026-02-10_00-00' },
            { text: 'B — MMU & Page Table', link: '/plan/02-plan-mmu-page-table-memory-model_2026-02-10_01-00' },
            { text: 'C — Exception, IPC, Scheduler', link: '/plan/03-plan-exception-ipc-scheduler_2026-02-10_02-00' },
            { text: 'D — User/Kernel Separation', link: '/plan/04-plan-user-kernel-separation_2026-02-11' },
            { text: 'E — Fault Isolation', link: '/plan/05-plan-fault-isolation_2026-02-11_22-00' },
            { text: 'F — Testing & CI', link: '/plan/06-plan-testing-infrastructure-ci_2026-02-11_23-00' },
            { text: 'G — Capability Access Control', link: '/plan/07-plan-capability-access-control_2026-02-11_23-30' },
            { text: 'H — Per-Task Address Space', link: '/plan/08-plan-per-task-address-space_2026-02-11_23-50' },
            { text: 'I — Enhanced IPC & Notifications', link: '/plan/09-plan-enhanced-ipc-notifications_2026-02-11_23-59' },
            { text: 'J — Shared Memory & IRQ Routing', link: '/plan/10-plan-shared-memory-irq-routing-user-driver_2026-02-12_00-30' },
          ],
        },
      ],

      '/standard/': [
        {
          text: '📐 Tiêu chuẩn an toàn',
          items: [
            { text: 'DO-178C — Hàng không', link: '/standard/01-DO-178C-hang-khong' },
            { text: 'IEC 62304 — Y tế', link: '/standard/02-IEC-62304-y-te' },
            { text: 'ISO 26262 — Ô tô', link: '/standard/03-ISO-26262-o-to' },
          ],
        },
      ],
    },

    socialLinks: [
      { icon: 'github', link: 'https://github.com/example/aegis' },
    ],

    search: {
      provider: 'local',
    },

    outline: {
      level: [2, 3],
      label: 'Mục lục',
    },

    docFooter: {
      prev: 'Trang trước',
      next: 'Trang sau',
    },

    darkModeSwitchLabel: 'Giao diện',
    sidebarMenuLabel: 'Menu',
    returnToTopLabel: 'Lên đầu trang',

    footer: {
      message: 'AegisOS — Microkernel AArch64 cho hệ thống an toàn tới hạn',
      copyright: '© 2026 AegisOS Project',
    },
  },

  // Exclude non-content dirs from routing
  srcExclude: [
    'prompts/**',
    'test/**',
    'idea/**',
    'node_modules/**',
  ],
})
