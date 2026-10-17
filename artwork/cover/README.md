# NanaType 封面

独立静态封面实现，只提交代码。角色以本地透明 PNG 输入，不包含 Live2D 模型、纹理、角色图片或封面成品。

```powershell
cd artwork/cover
npm ci
npm run check
npm run render -- C:/path/to/character.png
```

默认导出到忽略的 `out/`：`cover-16x9.png`（1920×1080）、`cover-4x3.png`（1440×1080）、`cover-standard.png`（1920×1200）、`cover-portrait.png`（1080×1440）。4:3 使用横版居中裁切，主要内容保留在安全区。

角色路径也可通过 `NANATYPE_COVER_CHARACTER` 设置。可选第二个参数指定输出目录；`REMOTION_BROWSER_EXECUTABLE` 可指定现有 Chrome / Edge 可执行文件，未指定时使用 Remotion 管理的浏览器。

排版、配色、背景图形与关键词集中在 `Cover.tsx`。渲染只打包一次并复用浏览器，角色副本与打包目录在退出时自动清理；不依赖宣传片、皮肤生成数据或本地 Live2D 工程。字体使用 Noto Sans SC，首次加载需要联网。
