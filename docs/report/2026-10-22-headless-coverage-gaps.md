# 2026-10-22：19 项缺口的无桌面应用测试

本批为用户授权的 D3 后续，基线为 `8e921a2fbd6cdd0c6ffb9fd42d8df37d316e0688`，
在 `codex/c2d-c3-coverage` 继续实施。尚未做跨模型审查。

## 背景与边界

旧证据包 `target/review-evidence/c2d-c3-2026-10-22/remaining-gaps.txt` 有 19 项具名缺口。
原测试多直接构造投影或纯判定的输入，未执行 `StickyApp` 对 canonical、配置、会话和回执的组合应用。
本批不操作桌面；不复用旧产品 EXE 的桌面结果。旧证据包保持原样。

采用真实 `StickyApp`、真实 canonical coordinator、Source/Preview 投影与实际 persistence/window
coordinator。每个 fixture 独占临时目录，关闭文件 watcher，禁用环境继承的启动诊断；worker 和
instance 句柄先释放，再移除自身目录。进程内一次性创建并在所属线程销毁 winit event sink，
不创建应用窗口或托盘，不运行原生事件循环，不发送物理输入，不操作用户剪贴板。
窗口大小、DPI、按键和当前视口由边界参数提供，判定与状态修改仍由产品调用的同一方法持有。
测试专用事件结算计数不进入产品构建。无新依赖、无新 runtime 对象或持久化权威。

## 已复现的问题与修复

实际焦点路径只取消编辑区 preedit，Search preedit 仍保留，因此 `window_guards` 的
`ime_composing` 在失焦后可能继续为真。步骤：打开 Search、设置尚未提交的 preedit、
执行 Focused(false) 的应用路径、读取 Search 与窗口 guard。修复前的回归测试明确失败，
错误为 `Search composition survived focus loss`（证据 03）。修复在同一失焦路径清除
Search preedit/cursor，保留已提交 query 和打开状态，不依赖后续 Ime::Disabled。
编辑区、Search、二者同时及二者皆无的组合均验证通过。

## 覆盖变化

| 原义务 | 本批应用级 owner 覆盖 |
| --- | --- |
| P08-D055 | 读取两类实际组字状态，失焦释放 guard 且不改正文/query |
| scroll-2 三项 | 实际 config × 三视图 × 长短文 × DPI × Content Zoom；槽内按下被消费 |
| scroll-3 drag-live-update | 两窗格逐次移动实际视口，验证每一步位置及文档端点 |
| scroll-4 text/generation/dirty 三项 | clean/dirty 两种 canonical 状态、选区/preedit、无新增保存 |
| scroll-4 四种 drag-exit | 两窗格 release/blur/view-switch/resize；结束后移动不再拖动 |
| scroll-5 Preview height | 缓存 document height 驱动两端和中间位置，frame/index 保持，未创建 worker |
| scroll-6 stale-receipt | 完整视口六字段逐一过期；不得替换 frame/offset；相符回执为正对照 |
| quit-2 两项 | 全部 WindowEffect、三种持久化屏障的八组合、每次结算计数及 autosave 顺序 |

这 16 项在静态表中新增应用级单元 owner；正式持有仍要求 plan 11 的完整、同 Source Freeze
且 owner 集合完全一致的 headless 收据。测试能运行、局部通过不自动建立该资格化身份。

余下三项仍为具名 gap：P08-D022 的 Enter、blur 两项，以及 Preview 窗外原生鼠标消息/捕获。
透明度新增五项应用测试已覆盖 Enter/repeat/synthetic、其他点击/窗口失焦/隐藏/退出提交、
composition 拒绝聚焦、IME 目标判定、迟到 IME 事件和 Esc/保存/F6/F7/忙碌导出优先级。
空闲导出对话框、原生输入及实际 IME 开关仍需桌面；注入越界坐标只证明钳位，不能证明 Windows
把窗口外的鼠标消息送达 Preview。旧 Source 桌面观察也不能填补 Preview。

## 自查与验证记录

第一轮核对 plan 07/09/11、应用调用点、状态权威和 fixture 范围。发现并修复上述 Search 问题；
同时禁用 fixture 对环境诊断路径的继承。原生读取包装保留在同层，新增方法只拆分读取与应用，
没有跨层写盘。大文件只新增同职责入口，新增成组测试置于子模块；不为行数机械拆分既有职责。

第二轮故障注入：把完整视口比较改为 `same_layout`，新测试在 scroll_y 0 与 420 的差异处失败；
把退出提示结算移到 autosave 之前，事件顺序测试失败。两项故意错误均已恢复；证据 09/10
是预期失败，不算产品通过。恢复后证据 11 的 20 项定向测试通过（包括既有相关测试）。
证据 08 的应用组为 98 passed / 2 ignored；这两项 ignored 为 Release 性能场景，未当作通过。

第三轮完整基线、owner 核验、最终 Git 身份与逐文件摘要记录在新的
`target/review-evidence/coverage-headless-2026-10-22/`，以其 README 与验证日志为准。
每次验证保存当时相对基线的完整 diff 及 SHA-256，失败、编译错误和故障注入均保留。
最终证据另保存最终提交相对基线、main、父提交的 diff，避免把不同工作区状态混作一次验证。

## 可选后续与规格影响

无需修改 plan：修复恢复既有失焦语义，新增测试服务现有义务。下一步可在获得桌面授权后补
余下三项的真实输入场景，并对新产品构建重新做桌面回归；再按既有流程建立正式候选证据。
人工状态仍为 `NOT TESTED`，整体 `NOT_READY`。本批不 push、不合并 main、不打 tag 或发布，
不改 `tmp/` 与既有 RISK 文件。完成自查不等于跨模型审查或人工验收。

补充：覆盖表“未创建 worker”专指 Preview worker；fixture 确实使用真实 persistence worker，
其写入和清理均限于自身临时目录。

## Resolution：Release 测试构建兼容（2026-10-22）

首轮固定提交 `92e7ea9` 的完整 CI 已通过工作区测试和全部 owner 核验，但在 Phase 3 Release
测试构建失败：新断言调用的 `PersistenceWorker::metrics` 原来仅在 `debug_assertions` 可见。
把该只读 getter 的条件扩展为 `any(debug_assertions, test)`，使 Release 测试也能读取已有计数。
产品 Release 的条件仍为假，既有 mailbox 计数、线程、I/O 与运行时行为不变。
证据 13 保留失败状态；修复后的定向 Release 测试和完整复验另存，不覆盖首轮记录。
