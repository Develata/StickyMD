# 共享 crate 依赖边界：计划未定的两处与已知局限

日期：2026-11-19。状态：问题 1 **USER 2026-11-19 选 A**，已写入 plan 03 并由检查执行；问题 2 **等待 USER 决定**。

## 背景

治理检查 `tools/stickymd-smoke/src/governance/crate_graph.rs` 读 `cargo metadata --locked --all-features`，按
[plan 03 共享实现节](../plan/03_system_architecture.md#shared-crates) 检查五个共享 crate：清单各表的声明、解析后的本地边、
经 normal 边可达的能力 crate。`stickymd-core`、`stickymd-render`、`stickymd-app`、`stickymd-desktop` 的允许依赖都能从计划
推出；`stickymd-posix` 只能推出一部分。

## 计划写明的部分（已检查）

- 平台无关共享 crate（core、render、app、desktop）不依赖 `stickymd-posix`（03 第 97 行）。
- `stickymd-posix` 是 Execution Domain 的原生 adapter（03 crate 表），不得依赖共享 Interaction Shell `stickymd-desktop`
  （03“禁止的跨层关系”第 3 条：Execution Domain 不反向调用 Shell）。
- 共享库不依赖平台宿主（03 依赖方向图），也不依赖 `stickymd-smoke`（`AGENTS.md`：不进入产品 runtime）。

## 问题 1：`stickymd-posix` 可否依赖 core、render、app

计划只说它“无业务逻辑”，没有说它能否用其他 StickyMD crate。现状：它不依赖任何 StickyMD crate，只用 `libc`、`sha2`、`thiserror`。

| 选项 | 含义 |
| --- | --- |
| **A（建议）** | 不依赖任何 StickyMD crate：只放原语，宿主把原语包装成 Capability Port 实现。与现状一致，Debian 复用时没有额外耦合 |
| B | 可依赖 `stickymd-core`（领域类型），不依赖 render 与 app |
| C | 不限制 |

**决定（USER 2026-11-19）：A。** 已写入 03 crate 表“边界”列与第 97 行之后的说明；检查拒绝 `stickymd-posix` 到任何
本地 crate 的 normal 或 build 边（dev 边只查成员身份）。

## 问题 2：原生绑定 crate 的名单

03 规则 8 已定：项目手写原生调用只在宿主 adapter 与 `stickymd-posix`，第三方库可以持有经批准的平台后端。所以检查只拒绝
**声明**绑定 crate，不拒绝经第三方的传递依赖（`sha2` 经 `cpufeatures` 带入 `libc`，`stickymd-render` 经 `walkdir` 带入
`windows-sys`，`winit` 带入 `objc2`）。现行直接禁表：core、render、app 不声明 `windows`、`raw-window-handle`；desktop 不声明
`windows`（它本身使用 `raw-window-handle`）；`stickymd-posix` 无绑定禁表（它就是原生层）。

计划没有给出完整名单。平台无关共享 crate 若直接声明 `libc`、`windows-sys`、`objc2` 系列或 `block2`，检查不拦：
`#![forbid(unsafe_code)]` 不管清单声明，也挡不住 `objc2-foundation` 这类绑定的安全 API（如 `NSString::from_str`）。
第三方后端是否“经批准”，检查同样不判断。这两项目前靠人工审查。待定的是名单本身与维护方式，建议：名单取宿主与
`stickymd-posix` 实际使用的绑定 crate，宿主引入新绑定 crate 时同一提交加入。未经批准不扩名单。

## 已知局限（不修，原因）

- **可达检查会多报，不会漏报。** 解析图合并了全部成员的 feature 与全部 target：宿主启用的 feature 可能让共享 crate 名下
  多出一条边（今天的例子：`stickymd-render → image → tiff` 只在合并图中出现，`cargo tree -p stickymd-render` 里没有），互斥 target
  的路径也会被拼起来。逐包、逐 target 复核需要解析 `cargo tree` 文本或多次运行 metadata，代价与脆弱性都高于收益。
  失败信息给出逐 target 的复核命令 `cargo tree -p <crate> -e normal --all-features --target <triple> -i <dep>@<version>`
  （`--target all` 同样会拼接互斥 target，包名有多个版本时须带版本）；确认是多报后检查仍然失败，交 USER 决定。
- **CI 下载量。** 不加 `--filter-platform` 的 metadata 需要锁文件中全部 target 的包（当前 368 个）。下载缓存按 OS 共享，
  冷缓存时 Ubuntu planner 的 `phase 00` 会多下载一次；耗时未在远端测量。
