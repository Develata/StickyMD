# 预览优化与桌面覆盖分支合并记录（2026-10-22）

## 结论与授权

USER 已确认预览优化原 1–7 项及长公式横向滚动补验通过，随后明确回复另一 session
已经停止，“你可以帮它合掉”。据此将两条分支完整整合到本地 `main`，保留原提交历史。
没有 push、tag 或发布；本记录不改变历史版本及完整发布资格化的状态。

## 分支身份与整合方式

| 对象 | 提交 |
| --- | --- |
| 原本地 main | `e169ad92c6b4ce4eccdef0b89d3bc08eb0b4f480` |
| fetch 后的 origin/main | `5a8f0cad3447f75102f5925667ff0887d076b6d8` |
| 预览优化分支 | `fd98d9521603e8a0475a571921dccf0d8f092635` |
| 桌面覆盖分支 | `5d27e428966127f157618d7b866474287db73ab2` |
| 已验证的整合提交 | `70c06c6b3c0361fee6f1793d6ce46bcbf4cd8f67` |
| 整合 tree | `9e028b0a60791c690cd1b2ba373ab0c28dfb7ae7` |

预览分支基点已包含覆盖分支相对 origin/main 的前 30 个提交，所以先明确另一 session
的合并范围。USER 授权后在现有隔离 worktree 整合其余 7 个提交，验证通过后将 main
快进到整合提交。两条分支仅共同修改 `docs/coverage-matrix.md`，自动合并无冲突；
无需重写、摘取或丢弃既有提交。

相对已人工补验 source `613e82f`，产品源码的差异仅为 `flow/editor.rs` 中两处
`cfg(test)` 示例文字；另有一份渲染测试夹具的文字调整。其余后续改动属于验证工具、
覆盖映射和文档。没有为合并修改运行逻辑、缓存预算、依赖或工程骨架。

## 验证与证据

对 clean source `70c06c6` 新运行以下检查：

- 完整 `stickymd-smoke all --ci`：20/20 项 PASSED，含 workspace tests、45 个 unit owner
  核验及全部既有无界面 Release 性能入口；收据 `worktree_dirty=false`。
- `cargo fmt --all -- --check`、全工作区/all targets/warnings denied 的 clippy 通过。
- `cargo build -p stickymd-win --release --locked` 通过；PE 检查无 developer runtime imports。
- 合并前后 diff/分支祖先关系核验通过，origin/main 和两条开发分支均被整合提交包含。

整合 Release EXE 为 8,642,048 bytes，SHA-256：
`08EEA371B335627AADE3355BBB37D5DEE6E807B4AD9DA9C74684316CEE744C09`。
没有覆盖 USER 已验收的 `formula-scroll-613e82f` 包或将新 EXE 冒称为该包的相同字节。
本报告之后的纯文档提交不改变上述验证源码和构建身份。

另一 session 的固定证据包 `target/review-evidence/coverage-desktop-2026-10-22/` 全部
345 个被列文件重新校验通过；清单 SHA-256：
`d04b1ea77487af7601e8daf85cc3e42726c313366bc44317283e53a328889e84`。
其中 124 项原生检查和 20 项 headless 结果属于 `5d27e42`，没有冒充本轮重新执行的桌面测试。

本轮日志、来源记录和 JSON 位于隔离 worktree 的 `target/integration-20261022/`。
新的 `headless-ci.json` SHA-256：
`9FA56BB11A382119E0CEDB178D62B055F5A28E5B803AC10D02A1D136505D686E`。
原工作区未跟踪的 RISK 文件没有纳入提交；其原 SHA-256 为
`E8FD89641DAE85714DE26D13FE5D82110896104B68C3EFCF8B90E662FA6FC7D3`。

## 验收边界

本次局部 USER 验收见[公式修复记录](2026-10-22-display-math-horizontal-scroll.md)及
[原预览验收记录](2026-10-22-preview-resource-acceptance.md)。没有重跑整合 EXE 的
物理桌面 G4、完整 OS 资源矩阵或正式候选人工矩阵；原证据与这些剩余边界继续保留。
另一 session 的同模型 review 与未执行跨模型 review 按其原记录保留，不重新标记。
