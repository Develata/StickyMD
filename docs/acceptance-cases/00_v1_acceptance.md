# 00_v1_acceptance.md - StickyMD v1 验收案例

> 验证合同投影。契约依据见 `docs/plan/`；本阶段只定义，不实现测试。
> 案例 ID 不复用；本表与 `docs/coverage-matrix.md` 保持同步。

---

## AC-001 Portable First Launch

### Preconditions
空的可写目录，含 `StickyMD.exe`，无 `note/`。

### Action
首次启动程序。

### Expected
自动创建 `note/note.md`、`note/config.toml`、`note/images/`、`note/.trash/`；
窗口以默认源码模式、Light、96% 透明度出现并可输入；不写任何程序目录外位置。

### Failure Signals
写入失败却静默继续；fallback 到 AppData/Registry；目录不可写时未提示即创建 UI。

---

## AC-002 Source Editing

### Preconditions
已启动，源码模式。

### Action
输入中英文混排文本、使用常见编辑快捷键、滚动。

### Expected
文本正确插入删除；Caret/selection 明显；仿宋_GB2312 与 Times New Roman 分段正确；
滚动流畅；内容进入自动保存调度。

### Failure Signals
乱码；光标漂移；快捷键无效；按键可见卡顿。

---

## AC-003 Microsoft Pinyin

### Preconditions
微软拼音为活动输入法。

### Action
执行 `07_editor_and_ime.md` 的 13 项验证（连续输入、混输、候选框位置、
selection 中起 composition、composition 内方向键/Backspace、commit 一次撤销、
取消不污染 undo、高 DPI、各视图与透明度、失焦重聚焦、输入不收起）。

### Expected
13 项全部通过。

### Failure Signals
候选框错位；commit 丢字；composition 内容进入 undo；输入期间窗口收起。

---

## AC-004 WeChat IME

### Preconditions
微信输入法为活动输入法。

### Action
同 AC-003 的 13 项。

### Expected
13 项全部通过。

### Failure Signals
同 AC-003。

---

## AC-005 Autosave

### Preconditions
已有输入内容，窗口聚焦。

### Action
停止输入约 650 ms；另测：窗口失焦、隐藏到托盘。

### Expected
debounce 后自动落盘；失焦/隐藏触发立即保存；`note.md` 为完整合法 UTF-8；
连续输入不断重置期间不产生中间坏文件。

### Failure Signals
半截文件；保存失败无提示；保存阻塞 UI。

---

## AC-006 Manual Save

### Preconditions
有未保存修改。

### Action
按 Ctrl+S。

### Expected
立即保存成功；保存失败时显示明确错误且程序保持运行、内容不丢。

### Failure Signals
失败被静默吞掉；Ctrl+S 无效。

---

## AC-007 External Clean Reload

### Preconditions
buffer 干净（无未保存修改）。

### Action
用外部编辑器修改并保存 `note.md`。

### Expected
自动载入新内容；undo 清空；预览更新；无阻塞对话框；程序自身保存事件不被误判为外部修改。

### Failure Signals
内容未更新；undo 未清空；误报冲突。

---

## AC-008 External Dirty Conflict

### Preconditions
buffer 脏（有未保存修改）。

### Action
外部修改 `note.md`。

### Expected
显示 banner“文件已在外部修改 [载入外部] [保留本地]”；autosave 暂停；
两个选择分别按契约执行（载入外部清 undo / 保留本地原子覆盖）。

### Failure Signals
偷偷覆盖任一侧；冲突期间 autosave 仍执行。

---

## AC-009 Undo Redo

### Preconditions
若干编辑操作已完成。

### Action
连续 Ctrl+Z / Ctrl+Y；触发 256 条与 4 MiB 上限。

### Expected
文本 roundtrip 正确；分组规则正确（IME commit/粘贴/Enter 独立成条）；
超限时淘汰最老条目而非崩溃；重启后历史为空。

### Failure Signals
撤销丢失中间状态；超限崩溃；undo 被写入磁盘。

---

## AC-010 Image Paste

### Preconditions
剪贴板含图片（截图、PNG/JPEG/WebP 文件、多文件）。

### Action
Ctrl+V。

### Expected
写入 `images/stickymd-<20hex>.<ext>`；PNG/JPEG/WebP 保留原编码、截图转 PNG；
相同内容去重；写入成功后才插入 Markdown 引用；多图各占一段。

### Failure Signals
写入失败仍插入引用；重复写相同 bytes；引用路径错误。

---

## AC-011 Managed Image Undo

### Preconditions
已粘贴图片并删除其引用。

### Action
Ctrl+Z，再 Ctrl+Y。

### Expected
撤销后图片从 `.trash` 恢复到 `images/` 且引用恢复；重做后再次进入 `.trash`；
预览同步更新。

### Failure Signals
图片丢失；trash 与引用状态不一致。

---

## AC-012 User Image Safety

### Preconditions
`images/` 内有用户手工放入的文件（非 `stickymd-*` 命名）。

### Action
运行 GC 全流程：编辑、删除引用、退出清理、启动清理。

### Expected
用户文件在任何流程后仍然存在且未被移动或重命名；可正常显示与导出。

### Failure Signals
用户文件被删除/移动/重命名。

---

## AC-013 Markdown Preview

### Preconditions
文档含标题、列表、表格、代码块、引用、链接、图片。

### Action
切换到预览/分栏，等待 debounce。

### Expected
各元素正确渲染；表格对齐与滚动正确；代码块 Consolas 无高亮；
链接可点击（http/https/mailto/file）；选择与复制可用；旧预览在新结果前保持可用。

### Failure Signals
布局崩溃；stale 结果覆盖新结果；预览可编辑。

---

## AC-014 Math Delimiters

### Preconditions
文档包含四种 delimiter 的公式、转义 `\$`、code 中的 `$`。

### Action
渲染预览。

### Expected
四种 delimiter 均识别；`$5` 类边界按 Comrak 语义；code 内公式标记不解释；
转义按 parser 结果。

### Failure Signals
自创识别规则；code 内误识别；`$5` 误判。

---

## AC-015 Math Error

### Preconditions
文档含非法公式与超长公式（>64 KiB）。

### Action
渲染预览。

### Expected
不 panic；显示原始文本 + 轻微错误边框 + 图标；hover 有简化错误信息；
`note.md` 不被修改。

### Failure Signals
崩溃；源文件被改动；错误无提示。

---

## AC-016 Raw HTML Safety

### Preconditions
文档含 `<script>`、`<style>`、inline HTML、block HTML。

### Action
渲染预览。

### Expected
全部按原文以 code 风格呈现；无脚本执行、无 DOM、无样式应用；用户原文完整保留。

### Failure Signals
任何执行/解释行为；原文丢失。

---

## AC-017 Remote Image No Network

### Preconditions
文档含 `![alt](https://example.com/a.png)`；可监视网络。

### Action
渲染预览并导出。

### Expected
零网络请求；预览显示 alt + 可点击链接；导出保留原 URL。

### Failure Signals
任何 HTTP 请求；下载缓存行为。

---

## AC-018 Export

### Preconditions
文档引用本地 managed 图片、用户图片与 remote 图片。

### Action
Ctrl+Shift+S 导出到目标目录。

### Expected
生成 `my-note.md` + `my-note-assets/`；仅复制实际引用的本地图片；
引用重写为 `my-note-assets/...`；raw HTML 保留；不导出 config/.trash/未引用图片；
工作文档不变。

### Failure Signals
切换了 active document；复制了未引用图片；路径未重写。

---

## AC-019 Left Dock

### Preconditions
浮动窗口靠近屏幕左边缘（≤12 DIP）。

### Action
松手；随后 hover 感应条；失焦等待。

### Expected
吸附并向左缩入，留 3 DIP 感应条（高度=窗高）；hover 100 ms 展开；
失焦 700 ms 收起；拖离 >16 DIP 恢复浮动。

### Failure Signals
感应条缺失/尺寸错误；计时不符；拖离后仍 dock。

---

## AC-020 Right Dock

### Preconditions
同 AC-019，换右边缘。

### Action
同上。

### Expected
向右缩入，其余行为同 AC-019。

### Failure Signals
同 AC-019。

---

## AC-021 Top Dock

### Preconditions
同上，换上边缘。

### Action
同上。

### Expected
向上缩入，感应条宽度=窗宽；其余行为一致。

### Failure Signals
同 AC-019。

---

## AC-022 Input Focus Guard

### Preconditions
窗口处于 dock 展开态，正在输入或 IME composition 中。

### Action
等待任意自动收起计时器到期；鼠标短暂移出。

### Expected
不收起。Esc 与手动收起按钮仍可收起。冲突/恢复提示存在时同样不收起。

### Failure Signals
输入中收起；composition 被中断；提示被收起吞掉。

---

## AC-023 Tray Lifecycle

### Preconditions
程序运行中。

### Action
点击窗口关闭按钮；托盘“显示/隐藏”“置顶”“退出”。

### Expected
关闭=隐藏到托盘且进程存活；显示=恢复先前状态（含 dock）；
退出=保存→资产事务→GC→配置→释放后结束；保存失败时报错并保持运行。

### Failure Signals
关闭即退出；退出丢数据；托盘出现多余菜单项。

---

## AC-024 Opacity

### Preconditions
打开透明度控件。

### Action
拖动 slider、输入 65/105/96.5、松开/Enter/失焦。

### Expected
70–100 实时预览；越界 clamp（65→70，105→100）；非整数不提交；
仅在松开/Enter/失焦写配置；整窗（含文字公式图片控件阴影）统一透明。

### Failure Signals
拖动过程写盘；部分元素不透明；越界未 clamp。

---

## AC-025 Theme

### Preconditions
主题控件可用。

### Action
切换 Light/System/Dark；System 模式下改变 Windows 主题。

### Expected
默认 Light；切换立即生效；System 跟随系统且运行时立即响应；选择写入配置。

### Failure Signals
System 不跟随；重启后主题丢失。

---

## AC-026 Same Directory Single Instance

### Preconditions
目录 A 已有运行实例。

### Action
在目录 A 再次启动 `StickyMD.exe`。

### Expected
第一实例被唤醒并激活；第二实例立即退出；不出现双窗口、不出现文件争用。

### Failure Signals
双实例同时编辑同一 note.md。

---

## AC-027 Different Directory Multi Instance

### Preconditions
目录 A、B 各有一份程序。

### Action
同时启动两者并分别编辑。

### Expected
互不干扰：各自编辑各自的 `note/note.md`；托盘与窗口独立。

### Failure Signals
互相唤醒；数据串写。

---

## AC-028 Monitor Disconnect

### Preconditions
窗口 dock/浮动于外接显示器。

### Action
运行中拔掉该显示器；随后重连；另测 sleep/resume 与主屏切换。

### Expected
窗口立即恢复到主显示器且完全可见；原尺寸尽量保留；原 dock 状态在主屏同边缘恢复；
不出现不可见坐标。

### Failure Signals
窗口消失/不可达；恢复后越界。

---

## AC-029 Mixed DPI

### Preconditions
双显示器不同缩放（如 100% + 150%），含负坐标布局。

### Action
在两屏间拖动窗口、dock、输入中文、渲染公式。

### Expected
scale 正确重算；3 DIP 感应条按显示器缩放；IME 候选框位置正确；
公式清晰（cache key 含 DPI）；无模糊/错位。

### Failure Signals
尺寸错乱；候选框错位；公式栅格化模糊或错位。

---

## AC-030 Crash Recovery

### Preconditions
编辑后强制结束进程（模拟崩溃），留下有效 `note.md.tmp`。

### Action
重新启动。

### Expected
检测到 temp：提示“发现未完成保存的内容 [恢复临时内容] [使用当前文件]”；
选择前不覆盖任何文件、autosave 暂停；两种选择均按契约执行；
无效 UTF-8 的 temp 不进入恢复候选。

### Failure Signals
静默覆盖；提示缺失；坏 temp 被恢复。
