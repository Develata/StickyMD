# Phase 17 macOS 虚拟机验收（2026-11-18）

Phase 17 的 macOS 人工视觉项（P17-N12、P17-R06）与资源项（P17-C12）在 `mac-test` 虚拟机里的记录。
行定义见 [Phase 17 验收矩阵](../acceptance-cases/phase-17.md)，实施进度见 [Phase 17 任务](../tasks/phase-17-copy-and-notices.md)。

## 身份

- **被测程序：** `2d73ab9` 的 release，用 `apps/stickymd-mac/packaging/build-app.sh` 打包、ad-hoc 签名。
  `b4ccd6b` 之后只改了文档，产品代码与分支 CI `44113614349` 测过的 `b4ccd6b` 相同。
  可执行文件 SHA-256 `806c03ec66e36df51175d6295de5095a3ce73251815714152e55834eda4102e4`。
- **资源测量的基线：** Phase 17 分支的起点 `0841524`，在单独的工作树里用同一脚本打包。
  可执行文件 SHA-256 `bf1ec9c0c916148633516feaf99e492141312554e4489868334faeb6681dc1ed`。
- **驱动：** `2d73ab9` 的 `stickymd-smoke`，按 `CARGO_PROFILE_RELEASE_PANIC=unwind CARGO_TARGET_DIR=target/unwind cargo build --locked --release -p stickymd-smoke` 构建，
  SHA-256 `025f1424a02a5a8bdc698a21cb05c8cef1fe1b19d7dca168ea1c7752f3e32ad7`。
- **虚拟机：** `mac-test`，macOS 27.2（26B123），`tart run --no-graphics`，经 SSH 主连接操作。

## 人工收据：P17-N12、P17-R06（macOS，2026-11-18）

**做法：**

- 被测程序装在虚拟机的 `~/p17`，程序目录里放一份测试笔记。笔记包含：
  - 短代码块、长行代码块（带横向滚动条）、空代码块；
  - 两个块级公式（多行与单行 `$$`）、一个 Raw HTML 块；
  - 引用中的代码块，以及四层列表中的代码块；
  - 一张表格与一个行内公式，二者不应有侧条。
- USER 经“屏幕共享”连到虚拟机，自己操作并查看。
- 给 USER 的检查清单：
  - R06：
    1. 侧条的位置与高度，长行代码块的侧条不含横向滚动条轨道，行内公式与表格没有侧条；
    2. 宽度 8 内容 DIP 是否合适；
    3. Light/Dark 下的低对比；
    4. 悬停变亮与手形光标，点击后的“已复制”，空代码块点击后剪贴板不变、无提示；
    5. 40% 透明度；
    6. 放大内容与窄窗下，放不下的侧条整根不画。
  - N12：
    1. Cmd+S 后的“已保存”、点击侧条后的“已复制”，看位置与对比度，Split 下横跨两栏；
    2. 三种外观各看一次：Light、Dark、40% 透明度；
    3. Source 的最后一行可被瞬时提示暂时盖住。

**USER 回复：** “都通过”（2026-11-18）。

**这次没有覆盖的：**

- **两类提示没有触发：** 冲突的待决定提示（带 F6/F7 按钮）与保存失败的故障展示，要由我在虚拟机里改动笔记或目录权限才会出现。
  我提出可以配合触发，USER 没有要求，回复里也没有提到这两类，所以它们的外观不在这次确认之内。
  它们的行为由自动化行覆盖（N07、N11、N13、N16 等）。
- **Windows 宿主：** 没有做。两行的 Windows 部分仍待在 Windows 宿主上确认。
- **侧条宽度：** 8 内容 DIP 在 macOS 上通过。plan 06 写的是“视觉验收后固定”；Windows 的视觉验收还没做，所以 plan 的“暂定”暂不改。

**验收表：** 两行的 Mode 都是 `Manual`，覆盖两个宿主，而 Windows 部分未做，所以 Status 保持 `NOT TESTED`。
Evidence 列加上本收据（macOS 部分）。

## P17-C12：进程内存门（macOS）

（测量进行中。）
