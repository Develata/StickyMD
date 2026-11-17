# macOS 验收证据改按产品输入指纹锚定（提议，2026-11-17）

## 背景与 USER 意见

[plan 11 #phase-16-macos-host](../plan/11_testing_and_release.md#phase-16-macos-host) 规定：

- `mac-gui` 账本的 PASS 只在 app 可执行文件、bundle 清单、驱动、虚拟机镜像、系统版本五项与当前完全一致时复用；
- USER 的人工收据绑定 app 可执行文件的 SHA-256，app 一变，收据不再成立。

v0.2.0 只改了版本号，但这会改变可执行文件的字节：Cargo 把包版本混入符号的元数据哈希；`build-app.sh` 的 ad-hoc 签名又把
Info.plist（含版本号）的摘要写进可执行文件的签名。于是 Phase 16 的全部自动化 PASS 与人工收据对 0.2.0 构建都不成立。

USER（2026-11-17）：“那我觉得这个流程或许有些问题，我们的锚定点最后采用 commit 编号或者其它的，而不是纯粹的哈希”；
对下文建议的方向回复：“我同意你的建议”。本报告给出细化设计，其中三项需要 USER 决定（见末节）。

## 选项

| 方案 | 做法 | 问题 |
| --- | --- | --- |
| A. 提交号 | 证据绑定构建所用的 commit | 只改文档也换提交号，和现在一样要重跑；也不包含工具链、目标与构建配置 |
| B. 产品输入指纹（建议） | 证据绑定决定程序行为的输入的摘要：产品源码、manifest 与锁文件（版本号规范化）、打包脚本与 Info.plist 模板、工具链、目标与构建配置 | 输入分类漏掉某个真实输入时会误复用；需要保守分类与治理检查 |
| C. 保留可执行文件哈希，把版本号移出二进制 | 只在 Info.plist 写版本 | 签名仍带 Info.plist 摘要；任何与行为无关的源码改动（注释、文档注释）仍使证据失效 |

Windows 的资格化模块已经用 B（[#module-success-ledger](../plan/11_testing_and_release.md#module-success-ledger)，指纹 v2 对根 `Cargo.toml`、`Cargo.lock`
的版本行做规范化），所以 B 是把 Windows 已有的锚定方式推广到 macOS，而不是另起一套。

## 建议设计

1. **自动化行的身份：** 用“macOS 产品输入指纹”替换可执行文件与 bundle 清单两项；驱动（测试工具本身）、镜像 digest 与系统版本三项保留，仍须完全一致。
   每条记录另存可执行文件与 bundle 清单的 SHA-256，只供审计，不参与复用判定。
2. **人工收据：** 改为绑定同一指纹。`NEVER_REUSED`（I01–I03 输入法、R03 下载包的 App Translocation）照旧每次确认，不复用。
3. **指纹的输入：** 沿用 `qualification/module_ledger/fingerprint.rs` 的机制（排序后的相对路径与文件字节，SHA-256，指纹头带算法版本），新增 macOS 产品域：
   - `apps/stickymd-mac/**`（含 `packaging/build-app.sh` 与 `Info.plist` 模板）；
   - 它链接的共享 crate：`stickymd-core`、`stickymd-render`、`stickymd-app`、`stickymd-desktop`、`stickymd-posix` 的全部文件；
   - 根 `Cargo.toml`、`Cargo.lock` 与各 member manifest（版本按 v2 规则规范化）、`rust-toolchain` 文件（若有）、`.cargo/` 配置；
   - 构建身份：`rustc -vV` 的输出、目标 `aarch64-apple-darwin`、`--release --locked`。
   - 不属于任何已知域的 tracked 文件一律算作输入（宁可重跑，不误复用），与 Windows 规则相同；文档、Windows 宿主与 smoke 工具（驱动另行按字节绑定）不是输入。
4. **内部依赖的版本要求：** 0.2.0 不得不把各 member manifest 里 `stickymd-* = { path = …, version = "0.1.0" }` 改成 `0.2.0`
   （0.x 下 `^0.1.0` 不接受 `0.2.0`）。这些行不在 v2 规范化范围内，所以 Windows 模块这次也全部失效。
   建议把内部 crate 改为只写 `path`（这些 crate 不发布；同时在 `[workspace.package]` 设 `publish = false` 防误发布），以后升版本只动 v2 已规范化的两个文件。
5. **迁移：** 现有账本记录只有可执行文件哈希，没有构建它的源码，无法换算成指纹，作为历史保留，不再参与复用。
   切换后在 `mac-test` 虚拟机重跑一次全部自动化行，USER 重新确认一次人工行；此后版本号与文档改动不再触发重跑。
6. **审计：** `plan` 发现同一指纹下记录过不同的可执行文件哈希时照常复用，但在输出中列出，便于事后核对构建是否可复现。

## 风险

- **漏分类导致误复用：** 一个影响行为的文件不在指纹里，旧证据被错误复用。缓解：未知路径默认算输入；macOS 域按目录整体纳入；
  治理检查（每次 CI 运行）拒绝没有规则的产品文件，和 Windows 一样。
- **工具链之外的环境差异：** Xcode Command Line Tools、系统 SDK 版本也会影响链接结果。建议把 `xcrun --show-sdk-version` 一并写入构建身份。
- **人工观察的语义：** 人工确认看的是实际渲染；输入相同则构建出的程序行为相同，这是 B 的前提。工具链或 SDK 变化会改变指纹，因此会要求重新确认。

## 对规格的影响

- 修改 plan 11 #phase-16-macos-host 的“记录”“复用”“人工收据”“自动化行”四条：身份由可执行文件哈希改为产品输入指纹（加驱动、镜像、系统），
  可执行文件与 bundle 哈希改为审计字段。
- 修改 #module-success-ledger：说明 macOS 域，以及内部依赖只写 `path` 后的 manifest 规则。
- 工具：`stickymd-smoke mac-ledger` 的 `record`、`plan`、`receipt` 改用指纹；记录格式升级版本号，旧格式只读。
- 不影响 Windows 发布包的 exact-byte 校验：发布出去的 ZIP 仍按字节绑定已验收的候选。

## 需要 USER 决定

1. 人工收据是否也改为绑定指纹（建议：是，`NEVER_REUSED` 照旧）。
2. 内部 crate 依赖改为只写 `path` 并设 `publish = false`（建议：是），还是扩展规范化器去识别这些版本行。
3. 切换后的一次性重跑：自动化行在 `mac-test` 重跑，人工行由 USER 重新确认一次（建议：在下一次 macOS 相关改动或下一版本前做）。

验证计划：指纹在“只改版本号”“只改文档”时不变、在共享 crate 或 macOS 宿主源码改动时改变的单元测试；`plan` 在新旧格式账本上的复用判定测试；
治理检查覆盖新域。

## Resolution — USER 批准（2026-11-17）

USER：“按建议实施，三项都同意”。据此：

1. 人工收据改为绑定产品输入指纹；I01–I03 与 R03 照旧不沿用。
2. 内部 crate 互相只按 `path` 引用，各 member 设 `publish = false`；`deny.toml` 加 `allow-wildcard-paths = true`
   （去掉这一项时 cargo-deny 把这些路径依赖报为通配符依赖，已验证）。
3. 切换后自动化行在 `mac-test` 重跑一次，人工行由 USER 重新确认一次；在下一次 macOS 相关改动或下一版本前做。

实现与本报告的差异：

- 指纹不单独建域规则，而是“除列出的目录与文件外全部算输入”，与报告的保守原则一致，规则更短。
- `xcrun --show-sdk-version` 已并入构建身份（报告“风险”一节的建议）。
- 指纹写进 Info.plist 的 `StickyMDInputs`；`build-app.sh` 在构建前后各算一次，不一致即拒绝。
- 旧账本记录（263 行）照常可读，作为历史保留，不参与复用。

plan 11 #phase-16-macos-host 与 #module-success-ledger 已按此改写。
