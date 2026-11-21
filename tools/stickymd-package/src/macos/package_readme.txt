Requirements: macOS 27 or later on Apple silicon (arm64). Earlier macOS versions have not been tested.

Installed (recommended): drag StickyMD.app into Applications (/Applications or ~/Applications) and open it there.
The note is kept in ~/Library/Application Support/StickyMD/StickyMD/note/. Replacing the app with a newer
StickyMD.app of the same name keeps the same note. A copy renamed to another name keeps another note.

Portable: move the whole StickyMD folder somewhere writable with Finder, then open StickyMD.app inside it.
The note is kept next to the app in StickyMD/note/. Copying the whole folder gives a second note.

Opening the app inside the folder just unzipped in Downloads may make macOS run it from a read-only random
location; StickyMD then says the folder is not writable and quits without writing anything. Move it first.

First open: this app is ad-hoc signed and not notarized, so macOS may refuse to open it. Verify the SHA-256
checksum, then open System Settings > Privacy & Security, choose Open Anyway for StickyMD and confirm.

Upgrade: quit StickyMD first. Installed: replace StickyMD.app in Applications. Portable: unzip the new
version, copy the old StickyMD/note/ folder into the new StickyMD folder, and check the note before deleting
the old folder.

Moving a note between portable and installed: quit every copy, back up the whole note/ folder, and copy it
only if the target has no note/ folder yet. StickyMD never moves, merges or deletes notes by itself.

StickyMD has no network access. Remote images are never downloaded; their alt text and link remain available.
License: MIT. Complete Rust dependency notices and the KaTeX font license are included in this package.
Project: https://github.com/Develata/StickyMD

中文说明
- 安装版（推荐）：把 StickyMD.app 拖进“应用程序”再打开，便签在 ~/Library/Application Support/StickyMD/StickyMD/note/。
- 便携版：用访达把整个 StickyMD 文件夹移到可写的位置再打开，便签在旁边的 note/。
- 不要在“下载”里解压出的原地打开；系统可能把它放到只读的随机位置运行，程序会提示“当前目录不可写”后退出，不写任何文件。
- 第一次打开：本程序为 ad-hoc 签名、未经公证。先核对 SHA-256，被拦下时到“系统设置 → 隐私与安全性”点“仍要打开”。
- 升级前先退出程序；在便携版与安装版之间搬运便签前先备份 note/，目标已有 note/ 时不要覆盖。
