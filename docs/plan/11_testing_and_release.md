# 11_testing_and_release.md - 测试与发布合同

## Metadata

- `Layer`: Verification
- `Status`: Approved Contract
- `Version`: 0.1.0
- `Last Review`: 2026-08-19
- `Scope`: v1 测试类别合同与发布形态合同；Phase 0 只定义契约，不实现测试与 workflow

---

## Purpose

定义 StickyMD v1 的验证体系与发布形态。本阶段只建立 contract；
测试实现与 CI workflow 在后续阶段逐步落地。

## Boundary

- 验收案例的具体内容在 `docs/acceptance-cases/`；本章定义类别与规则。
- 性能目标的性质定义在 `10_performance_reliability.md`。

---

## 测试类别合同

### Unit（单元）

- 文本编辑：UTF-8 byte range、CJK、emoji、combining mark、selection 替换、
  undo grouping、256/4 MiB 限制、IME commit 一次撤销。
- Markdown 转换：所有 CommonMark block、GFM 表格、task list、strikethrough、
  autolink、四种公式 delimiter、转义 dollar、code 中公式标记、raw HTML literal、
  reference link/image、本地/远程图片、malformed input 不 panic。
- 数学 fixture：分数、根式、上下标、积分、求和、极限、矩阵、cases、align、
  可伸缩括号、Greek、`\mathbb`、`\mathbf`、`\operatorname`、Unicode 数学字符、
  错误公式、超长公式。
- 文件：UTF-8 BOM、CRLF/LF、混合换行、原子替换、temp 恢复、config 损坏、
  无效 UTF-8、外部删除、自身写入 watcher 忽略、脏冲突。
- 图片：编码保留、bitmap 转 PNG、hash 去重、多图粘贴、managed/user 区分、
  move to trash、undo restore、redo re-trash、启动恢复、启动清理、路径穿越、
  remote 不下载、超限占位符。

### Property（property-based）

- 任意 Unicode TextDelta 不破坏 UTF-8。
- undo 后恢复原文；redo 后恢复编辑后文本。
- 任意图片事务最终与引用状态一致。
- 任意窗口几何变化后窗口仍在至少一个工作区内。
- 任意配置缺字段时使用默认值。
- Markdown AST 转换不 panic。

### Fuzz

```text
fuzz_markdown_to_owned_ast
fuzz_render_tree_builder
fuzz_managed_asset_scanner
fuzz_local_path_normalizer
fuzz_text_delta
```

定时运行，不阻塞普通快速 CI。

### Golden（golden tests）

数学与预览使用固定测试字体，覆盖 Light/Dark × 100/150/200% DPI；
允许极小 anti-aliasing tolerance，不允许大范围 mismatch。

### 手工 Windows 11 验收

- 系统：当前与前一个受支持 Windows 11 版本；100/125/150/200% DPI；
  单显示器、双显示器（同 DPI / 混合 DPI / 左侧 / 上方）、运行中断开外接、
  sleep/resume、RDP reconnect。
- 输入法：微软拼音、微信输入法（验证项见 `07_editor_and_ime.md`）。
- 窗口与文件矩阵见 `09_windows_shell.md`、`05_document_persistence.md`。

### 文件故障注入

写失败、替换失败、kill 进程后 temp 恢复、config 损坏、外部删除、无效 UTF-8、
双实例、无写权限。

### 内存测量

按 `10_performance_reliability.md` 的测量口径执行。

---

## CI 合同（方向性，后续阶段落地）

- Windows job：fmt --check、clippy -D warnings、tests --locked、release build、
  cargo deny。
- Portable-core job：在 Linux runner 上只构建平台无关 crates（防止平台无关代码
  被 Win32 污染）；目的不是发布 Linux app。
- Scheduled：advisories、依赖更新 dry-run（不自动合并）、fuzz smoke、
  sanitizer/Miri 平台无关核心、许可证报告。
- 失败日志与 math/preview diff 作为 artifact 上传。

---

## Release 合同

### 触发与步骤（方向性）

tag `v*` 触发：版本一致性校验 → 测试 → deny → release build → manifest 检查
→ smoke test → portable ZIP → SHA-256 checksums → 许可证 notice → SBOM
→ provenance/attestation → draft release → 人工 Windows 11 验收后发布。

### 发布物

```text
StickyMD-v1.0.0-windows-x64-portable.zip
├─ StickyMD.exe
├─ README.txt
├─ LICENSE.txt
├─ THIRD_PARTY_NOTICES.txt
└─ licenses\
   ├─ SIL-OFL-1.1.txt
   └─ KaTeX-fonts-NOTICE.txt

StickyMD-v1.0.0-SHA256SUMS.txt
StickyMD-v1.0.0-symbols.zip
SBOM.spdx.json
```

- 不预创建用户 `note/`（首次运行创建）。
- v1 不提供：MSI、MSIX、Microsoft Store、自动更新器、管理员安装、
  Program Files 安装。代码签名可后续加入，不阻塞开源 v1。
- License：MIT；数学字体为 OFL 1.1，release 必须附带相应声明。

### unsafe 边界合同

- `stickymd-core` / `stickymd-render`：`#![forbid(unsafe_code)]`。
- `stickymd-win`：`#![deny(unsafe_op_in_unsafe_fn)]`；所有 unsafe 只位于
  `platform/windows/` 或经批准的 RichEdit fallback，紧邻 `SAFETY` 注释，
  不把裸句柄泄漏到核心层。

### 依赖治理合同

- 保留 `Cargo.lock`，正式构建使用 `--locked`。
- 新增依赖前检查：许可证、transitive、二进制体积、MSRV、现有依赖能否完成。
- 禁止依赖清单见根 `AGENTS.md`；例外需 ADR + USER 批准。

---

## Failure Paths

- release gate 未过（测试/内存/体积）：不发布。
- 依赖 advisory：按 scheduled 报告处理，不静默忽略。
- 手工验收未完成的 draft release：不得公开。

## Configuration

Not applicable。

## Lifecycle

契约在 Phase 0 建立；实现在后续阶段；release 以 Definition of Done 全过为准。

## Extension / Replacement Points

fuzz 引擎、golden 容差策略。

## Performance Critical Paths

Not applicable（由 `10` 持有）。

## Verification

本章节自身的验证 = 后续阶段逐项落实并回写完成状态。

## Non-Goals

自动更新、遥测上报、在线 CI 缓存之外的云服务、多平台发布。
