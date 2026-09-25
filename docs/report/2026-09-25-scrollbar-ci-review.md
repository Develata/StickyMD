# 2026-09-25 滚动条与模块 CI 审查

## 范围与结论

审查起点 `08419da171bfdfc0826f7b25da8f53627504b212`。USER 要求复核滚动条的正确性、职责边界、
长文性能、交互体验与模块并发验收。本轮沿用 plan 06/07/09/10/11 的已批准行为，不新增产品对象、
配置、线程或依赖，不变更持久化/输入法权威；以下均为既有合同内修复。

## 按严重程度排列的发现与修复

1. **P2 — 同 generation 的过期视口可能重新显示或参与命中。** 原回执只核对请求 scroll_y，
   resize/zoom/theme/selection 改变但位置相同时仍可能被接纳；旧布局也可能用于 Split 同步。
   回执现携带完整请求视口，在主线程与当前目标比较；不匹配时仅复用 worker 的语义树生成最新帧，
   不接纳过期像素。布局身份同时用于滑块、语义同步、选区/链接的新命中。
2. **P2 — 新增长文拖动性能入口没有进入 CI 任务图。** 原来只能显式指定 ignored test。
   现加入 Phase 03 性能组、render 模块与完整 CI performance 分片；覆盖/去重测试同步约束该任务。
3. **P2 — JSON 模式的成功性能输出被丢弃，CI 缺少模块诊断归档。** Rust runner 现在把有界命令
   摘要输出到 stderr，stdout 继续保留 JSON；workflow 保存计划和按 module/mode 区分的日志。
   日志捕获显式保留 native 非零退出码，聚合门仍拒绝失败、取消和意外 skip。
4. **P3 — 鼠标靠近的加粗区域偏窄。** 原来只有 10 DIP 轨道命中才能触发加粗；现在整个预留
   窄槽都可触发，点击/拖动仍使用避开 resize 边界与 Split 同步按钮的轨道。

## 内聚、复杂度与 CI 设计取舍

- Preview worker 合并为 Build 与完整视口更新两类请求，减少分散的部分字段合并分支。
  视口更新调用现有 pipeline 快速路径：尺寸/缩放/主题相同时仅 paint；不重新解析或布局全文。
  UI 仅持有当前 frame 的小型视口 key，比较为 O(1)，不新增文档副本。
- Source 沿用按 generation/视口缓存的末端一屏测量，拖动仍按逻辑行定位；不增加全文高度索引、
  第二套排版器或复杂增量树。长短段落的比例仍是已批准的逻辑行近似。
- 模块注册/反向依赖/全量回退与聚合继续由 std-only Rust CLI 持有；workflow 只负责隔离 runner、
  缓存、转发和归档。日常模块并发、完整 tests/performance 分片、fail-fast=false 与日常新提交取消
  均保留；同一 runner 的性能测试统一串行，避免相互竞争污染样本。
- Source scrollbar、Phase 05、11-B、14 的 render 单元性能入口使用 `--lib`，不构建/链接无关
  integration targets。跨 crate 的现有入口继续保留其覆盖范围。
- 没有把一次性 PowerShell native-message 探针升级为正式 Rust qualification。人工输入法、物理
  鼠标手感和发布 exact-artifact 身份继续独立验收，不因 headless 通过自动升级。

## 已执行的验证与待补证据

- Windows `cargo test --workspace --locked`：646 passed，19 ignored，0 failed；ignored 项为 opt-in
  性能入口。新增 worker 回归验证 resize/zoom/反向滚动合并，普通滚动不增加 parse/layout counters。
- `cargo clippy --workspace --all-targets --locked -- -D warnings` 通过；`actionlint` 通过。
- PowerShell 实际子进程检查：stdout、stderr 同时归档，`exit 7` 经 Tee-Object 后仍为 7。
- 证据目录：`dist/evidence/scrollbar-review-20260925-*/`。后续性能与本地窗口结果见追加 Resolution。
- 远程 Actions 未在本轮执行；本地检查不证明远程 job 的实际耗时、缓存命中或上传成功。
  真实物理鼠标、完整主题/DPI/IME 矩阵保持 `NOT TESTED`；本地构建不属于 v0.1.0 已发布 artifact。

## Resolution — 2026-09-25 性能与窗口复核

- `modules run render --mode=performance` 完整执行 8 个任务（治理 + 7 个性能组），10 项 opt-in
  性能测试全部通过。Source 1,100,000 bytes / 100 次跳转绘制：median 1.2891 ms，p95 1.7528 ms，
  max 57.901 ms；生产图片 adapter 下 1 MiB 纯文本 Preview scroll p95 0.2302 ms、added_layouts=0；
  5,000 行代码块 scroll p95 0.4666 ms。均为本机诊断，不能把 p95 当成最大延迟，亦不等同于真实
  输入到屏幕延迟或隔离 host 的 release performance receipt。
- Release 构建通过。EXE 为 8,563,712 bytes，SHA-256
  `569559753F9C497B630CA89C5ED3B056BF3F67604AD70F87CD455283428494BE`。
- 首轮合成 hover 检查失败，保留在 `desktop-final/`：两张截图均为细滑块。检查 winit 的
  `TrackMouseEvent(TME_LEAVE)`/`CursorLeft` 路径后，增加真实鼠标进入测试窗口的定向观察。
  `desktop-confirmed/` 在相同 EXE 上通过：窄槽内、轨道外的位置可加粗滑块（所扫描行的纯色像素
  从 2 增至 6；3/7 DIP 几何的边缘有抗锯齿）。结合两组结果，首轮失败归因于合成消息无法维持
  真实 hover 状态；没有为通过探针而改变产品的 CursorLeft 语义。
- 同一份本地窗口结果还确认 Source/Preview 首尾、release 后停止拖动、Split 同步开启/关闭、
  20 次快速反向 Preview 拖动最终停在顶部；测试 note 的 SHA-256 不变。只操作独立 fixture，
  原用户实例 PID 29116 继续运行。真实鼠标观察后已恢复原位置与 foreground；没有对用户便签输入。
- 自动化窗口输入仍以合成消息为主；此次只补了 100% DPI / Light 的真实 hover 观察，不升级
  人工验收矩阵。物理拖动手感、完整 Light/Dark/DPI 与真实 IME 仍为 `NOT TESTED`。
- Phase 00 governance/readiness 通过，`ci plan --full` 与 render 性能计划均保存到
  `dist/evidence/scrollbar-review-20260925-175005/`。Actions 语法、Rust job 聚合和日志退出码已在
  本地检查；远程执行、缓存收益及 CI 总耗时仍未验证。
