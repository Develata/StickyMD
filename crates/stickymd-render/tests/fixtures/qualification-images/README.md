# G5 图片样本

这四份 96×64 图片由当前锁定版本的 Rust `image` 编码器生成，包含红、蓝、绿、黄四个
不透明区域，供桌面截图辨认。PNG/JPEG/WebP/GIF 的格式和实际解码均由产品路径验证：

```powershell
cargo test --locked -p stickymd-render --test qualification_images
```

G5 smoke 通过 `include_bytes!` 使用同一份文件，不保留另一套 base64 副本；smoke CLI
仍然只依赖标准库。图片变化必须使 G5 qualification fingerprint 失效。此测试证明样本
可解码，不替代 exact candidate 的桌面运行或人工视觉验收。
