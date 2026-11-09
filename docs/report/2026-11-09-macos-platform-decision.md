# macOS 平台决定

日期：2026-11-09。基线：`main` `32b1872`（含 Phase 15 E，`flow/note_startup.rs` 存在）。状态：**等待 USER 批准**；批准前不建宿主、不改 `docs/plan`。

环境：MacBook Pro，Apple M6，macOS 27.2（`26B7124`），APFS，单块内建屏（1512×982 pt，scale 2），Rust 1.97.1。

## 结论

1. **可以建 macOS 宿主，不需要 WebView、async runtime 或换框架。** 窗口定位、隐藏、唤醒、置顶、透明度、菜单栏图标都在本机跑通了读回。
2. **基线没有全绿：** 2 个失败加 1 个不终止，同一根因，是 macOS 自带的一个位图字体让 cosmic-text 算出 `inf`。这也是产品缺陷：代码块里出现中文就会触发。修法见“共享层改动”S1。
3. **建议的形态：** macOS 27+、仅 arm64、`StickyMD.app` 放在一个普通目录里，**目录是便签身份**，`note/` 与 `.app` 同级，不进 bundle。本机 ad-hoc 签名，暂不公开分发。
4. **存储用普通 `rename` 发布，语义与 `05` 已接受的 TOCTOU 相同，要求卷支持 `RENAME_EXCL`（APFS）。** 比 Windows 弱的有三处，需要逐项批准：持有旧句柄的写者的写入会丢失；保留 invalid canonical 变成“移动后核对”；受管图片不做自动移动和物理删除。
5. **光标闪烁必须走原生图层：** 整窗提交每次闪烁 5.66 MB、CPU 1.94%，约为空闲硬门 0.1% 的 19 倍；`CALayer` 开关 0.037%。
6. **两项风险靠人工定案：** 未聚焦时 3 pt 感应条能否收到悬停，输入法（`replacementRange` 被 winit 丢弃）。探针已备好，见“人工项”。
7. **winit 没有接系统退出：** 注销和关机不经过保存屏障，需要宿主补接线（S4）。

## 证据等级

- **已验证**：在本机运行得到，原始输出在 [`experiments/macos-probes/results/`](../../experiments/macos-probes/results/)。
- **读源码**：读锁定版本 crate 的源码得到，未运行。
- **推断**：由上述或 Apple 文档推出，未实测。
- **未验证**：需要人工或另一环境。

## 基线

```bash
cargo test --locked --no-fail-fast -p stickymd-core -p stickymd-render -p stickymd-app -p stickymd-desktop
```

退出码 101，未通过。完整输出：`results/baseline-macos.log`。

| 目标 | 通过 | 失败 | 忽略 |
| --- | --- | --- | --- |
| `stickymd-app` | 326 | 0 | 3 |
| `stickymd-core`（lib + property） | 57 | 0 | 1 |
| `stickymd-desktop` | 251 | 0 | 2 |
| `stickymd-render` lib | 186 | **2** | 17 |
| `phase5_semantics` / `phase6_math` / `qualification_images` / `table_math_pipes` | 5 / 6 / 1 / 7 | 0 | 0 |
| `rendering_stress` | 5 | **1 个不终止** | 0 |

失败项：`long_code_scrolls_without_wrapping_or_changing_copy_and_hit_ranges`、`horizontal_clip_matches_full_draw_for_bidi_emoji_italic_and_combining_ink`（`glyph_cache.rs:100` 加法溢出）。不终止项：`stress_fixture_builds_every_formula_through_native_preview`，运行超过 10 分钟后由我终止（SIGTERM），`sample` 显示主线程仍在绘制（`results/rendering_stress-sample-excerpt.txt`）。

**根因（已验证）。** `/System/Library/Fonts/Supplemental/NISC18030.ttf`（`GB18030 Bitmap`）是纯位图字体：表目录用 `bhed` 没有 `head`。它被标为等宽，于是 `Family::Monospace` 的中文回退选中它，字形宽度为 `inf`、偏移为 `NaN`，之后同一行所有字形的 x 都是 `inf`。debug 下 `SubpixelBin::new` 溢出 panic；压力测试里滚动上限是 `inf`，`x += 400` 永远追不上。

**确认方式（已验证）。** 临时用打了补丁的 cosmic-text（只删这个 face，`results/diagnostic-cosmic-text.diff`，放在 scratchpad，经 `--config patch` 注入，仓库未改，所以这次没有 `--locked`）跑 `stickymd-render` 全部目标：188 + 5 + 6 + 1 + 6 + 7 全过，退出码 0（`results/baseline-diagnostic-without-bitmap-face.log`）。这是诊断，不算基线通过。971 个 face 中只有这一个没有 `head`。

## 决定项

### 1. 版本、架构、身份、签名

| 项 | 建议 | 依据 |
| --- | --- | --- |
| 最低版本 | macOS 27.0（`LSMinimumSystemVersion`、部署目标同值） | 只声明测过的版本；有环境再放宽。API 下限都低得多（`renameatx_np` 10.12，`O_NOFOLLOW_ANY` 11，读 SDK 头文件） |
| 架构 | `aarch64-apple-darwin`，不打 x86_64 或 universal 包 | USER 决定 |
| 应用身份 | `StickyMD.app`，`CFBundleIdentifier = io.github.develata.stickymd`，`LSUIElement = true` | 所有副本同一 ID：另一路径的副本仍会独立启动（已验证） |
| 沙盒 | 不启用 | 沙盒下无法访问 bundle 旁的 `note/`（推断） |
| 签名 | 本机 `codesign --sign -` | 本机 0 个有效签名身份（已验证） |
| 分发 | 第一阶段只本机构建，不发公开包 | ad-hoc 包 `spctl` 判定 rejected（已验证）。公开 ZIP 需要 Developer ID 与公证（付费账号），届时单独决定 |

### 2. 便签目录与实例身份

| 方案 | 结果 |
| --- | --- |
| **A：外层目录是程序目录**（建议） | `<dir>/StickyMD.app` + `<dir>/note/`。复制整个目录得到第二张便签，签名不受影响 |
| B：裸可执行文件 | 与 Windows 一致，但 Finder 双击会打开终端（推断），也没有 `LSUIElement` |
| C：数据放进 bundle | 写入 `Contents/MacOS/note/` 后 `codesign --verify` 报 sealed resource 无效（已验证），否决 |
| D：`~/Library/Application Support` | `02` 明确禁止，且破坏“目录即身份”，否决 |

**解析算法。** `current_exe` → `canonicalize` → 路径形如 `<X>/*.app/Contents/MacOS/<exe>` 时程序目录为 `<X>`，否则（开发时 `cargo run`）取可执行文件的父目录。`canonicalize` 对大小写写错的路径返回磁盘上的真实大小写（已验证）。

**App Translocation（未验证）。** ad-hoc 包会被 Gatekeeper 拒绝，本机无法走到随机化路径。设计上不依赖检测：随机化路径是只读挂载（推断），可写检查失败就显示现有提示后退出，不回退。路径含 `/AppTranslocation/` 时追加一句“请先用访达把 StickyMD.app 移到别处”，只改提示文字。

**受保护目录（推断）。** 放在桌面、文稿、下载时系统会弹授权；拒绝则可写检查失败。ad-hoc 身份随每次构建变化，开发时会反复询问，建议开发目录放在这三处之外。

**实例身份。** 不用路径字符串，用程序目录的 `(st_dev, st_ino)`，大小写与 Unicode 归一化问题随之消失（本卷大小写不敏感、NFC/NFD 互通，已验证）。

- **锁：** 对程序目录的 fd 取 `flock(LOCK_EX | LOCK_NB)`。第二个描述符得到 `EWOULDBLOCK`，首个关闭后可取（已验证）。不创建任何文件，满足“第二实例在 durable 文件创建前退出”；进程崩溃时内核释放（推断）。
- **唤醒：** `<用户临时目录>/stickymd-<16 位十六进制>.sock`，由 `(dev, ino)` 的 SHA-256 取前 64 位。用户临时目录取 `confstr(_CS_DARWIN_USER_TEMP_DIR)` 而不是 `$TMPDIR`，后者可被调用方覆盖，同目录的两个进程会找不到彼此。本机该目录 49 字节，完整路径 79 字节，小于 `sun_path` 的 104（已验证）。超长时视为唤醒不可用。
- **时序：** 取锁 → 持锁者 `unlink` 残留 socket → `bind` + `listen`。第二实例取锁失败后 `connect`，失败按 50/150/300 ms 重试，覆盖“已持锁但尚未 listen”；最终失败仍退出，不启动重复会话（`05` Failure Paths）。
- **系统路径：** 同一 bundle 第二次 `open` **不会启动新进程**，而是投递 reopen Apple Event（已验证）。winit 不上报它，宿主要在 `NSAppleEventManager` 注册 `kAEReopenApplication`，转成 `HostSignal::ShowRequested`。事件送达已验证；隐藏状态下的“reopen → 显示 → 取得焦点”端到端未验证，列入 M2 验收。`open -n` 与直接执行会产生新进程（已验证），走 flock。

### 3. 存储保证

`03` 规则 5 的落实：每个 `NoteStorage` 调用是宿主内的一个完整事务，宿主自己观察、裁决、执行，共享层只给预期指纹。宿主在启动时持有 note 目录 fd，全部用 `*at` 调用；目录运行中被移动，操作仍落在原目录（已验证）。

| 操作 | Windows | macOS 建议 | 与 Windows 比 |
| --- | --- | --- | --- |
| 写 temp | `FlushFileBuffers` | `openat(O_NOFOLLOW)`，写入，`F_FULLFSYNC` | 对等。20 KiB 中位 3.9 ms（`fsync` 0.05 ms 不保证落盘），在 I/O worker |
| 发布（已存在） | guard 后 `ReplaceFileW` | guard（`O_NOFOLLOW` 读并比对指纹）后 `renameat`，再对目录 `F_FULLFSYNC`。`ForceOverwrite` 跳过 guard | 检查与替换之间的 TOCTOU 与 `05` 已声明的相同 |
| 发布（首次） | `MoveFileExW` 不覆盖 | `RENAME_EXCL`，`EEXIST` 转 Conflict | 对等（已验证） |
| 目录同步失败 | 无对应步骤 | 名称已切换但持久性未确认：按 `05` 报 `SaveFailed`，文本保持 dirty，退出被取消。adapter 记住这次已发布的指纹，下一次 guard 把它也视为预期，重试时不会与自己冲突 | 期间 watcher 可能把这次发布报成外部修改，进入冲突提示；内容不丢。路径罕见，M1 用故障注入验收（推断） |
| 外部写者持有旧文件 | 共享模式使替换失败 | **无法阻止，其写入丢失且不报错**（已验证）。`O_EXLOCK` 是建议锁，不拦写者、改名、删除（已验证） | **更弱**，见下 |
| 符号链接 | 拒绝 reparse point | `O_NOFOLLOW`（`ELOOP`），路径含链接时 `O_NOFOLLOW_ANY` / `RENAME_NOFOLLOW_ANY`（已验证）。另加 `O_NONBLOCK` 防 FIFO 阻塞、`fstat` 要求普通文件（推断） | 对等 |
| 校验后移动（保留 invalid canonical） | 同一句柄：排斥写者、按句柄改名 | `O_NOFOLLOW` 打开并哈希，`RENAME_EXCL` 改名（改名不毁任何内容），再比对 `(dev, ino)` 并重新哈希。不符时用 `RENAME_EXCL` 移回，成功报 `Changed`；移回失败（原名已被占）报失败并指明文件所在，拒绝本次恢复 | **更弱**：竞争时会短暂移动 `05` 第 5 条要求不移动的文件，但内容不丢 |
| 受管图片的移动与删除 | 同一句柄校验后按句柄改名、删除 | **v1 不做自动移动，也不做物理删除。** 未引用的图片留在 `images/`，`sync` 报 `skipped_untrusted` 与 `physical_delete_deferred`。只保留 `.trash/` 到 `images/` 的恢复，移动后校验 | **更弱**：被删除引用的图片不会清理，需手动处理 |
| 观察身份 | 卷序列号 + 文件 ID | `(st_dev, st_ino, size, mtime_ns, ctime_ns)`；inode 跨改名不变，替换后变化，fd 可读旧内容（已验证） | 对等 |

**三处弱点要分别批准。**

1. **持有旧句柄的写者：** `05` 已声明“v1 不通过长期独占文件锁破坏外部编辑能力”，OCC 本来就是主要防线。macOS 没有共享模式的对等物，无法靠实现补足，应写入 `05`。
2. **保留 canonical：** 这不满足 `05` 第 5 条与 `PreserveOutcome::Changed` 现在的语义（变化了的内容留在原处），批准后要一起修改合同。文件在哈希之后被替换或原地改写，就会被移动；移动后、移回前崩溃，或移回失败，文件会一直留在 `note.invalid-*`，内容不丢但位置变了。替代是在 macOS 上拒绝这类恢复，让用户手动移走文件。建议接受移动后核对，因为改名不毁内容，且只发生在 canonical 已无效、用户主动选择恢复时。
3. **受管图片：** `08` 要求校验与改名、删除绑定同一句柄，POSIX 做不到。`RENAME_EXCL` 只保护目标，不保护源：校验后外部把源路径换成用户文件，移动的就是用户文件；`unlinkat` 则可能直接删掉它。`08` 允许不能证明时降级，这里长期降级。备选是“移动后核对再移回”，代价同第 2 条，等确有需要再议。

**否决的方案：`RENAME_SWAP` 发布后核对被换下的内容，不符再换回。** 初稿曾建议它，独立审查指出两个问题，核实成立：

- 换回不是原子的。换下外部版本 E1 之后，外部又写入 E2，换回会把 E2 放到 temp，随后被清理，**外部新版本丢失**。探针（`fs_semantics` 的 swap-then-verify）只串行执行，不能证明并发安全。
- swap 后 temp 里是旧内容，破坏 `05` 恢复规则对 temp 角色的假设：0 字节的旧内容按第 2 条被静默删除，非空的会被当成“恢复临时内容”。

普通 `rename` 没有这两个问题，崩溃现场与 Windows 相同（temp 是新内容），恢复规则不用改。代价是失去对检查与替换之间那次外部替换的发现能力，这正是 `05` 已接受的窗口。

**卷要求。** `RENAME_EXCL` 在 exFAT、FAT32 返回 `ENOTSUP`，在 APFS、HFS+ 可用（已验证）。不用 `rename` 回退，否则“目标突然出现时不覆盖”无法保证。可写检查增加一次真实的 `RENAME_EXCL`，失败时提示“当前卷不支持安全保存，请将程序移到 APFS 卷”并退出。U 盘（exFAT）因此不可用；验收目标只有 APFS，iCloud 同步目录标 `NOT TESTED`。

**首次创建的换行：** 建议 LF，由 `plan_note_startup` 已有的参数传入；“数量相等用 CRLF”是共享规则，不动。`note.invalid-*` 的命名沿用 Windows。

**监视（已验证）。** `notify` 工作区配置关了默认特性，宿主要启用 `macos_fsevent`。首事件 10–12 ms；非递归监听确实不报 `images/`；自身保存产生约 10 个事件，由既有的 150 ms 合并与指纹比对吸收。目录被移走后只有一个 `Modify(Name)`，随后再无事件，宿主应把它转成 `WatchFailed`（degraded，OCC 仍保护保存）。

### 4. 窗口行为

| 能力 | 实现 | 结果 |
| --- | --- | --- |
| 无 Dock 图标 | `ActivationPolicy::Accessory` + `LSUIElement` | `activationPolicy = 1`（已验证）。这是 Tool Window 的对应物 |
| 定位与读回 | winit `set_outer_position` | 读回精确相等（已验证），定位回执容差沿用 0 |
| 工作区 | `NSScreen.visibleFrame` | 顶部 33 pt，Dock 90 pt（已验证）。winit 不给工作区，宿主自取 |
| 离屏收起 | 同上 | 左、右、下可留 3 pt（已验证）。**顶部被 AppKit 夹到菜单栏下方** |
| 顶部收起 | 在窗口类上补 `constrainFrameRect:toScreen:` | 补上后顶部可离屏（已验证）。备选是 macOS 上取消 Top dock，但那要改共享状态机。建议补方法，放在宿主 adapter |
| 隐藏 / 显示 | `set_visible` | `isVisible` 读回一致，`is_minimized` 可读（已验证） |
| 唤醒 | `focus_window` | Finder 在前台时调用，1 s 内成为 active + key（已验证）。winit 用的 `activateIgnoringOtherApps` 已弃用但仍生效 |
| 置顶 | `WindowLevel::AlwaysOnTop` | `level = 3`，可读回（已验证） |
| Spaces 与全屏 | `canJoinAllSpaces \| fullScreenAuxiliary` | 位可写可读（已验证），实际效果未验证。建议便签在所有 Space 可见 |
| 透明度 | `NSWindow.setAlphaValue`（宿主实现 `set_opacity`） | 读回 0.4（已验证） |
| 模态 | handler 内 `runModal` | 不 panic；模态期间 worker 事件送达 0 个，结束后 6 个全部送达（已验证，用 NSAlert） |
| 圆角、阴影 | 根图层 `cornerRadius` | 未验证 |
| 多屏与混合 DPI | — | `NOT TESTED`（本机单屏） |

**菜单栏图标。** `tray-icon` 0.24.2 在 `NewEvents(Init)` 创建成功，事件循环运行后 rect 有效（已验证）。在循环启动前创建，本机同样成功，但其文档要求事件循环已运行，按文档放在 `Init`。

**不可用时的行为。**

- 创建失败：走现有无托盘路径（`09` Failure Paths），关闭即安全保存并退出；策略切回 `Regular`（`restore_ordinary_entry`），窗口回到 Dock。
- 创建成功但图标被挤掉或被用户隐藏：宿主无法可靠发现（推断）。隐藏仍然允许，因为再次打开 `.app` 能发出 reopen（事件送达已验证）。此时托盘这条路径不存在，退出靠下面的 Cmd+Q。

**退出入口（与 Windows 的差异，需批准）。** `09` 规定真正退出只来自托盘，但图标不可见时用户就只能强杀进程。建议 macOS 上窗口聚焦时 Cmd+Q 走同一条退出屏障。

**系统终止。** 不建默认菜单并不能阻止系统退出：注销、关机仍会终止进程。winit 只实现了 `applicationWillTerminate:`，没有 `applicationShouldTerminate:`（读源码 `app_state.rs:69`），未保存内容拦不住退出，违反 `05` 的 session shutdown 立即保存点。建议宿主用与顶部夹取相同的方式给 winit 的 delegate 补 `applicationShouldTerminate:`：返回 `NSTerminateLater`，投递退出信号走共享屏障，成功回 `YES`，保存失败回 `NO` 并保持运行。需要 S4。

有模态窗口时要先结束它：winit 在 handler 占用期间不派发用户事件（读源码 `app_state.rs:378`，探针也测到 0 个），信号进不了共享屏障。所以先 `abortModal`（导出按取消处理），再走屏障。系统对注销、关机的答复有约两分钟期限（Apple 文档），屏障超时回 `NO`。这一段都未实测，M2 验收覆盖：普通退出、保存失败、导出对话框打开时退出、`osascript` 退出、真实注销。

### 5. 输入

**快捷键。** 共享层把修饰键写死成 Ctrl（`shell/input.rs:136`、`pointer.rs:167`、`opacity_field.rs:110`），`KeyZ` 不区分 Shift（`input.rs:356`），查找/替换写死 F/H（`input.rs:163`），需要 S2。Cmd 键事件能否到达 winit 未验证，见人工项。

| 动作 | Windows | macOS 建议 |
| --- | --- | --- |
| 保存 / 导出 | Ctrl+S / Ctrl+Shift+S | Cmd+S / Cmd+Shift+S |
| 复制、剪切、粘贴、全选 | Ctrl+C/X/V/A | Cmd+C/X/V/A |
| 撤销 / 重做 | Ctrl+Z / Ctrl+Y | Cmd+Z / Cmd+Shift+Z |
| 查找 / 替换 | Ctrl+F / Ctrl+H | Cmd+F / Cmd+Option+F（Cmd+H 在 macOS 上约定为隐藏） |
| 视图 / 缩放 | Ctrl+1/2/3、Ctrl+=/-/0 | Cmd 同键；Cmd+滚轮 |
| 行首行尾 / 文首文末 | Home/End、Ctrl+Home/End | Cmd+←/→、Cmd+↑/↓（Home/End 保留） |
| 退出 | 仅托盘 | 托盘，或聚焦时 Cmd+Q（第 4 节） |
| Insert 别名 | Ctrl/Shift+Insert | 无（Mac 键盘没有） |

Ctrl 组合键在 macOS 上不绑定，v1 不做 Emacs 式导航。Windows 现在也没有按词移动（`input.rs:219`），Option+←/→ 是新功能，首版不做，单独决定。

**默认字体**（本机 fontdb 实查）：

| 用途 | Windows | macOS 建议 | 本机 face 数 |
| --- | --- | --- | --- |
| 中文 | 仿宋_GB2312 系 | Songti SC → STSong → PingFang SC | 4 / 1 / 6 |
| 西文 | Times New Roman | Times New Roman → Georgia | 4 / 4 |
| 代码 | Consolas | Menlo | 4 |
| 数学 | RaTeX 字体 | 不变 | — |

仿宋在本机为 0 个 face（STFangsong、FangSong、仿宋都没有），所以取宋体。`FontSystem::new` 中位约 6–10 ms（971 个 face，`results/glyph_inf.txt` 这次 5.9 ms）。

**输入法验收矩阵（全部未验证）。** 一级：系统简体拼音、微信输入法 Mac 版。

| 用例 | 预期 |
| --- | --- |
| 组字、提交、取消（Esc）、组字中退格 | preedit 不进文档，提交只产生一条撤销 |
| 候选窗位置（编辑区、Search、窗口贴边时） | 贴近光标 |
| 组字期间失焦、切换视图、自动收起 | 遵守现有守卫 |
| 中英切换、全半角 | 正确 |
| 表情面板（Ctrl+Cmd+Space）、听写、文本替换 | 记录实际结果 |
| 英文长按选重音 | 不产生重复字符 |

**`replacementRange`（读源码）。** winit 0.30.13 的 `insertText:replacementRange:` 与 `setMarkedText:selectedRange:replacementRange:` 都丢弃该参数（`view.rs:274`、`393`，带 TODO）；`selectedRange` 恒为 NotFound，`attributedSubstringForProposedRange` 恒为 `None`，`characterIndexForPoint` 恒为 0。输入法读不到上下文，也改不了已提交的文本。候选窗位置靠 `set_ime_cursor_area`，路径是通的。

| 方案 | 代价 |
| --- | --- |
| **A：v1 接受，受影响用例记为不支持；关闭长按（`ApplePressAndHoldEnabled = NO`，只作用本进程）避免重复字符**（建议） | 无代码侵入；失去重音弹窗 |
| B：宿主在运行时替换 `WinitView` 的这两个方法，合成带范围的 intent | 依赖 winit 私有类，升级易碎；需要共享层接收范围 intent |
| C：维护 winit 分支 | 长期维护成本 |

一级输入法基本组字失败时停止实现，按 `AGENTS.md` 写 RISK 报告，再转 B。

**剪贴板。** `arboard` 3.6.1 读图只取 `NSPasteboardTypeTIFF`（`osx.rs:218-224`，读源码），违反 `08` 的“保留原编码”。宿主直接读 `NSPasteboard`，不用 arboard。优先级按 `08` 映射：

```text
public.file-url（图片文件列表）→ public.png → public.jpeg → org.webmproject.webp → public.tiff（转 PNG）→ public.utf8-plain-text
```

未实测。验收沿用 `08` 的“逐组写明发布格式与预期读取格式”。

**保存对话框。** `NSSavePanel.runModal`，主线程阻塞，与 Windows 语义相同，经 `objc2-app-kit`，不加 `rfd`。模态内不派发事件见第 4 节；`NSSavePanel` 本身未实测。

### 6. 呈现成本（已验证）

描述核实无误：`buffer_mut` 每次 `vec![0; w*h]`，`age()` 恒为 0，`present_with_damage` 忽略 damage（`cg.rs:261-296、364`）。release，scale 2：

| 窗口 | 物理像素 | 每帧堆分配 | 分配次数 | `buffer_mut` p50 | `present` p50 / p95 |
| --- | --- | --- | --- | --- | --- |
| 520×680 pt（默认） | 1040×1360 | **5,657,634 B** | 1.1 | 0.03 ms | 2.40 / 2.93 ms |
| 900×680 pt（Split） | 1800×1360 | **9,792,034 B** | 1.1 | 0.06 ms | 3.91 / 4.61 ms |

`calloc` 懒清零，所以 `buffer_mut` 本身便宜，代价在页故障和提交。240 帧连续提交时占用涨到 76.5 MiB。

光标闪烁（`caret_cost`，事件循环定时器驱动，530 ms，每组 30 s，开始前静置 10 s）：

| 场景 | 本进程 CPU | Rust 堆/次 | 占用 |
| --- | --- | --- | --- |
| 空闲 | 0.020% | — | 27.6 MiB |
| 整窗提交 | **1.938%** | 5,657,600 B | 27.6 → 33.2 MiB |
| `CALayer` 开关（`setHidden`） | **0.037%** | 0 | 33.2 MiB |

57 次开关中 56 次在 `presentationLayer` 上观察到已提交。限制：堆数字只计 Rust 分配器，AppKit 和 Core Animation 的分配不在内；WindowServer 的合成开销不在本进程 CPU 内。

**对策。**

1. 实现现有的 `Host::caret_overlay`：`differenceBlendMode` 的 sublayer。闪烁不产生帧，共享层不用改。0.037% 在 ≤0.05% 目标内，但余量小，真实宿主要复测。反色的视觉效果未验证（截图权限被拒）；不成立时改纯色图层。
2. 普通帧继续用 softbuffer：2.5 ms 在输入延迟目标内。
3. 不提前自写 IOSurface 后端。**触发条件**：真实宿主越过 p95 输入延迟或内存硬门时再决定。

### 7. 资源与性能门

| 项 | 建议 |
| --- | --- |
| 内存口径 | `phys_footprint`（活动监视器的“内存”），release，scale 2，等 30 s，≥5 次取中位和最大 |
| 地板 | 探针（窗口，无文档）27.6 MiB；加菜单栏图标 31.3 MiB；闪烁 30 s 后 33.2 MiB；连续提交时峰值 76.5 MiB（已验证） |
| Windows 内存门不可沿用 | Retina 帧面积是 100% DPI 的 4 倍；加托盘后的地板已超过 Source 目标 28 MiB，还没有文档和字体 |
| 临时硬门（经验估算，未实测） | Source > 72、Preview > 96、Split > 128、Hidden > 48 MiB。M3 测出基线后由 USER 定最终值 |
| 沿用 Windows | 空闲 CPU（含闪烁）≤0.05% / >0.1%，启动 >550 ms，输入延迟，预览构建，ZIP 大小 |
| 预览、启动、输入的 macOS 基线 | 宿主建立后测，当前 `NOT TESTED` |

## 共享层改动（需批准）

| ID | 改动 | Windows 影响 |
| --- | --- | --- |
| S1 | `stickymd-render` 统一构造 `FontSystem`，`cfg(target_os = "macos")` 时移除没有 `head` 表的 face；用到 `FontSystem::new` 的测试一并换用。census 约 8–9 ms，可接受 | 无（cfg） |
| S2 | `Keymap` 改为由完整修饰键得出语义命令：快捷键判定、撤销/重做、查找/替换入口、行首行尾与文首文末 | `ctrl_standard` 行为不变，现有断言不改，并补测试钉住现状 |
| S3 | 默认字体候选由宿主提供（`03` 规则 9） | 值不变，只换来源 |
| S4 | 宿主接收退出屏障的结果（成功或取消），用于回答系统的终止请求，含模态窗口与超时处理；Cmd+Q 进入同一屏障 | 默认实现为空，Windows 不变 |

没有 `Unsupported` 分支的改动：这些能力 macOS 都有，Debian 报告里的能力分支不需要。

## 对规格的影响（批准后）

| 章节 | 改动 |
| --- | --- |
| `02` | 平台节写入 macOS 27+/arm64、目录身份方案 A |
| `03` | Platform Host 增加 `apps/stickymd-mac`；S2、S3 |
| `05` | macOS 存储映射、卷要求、三处弱点的声明、实例身份、新建 LF；`PreserveOutcome` 语义 |
| `07` / `08` | 输入法矩阵与 `replacementRange` 决定；pasteboard 优先级；macOS 不做受管图片的自动移动与物理删除 |
| `09` | macOS 实现映射、顶部夹取、reopen、托盘不可用行为、Cmd+Q 与系统终止 |
| `10` | 口径、临时硬门、呈现基线 |
| `11` | 新 Phase（macOS 宿主）与验收表；未测项保持 `NOT TESTED` |

新 crate `apps/stickymd-mac` 同批登记 `unsafe` 治理、CI 选择、指纹规则，`objc2` 系依赖已在 lockfile 中（`tray-icon`、`softbuffer` 带入）。

**宪法 10.1.3：** 变更原因是 `02` 要求的单独批准。不变更则 macOS 停留在“共享库可编译”。迁移：无既有 macOS 数据。回退：删除 `apps/stickymd-mac`，S2、S3 为纯重构，S4 默认空实现，S1 只影响 macOS。S2、S3、S4 改动共享层，合入前需要你在 Windows 机器上跑桌面回归。

**实施顺序（批准后）：** M0 修基线（S1）→ M1 存储与单实例，过 `storage_conformance!` → M2 窗口、托盘、唤醒、光标图层 → M3 输入、剪贴板、导出，测基线 → M4 打包脚本与人工清单。

## 人工项

这三项决定方向，批准前能看最好。终端运行，Ctrl+C 结束。

**H1 未聚焦悬停。** winit 用旧式 `addTrackingRect`，应用未激活时能否收到事件未知。不成立则 `hover reveal` 要另找机制。

```bash
/Users/develata/gitclone/StickyMD/experiments/macos-probes/target/release/manual_probe sliver
```

先点开别的应用，把鼠标移到屏幕最左边缘。看到 `CursorEntered` 算通过。

**H2 Cmd 键与 H3 输入法。**

```bash
/Users/develata/gitclone/StickyMD/experiments/macos-probes/target/release/manual_probe
```

点黄色窗口取得焦点。H2：按 Cmd+S、Cmd+C、Cmd+Shift+Z、Cmd+1，每个都应打印 `Key`。H3：切到简体拼音和微信输入法，打几个词，再按 Esc 取消一次，应看到 `Ime Preedit` 与 `Ime Commit`，候选窗贴近窗口左上方（探针把光标区设在 (40, 60)）。

其余人工项（圆角、Spaces、反色、受保护目录、Translocation、多屏）在 M2–M4 列清单。

## 待批准

1. 第 1、2 节：版本、身份、目录方案 A、实例身份。
2. 第 3 节：普通 `rename` 发布、卷要求 `RENAME_EXCL`；三处弱点逐项批准（持有句柄的写者、保留 canonical 的移动后核对、受管图片不做自动移动与物理删除）。
3. 第 4 节：顶部补 `constrainFrameRect`、托盘规则、Cmd+Q 退出、系统终止接线、所有 Space 可见。
4. 第 5 节：Cmd 映射、字体、输入法方案 A、pasteboard 优先级。
5. 第 6、7 节：`CALayer` 光标、临时硬门。
6. S1–S4。

## 独立审查

2026-11-09，`codex-reviewer`（`gpt-6-astra`，high）审查初稿 `e0c0fa8`，结论不通过。10 条发现都对照源码或重新实测核实，全部成立，正文已按此修订：

| # | 级别 | 发现 | 处理 |
| --- | --- | --- | --- |
| 1 | 阻断 | swap 换回有竞争，会丢外部新版本 | 否决 swap，改普通 `rename` |
| 2 | 阻断 | swap 后 temp 角色与恢复规则冲突 | 同上；恢复规则不用改 |
| 3 | 阻断 | 按路径删除可能删用户文件；移回失败时不能报 `Changed` | 不做物理删除；移回失败报失败 |
| 4 | 主要 | winit 没有 `applicationShouldTerminate:` | 增加系统终止接线与 S4 |
| 5 | 主要 | 光标探针在 handler 内 sleep，图层改动可能未提交 | 新增 `caret_cost` 用事件循环重测：0.009% → 0.037%，整窗 1.584% → 1.938% |
| 6 | 主要 | `$TMPDIR` 可被覆盖，hash 长度未定 | 改用 `confstr`，定 16 位十六进制，补时序 |
| 7 | 主要 | S2 范围不够；Windows 现在没有按词移动 | 扩大 S2，修正键位表 |
| 8 | 主要 | 图标不可见时没有退出入口；reopen 只验证了送达 | 增加 Cmd+Q；降级端到端声明 |
| 9 | 待核 | 基线日志和诊断补丁没有随附 | 已放入 `results/`；补打模态结束后的送达数 |
| 10 | 次要 | 字体耗时与随附结果不符 | 改用随附结果 |

第二轮（同模型，审 `530eb21` 的修订）结论仍不通过，4 条发现核实后成立，正文已修订；socket 协议与 `caret_cost` 没有发现问题。

| # | 级别 | 发现 | 处理 |
| --- | --- | --- | --- |
| 1 | 阻断 | 目录同步失败返回 `Saved` 违反 `05` 的 SaveFailed | 改报失败，adapter 记住已发布指纹 |
| 2 | 阻断 | `RENAME_EXCL` 不保护源文件身份，可能移动用户文件 | macOS 不做自动移动 |
| 3 | 次要 | 对保留 canonical 的弱化描述偏轻 | 写明永久位移与合同修改 |
| 4 | 待核 | 模态期间到来的系统终止无法进入屏障 | 先 `abortModal`，加超时，列入验收 |

第二轮的修订没有再送审。两轮审查都未覆盖故障注入、掉电、多进程竞争实跑、输入法人工验收和多屏。修订后的存储协议尚未写成代码，竞争结论来自推理。

## 复现

```bash
cd /Users/develata/gitclone/StickyMD/experiments/macos-probes && cargo build --release --bins
```

`run_fs_matrix.sh`（APFS、大小写敏感 APFS、HFS+、exFAT、FAT32，用临时磁盘映像）、`run_bundle_matrix.sh`、`window_probe`、`window_extra`、`notify_probe`、`glyph_inf`。exFAT、FAT32 的探针在 `RENAME_EXCL` 返回 `ENOTSUP` 后中止，属预期。
