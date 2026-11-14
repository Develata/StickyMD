# macOS R03：App Translocation 探针

P16-R03（`docs/acceptance-cases/phase-16.md`）：从网上下载的包，在 App Translocation 下要先给出提示，再退出。本页说明怎样在测试虚拟机里复现，不需要 Apple 开发者账号。

## 前提与限制

- 在 Tart 虚拟机 `mac-test` 中运行，不在 USER 桌面运行。
- 2026-11-13 的实测是在 Gatekeeper 评估关闭的条件下做的（`spctl --status` 显示 `assessments disabled`），这是该镜像的默认状态。
- Gatekeeper 打开时，ad-hoc 签名的包会先被拦下，要在“系统设置 → 隐私与安全性”里选“仍要打开”。之后是否同样随机化，未验证。
- 随机化的触发条件 Apple 没有完整公开，可能随系统版本改变。

## 步骤

1. 把待测的 `StickyMD.app` 放在 `~/stickymd/`。
2. 模拟一次浏览器下载：打成 zip，加上隔离标记，再用“归档实用工具”解压。归档实用工具会把隔离标记传给解出的每个文件。

   ```sh
   D=~/r03; rm -rf "$D"; mkdir -p "$D/download"
   ditto -c -k --keepParent ~/stickymd/StickyMD.app "$D/download/StickyMD.zip"
   xattr -w com.apple.quarantine "0083;$(printf %x $(date +%s));Safari;" "$D/download/StickyMD.zip"
   open -W -a "Archive Utility" "$D/download/StickyMD.zip"
   xattr -p com.apple.quarantine "$D/download/StickyMD.app"
   ```

3. 不移动包，直接从解压目录启动，与双击走同一条 LaunchServices 路径：

   ```sh
   ls -la "$D/download" > "$D/before.txt"
   open "$D/download/StickyMD.app" --stderr "$D/stderr.log"
   ```

4. 等提示出现。启动约需 5 秒，期间系统在做 AMFI 与 TCC 检查。然后检查以下几项：
   - `ps -o comm= -p $(pgrep -x StickyMD)` 的路径含 `/AppTranslocation/`；
   - `mount | grep AppTranslocation` 显示一个只读的 nullfs 挂载；
   - `stderr.log` 中有“当前目录不可写，请先用访达把 StickyMD.app 移到别处。”与 `Read-only file system`；
   - 提示窗口在屏幕上（模态面板层级）。
5. 关掉提示。从 SSH 启动时，StickyMD 不会自动成为前台程序，所以先用 `open "$D/download/StickyMD.app"` 再打开一次：系统会激活已在运行的那个实例，不会再起一个新进程。然后按回车，或点“好”。
6. 进程应当退出，launchd 记录的状态为 `(0, 0, 256)`，即退出码 1。`ls -la "$D/download"` 应与 `before.txt` 相同：下载目录里没有新文件，也没有 `note/`。

## 记录

结果与证据记在 [VM 验收报告](../../docs/report/2026-11-10-macos-vm-acceptance.md)。R03 是人工行，探针结果要经 USER 认可才能作为收据，而且每次确认只代表那一次，不复用（`mac-ledger` 的 `NEVER_REUSED`）。
