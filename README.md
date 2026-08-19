# StickyMD

StickyMD 是一个极致轻量、常驻 Windows 11 桌面的便携式 Markdown 临时草稿纸：
打开即写，自动保存，公式可靠，贴边即隐，需要时迅速出现。

它不是 Obsidian，不是 Typora，不是知识管理工具，也不是通用 Markdown 编辑器。

---

## Current status

```text
阶段：architecture / governance initialization（Phase 0）
```

**当前仓库尚未进入正式功能实现阶段。**
仓库中只有治理文档与架构契约，没有可运行程序，也没有任何 crate 依赖。

- 目标平台：Windows 11 x64
- 实现主体：Rust（winit / cosmic-text / tiny-skia / softbuffer / Comrak / RaTeX 为已批准架构方向）
- 交付形式：单 EXE portable，解压即用，一个程序目录就是一张便签
- License：MIT

---

## Documentation

| 文档 | 说明 |
| --- | --- |
| [AGENTS.md](AGENTS.md) | Agent 工作总入口与强制工作流 |
| [工程宪法](docs/plan/00_engineering_constitution.md) | 系统设计与演化的最高约束 |
| [架构契约](docs/plan/) | 唯一架构权威文档树 |
| [术语表](docs/plan/01_terminology.md) | 核心术语固定定义 |
| [产品行为投影](docs/features/00_v1_product_behavior.md) | v1 用户可见行为 |
| [验收合同](docs/acceptance-cases/00_v1_acceptance.md) | v1 可验证案例 |
| [架构概览](docs/overview/architecture.md) | 可读版架构投影 |
| [覆盖矩阵](docs/coverage-matrix.md) | plan ↔ feature ↔ acceptance ↔ 未来代码 |

---

## License

MIT — 见 [LICENSE](LICENSE)。
