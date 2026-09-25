# 2026-09-25 垂直滚动条维护验证

## 背景与范围

USER 确认：Source、Preview 都添加滚动条，Split 两侧各一条；长文常驻细滑块，鼠标靠近时加粗。
这是既有视口滚动的交互补充，契约见 `09_windows_shell.md#vertical-scrollbars`，不新增配置或文档权威。

## 实现与审查

- 各面板预留 20 DIP 窄槽，正文布局/绘制/命中共用缩减后的内容尺寸。滑块与窗口 6 DIP resize
  边界分离；Split 左侧轨道还避开同步按钮的 22 DIP 点击区。短文保留槽宽但隐藏滑块。
- 滑块默认 3 DIP、hover/drag 7 DIP，最小高度 24 DIP；拖动保留抓取偏移，轨道点击翻页。
  鼠标松开、失焦、resize、模式切换或不可交互状态结束拖动；既有 winit Windows mouse capture
  负责窗口外的 move/release，不新增平台 unsafe。
- Source 在逻辑行及折行偏移坐标中定位，仅为末端边界排版最后一屏并按 generation/视口缓存。
  采用该方式避免为精确整篇像素高度引入全文布局或另一套文本布局权威；长短段落混排时滑块比例
  表达逻辑行位置，并非整篇像素高度的精确百分比。Preview 复用既有布局高度。
- 原滚轮与新增滑块汇入同一滚动/同步路径，保留 generation 检查与 source anchor 同步。
  不调用 caret reveal 或文档编辑路径，因此不替换选区、不提交/取消 preedit、不改变文档或 dirty。
- 审查发现：Preview completion 只带 generation，旧 paint 回执可能覆盖新滚动目标。现在额外保留
  原请求位置，将过时位置的回执与显示位置的正常 clamp 区分，并重绘最新目标。

## 自动化证据

定向入口：`cargo test -p stickymd-render -p stickymd-win --locked scrollbar`。

- 七项测试通过：10,000 行首尾跳转（布局缓存少于 40 行）、单行折行/短文/空文档/resize、编辑后
  边界更新与 preedit 保留、测量与往返前后精确像素不变、抓取偏移/窗口外 clamp/翻页、
  100/125/150/200% DPI 窄槽绘制及 Split 控件隔离、worker 回执保留未 clamp 的请求位置。
- 1,100,000-byte Source，100 次分散跳转并绘制，Release 本机样本：median 1.0722 ms，
  p95 1.3519 ms，max 37.5031 ms。入口：
  `cargo test -p stickymd-render --lib --release --locked scrollbar_release_baseline -- --ignored --nocapture --test-threads=1`。
  此数据是 projection + paint 耗时，不能替代真实鼠标到屏幕延迟或完整性能资格化。
- `cargo fmt --all --check`、render/win 严格 Clippy、两 crate 全测试与本地 Release build 通过。

## 本地窗口探针与限制

Ignored evidence 位于 `dist/evidence/scrollbars-20260925-171925/`，包含构建/测试日志、
独立测试程序与 note、native message 探针、截图及 JSON。探针只操作自己启动的进程；用户已有
实例与 note 不作为测试输入。它是一次性合成窗口消息检查，不是 Rust smoke CLI 的正式资格化入口。

第一轮 100% DPI / Light / 900×680 测试验证：Source 和 Preview 可到首尾；松开后移动鼠标不再
滚动；Split 默认同步有效；测试 note 的 SHA-256 不变。截图复查促使左侧轨道避开 Split 同步按钮，
后续结果以追加 Resolution 记录。离屏像素测试确认预留窄槽不会改变未折行文字，且首尾往返后
恢复相同像素；窗口截屏不等同于完整真人视觉验收。

未覆盖：真实物理鼠标手感、触控、多显示器混合 DPI、完整 Light/Dark 截图矩阵、滚动时真实
Microsoft Pinyin/WeChat Input Method 候选窗。对应人工项保持 `NOT TESTED`。
本地 EXE 不继承 v0.1.0 exact artifact、release tag 或资格化收据。

## Resolution — 2026-09-25 最终检查

- 修正 Split 点击区后重新运行：fmt、render/win 严格 Clippy、408 项测试全部通过；18 项 opt-in
  检查在普通 test 中跳过，新增 scrollbar Release baseline 已单独运行。Phase 00 governance 与
  acceptance readiness 均通过；Release 构建通过。
- `desktop-final/desktop-results.json` 与截图验证最终 EXE：Source/Preview 首尾往返、release 后
  停止拖动、Split 开启同步时双侧跟随，关闭同步后 Source 回到顶部而 Preview 保持原底部位置。
  左侧滑块顶端为 62 px，位于同步按钮下方；右侧顶部为 42 px。测试 note SHA-256 保持不变。
- 最终本地 EXE：8,563,712 bytes；SHA-256
  `97F9FF1DA00FE90BD83B04D3C772D3EBE85159E69470873B1A78DFF605F4834B`。
  这是本次维护的本地构建身份，人工/发布边界仍如上。

## Resolution — 2026-09-25 后续 review

后续审查补齐了 Preview 完整视口回执校验、过期布局命中隔离、窄槽 hover 范围、Source 性能入口的
CI 覆盖和模块诊断归档。发现、实现取舍、验证结果及新本地 EXE 身份见
[滚动条与模块 CI 审查](2026-09-25-scrollbar-ci-review.md)；上面的初始证据保留为历史记录。
