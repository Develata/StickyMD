# Phase 16 macOS 宿主

## Completion State

In Progress。M0–M4 的代码与自动化已完成；真实桌面、输入法与资源基线的人工项见
[phase-16 验收矩阵](../acceptance-cases/phase-16.md)，保持 `NOT TESTED` 直到实际运行。

## Goal

在 `aarch64-apple-darwin`（macOS 27 及以后）提供与 Windows 功能对等的宿主 `apps/stickymd-mac`。
Windows 产品行为不变，已批准的例外只有 S7、S8。

## Inputs

- [决定报告](../report/2026-11-09-macos-platform-decision.md)（USER 批准 2026-11-10）。
- `02` macOS 节、`05` macOS 存储、`07` macOS 输入、`08` macOS 剪贴板与资产、`09` macOS 实现映射、
  `10` macOS 门、[`11` Phase 16 节](../plan/11_testing_and_release.md#phase-16-macos-host)。

## Scope

| 里程碑 | 状态 | 内容 |
| --- | --- | --- |
| M0 | 完成 | macOS 字体库移除没有 `head` 表的字体（S1） |
| M1 | 完成 | crate、治理、CI 选择、指纹归属；存储、单实例、监视、启动 |
| M2 | 代码完成，真实桌面待人工 | 窗口、菜单栏图标、唤醒、光标图层、系统终止；共享层 S2、S4、S6、S7、S8 |
| M3 | 代码完成，真实桌面待人工 | 输入、剪贴板、导出；共享层 S3 |
| M4 | 打包完成，exFAT 与资源基线待解锁桌面 | `.app` 打包、ad-hoc 签名、Rust 资源测量 CLI |

## Out Of Scope

Intel、universal 包、公开分发、多显示器验证、收起态半透明、Debian。

## Verification

```bash
cargo test --locked --workspace --exclude stickymd-win
```

```bash
cargo clippy --locked --workspace --exclude stickymd-win --all-targets -- -D warnings
```

```bash
cargo test -p stickymd-mac exfat -- --ignored
```

```bash
apps/stickymd-mac/packaging/build-app.sh
```

```bash
cargo run --quiet --locked -p stickymd-smoke -- mac-resources --app dist/macos/StickyMD.app --out dist/evidence/mac-resources
```

`stickymd-win` 的 build script 只能在 Windows 上运行。exFAT 用例会创建并挂载一个 128 MB 的临时磁盘映像（卷标不超过 11 个字符）。

资源 CLI 只在测试 VM 的解锁桌面运行；每个可见场景至少 5 次，等待窗口出现后预热 30 秒、空闲采样 60 秒，
原始样本与汇总写入 `mac-resources.json`。Hidden 等待后续 GUI driver 操作菜单栏菜单，保持 `NOT_TESTED`；
退出码 0 仅表示 Source/Preview/Split 本次采样完整且未越内存门，不代表完整验收。

GUI 驱动只在专用 Tart macOS 27.2 VM（自动登录、解锁桌面、ABC/US 输入源）内运行：先用 `CARGO_PROFILE_RELEASE_PANIC=unwind cargo build --locked --release -p stickymd-smoke` 构建，将该 CLI 与 `StickyMD.app` 复制进 VM，再以登录用户执行 `./stickymd-smoke mac-gui --app "$PWD/StickyMD.app" --out "$PWD/gui-evidence"`（可加 `--only W03,W04`）。VM 检测先于路径访问，缺少 Accessibility/PostEvent 时明确拒绝，Screen Recording 仅用于可选截图；不得在主人的 Mac 上运行，也不得用 `--allow-host` 绕过本次限制。每场景使用独立程序目录，原子输出 `mac-gui.json`，子检查缺口保留 `NOT_TESTED`；具体权限、外部 oracle 与 VM 首跑待验证项见 [smoke README](../../tools/stickymd-smoke/README.md#macos-gui-evidence-in-a-dedicated-vm)。当前只实现并验证无 GUI 的驱动逻辑，首次真实运行等待 VM 就绪，验收矩阵不改为通过。

共享层改动的 Windows 桌面回归（S2、S4、S6、S7）只能在 Windows 机器上运行；S8 按 USER 决定不单独回归。2026-11-11 已在 Windows 上运行 P16-W15，未发现回归，未测部分 USER 豁免（[Windows 回归](../report/2026-11-11-windows-p16-w15-regression.md)）。
