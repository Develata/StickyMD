# Phase 1C Spike — Text / IME（cosmic-text 0.19 投影模型 + winit IME + Win32 剪贴板）

> 实验性代码。本目录不属于生产 workspace，可随时删除。
> plan_ref: docs/plan/03_text_model.md ; docs/plan/09_windows_shell.md

## 1. 验证目标（来自 Phase 1 任务 1C）

- 规范 `String` 缓冲是唯一权威；cosmic-text `Buffer` 只是**投影**，每次变更后重建。
- winit IME 事件（`Ime::Enabled/Disabled/Preedit/Commit`）驱动编辑；`set_ime_allowed` +
  `set_ime_cursor_area` 已接线。
- 按脚本（script）划分字体 run：CJK run 与 Latin run 使用不同字体族。
- Win32 剪贴板文本 adapter（薄封装，OpenClipboard/GlobalAlloc/SetClipboardData）。
- 简单 undo 栈（快照式）。

## 2. 环境

| 项 | 值 |
| --- | --- |
| OS | Windows 11 x64 |
| 工具链 | Rust 1.97.1，MSVC |
| 依赖 | winit 0.30.13 / softbuffer 0.4.8 / cosmic-text 0.19.0（默认 swash 特性）/ raw-window-handle 0.6.2 / windows 0.62.2 |

## 3. 结果矩阵（如实标注）

| 项 | 判定 | 证据 |
| --- | --- | --- |
| canonical String → cosmic-text Buffer 投影（重建式） | **PASS** | `SPIKE_TEXT_SELFTEST=1`：rebuilds=3，文本经 `set_rich_text` 重建后渲染 |
| 编辑管线（插入/退格/换行/undo 快照） | **PASS** | SELFTEST：insert=PASS（+7 字节 "你好A"），undo=PASS（逐字节恢复） |
| 按脚本字体 run（CJK vs Latin） | **PASS** | `script_runs()` 分段 + 每段独立 `Attrs.family`；字体探测日志见 §4 |
| 混排渲染管线（CJK+Latin+换行+自动折行 Wrap::Word） | **PASS（管线级）** | 初始 3 行混排文本成功 shaping+光栅化+present；**字形视觉正确性需人工目检** |
| Win32 剪贴板 adapter（set+get，CF_UNICODETEXT） | **PASS** | 启动自检 `clipboard self-check: set+get => PASS`（含保存/恢复用户剪贴板） |
| IME 事件接线（Preedit/Commit → 编辑 + 投影重建 + preedit 下划线带） | **代码完成** | 事件处理、日志、`set_ime_cursor_area` 均已实现 |
| **真实 IME 输入（微软拼音 / 微信输入法 组词、候选、提交）** | **NOT TESTED（人工）** | Agent 无法执行交互式 IME 输入；禁止伪造 PASS |
| `set_ime_cursor_area` 在真实 IME 下的候选窗定位 | **NOT TESTED（人工）** | 同上 |

## 4. 关键事实与发现

1. **cosmic-text 0.19 API 记录**（与旧版差异大，生产开发须按此）：
   - `Buffer::set_text/set_rich_text` **不再**接收 `&mut FontSystem`；
     `set_size(width_opt, height_opt)` 同理。
   - `Buffer::draw(&mut FontSystem, &mut SwashCache, Color, FnMut(i32,i32,u32,u32,Color))`
     走 `LegacyRenderer`，字形按**单像素回调**交付（spike 可接受；生产需实现
     `Renderer` trait + `SwashCache::with_pixels` 批量混合）。
   - `Attrs::new()` 无参 + `.family(Family::Name(&str))` 构造。
   - fontdb 0.23：`Database::query(&Query{ families, weight, stretch, style })`。
2. **字体探测结果（本机）**：
   - `仿宋_GB2312` **未安装**；候选回退链命中 `FangSong`（found=true）。
   - `Times New Roman` found=true。
   - 结论：按字体族名精确匹配不可靠，生产必须保留回退链 + 启动探测（已验证该机制可行）。
3. **投影模型验证**：canonical String 变更 → `script_runs` 分段 → `set_rich_text` 重建 →
   `shape_until_scroll` → 渲染。caret 通过 `layout_runs()` 按 (line_i, byte index) 计算。
4. **winit 0.30 细节**：`KeyEvent.text: Option<SmolStr>` 提供非 IME 直输文本；
   `WindowEvent::ModifiersChanged(Modifiers)`（注意是 `Modifiers` 包装，需 `.state()`）；
   方法名是 `control_key()` 不是 `ctrl_key()`。
5. **局限（如实记录）**：
   - undo 为整串快照（spike 简化）；生产按 Phase 2 设计用 TextDelta。
   - 光标移动仅实现 Left/Right；Up/Down 未实现（不影响 IME 结论）。
   - preedit 光标位置（`(usize,usize)` 范围）未精细渲染，仅显示下划线带。
   - 合成键盘注入（PostMessage/SendInput）无法驱动 winit 输入路径，
     故编辑管线改用进程内 SELFTEST 验证；**真实键入仍属人工验证**。

## 5. 结论

判定：**CONDITIONAL PASS**。文本/IME 所需的工程机制（投影模型、脚本字体 run、
IME 事件接线、剪贴板、caret/IME 区域）全部打通并可自动化验证的部分全部 PASS；
**交互式 IME 组词行为（微软拼音/微信输入法）与字形视觉正确性保留给 USER 人工验证，
此处如实标记 NOT TESTED**。

## 6. 复现

```powershell
cd experiments/phase-01/text
cargo run --release                      # 打开窗口，人工测试 IME/按键/剪贴板
$env:SPIKE_TEXT_SELFTEST=1; cargo run --release   # 无交互自检（编辑管线+剪贴板+字体探测）
# 窗口内: 键入测试 IME; Ctrl+C/V 剪贴板; Ctrl+Z undo; Esc 退出打印统计
```
