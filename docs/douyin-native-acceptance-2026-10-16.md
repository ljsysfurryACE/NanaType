# 抖音原生 Rust 验收

2026-10-16，Windows 11 企业版 10.0.26200 x64。精简实现后，在公开直播间 [imxiaoxin，694562812381](https://live.douyin.com/694562812381) 重新验证；全程匿名接收，不启动浏览器或外部采集器。

| 项目 | 结果 |
| --- | --- |
| 运行与接收 | 188.37 秒，161 条解码消息 |
| 数据通道离线 | 第 31–51 秒，共 20 秒；阻断 4 次重连拨号 |
| 恢复后的首次消息 | 第 68 秒，恢复耗时约 17 秒，含退避与初始化 |
| 恢复后持续接收 | 120 秒，110 条消息；每分钟 72、38 条 |
| 已验证原生连接 / 重复消息 | 2 次连接，非礼物消息 ID 重复 0 |

测试通过回环字节中继切断真实 WebSocket TCP 连接并阻止新拨号；TLS 仍由 Rust 与官方服务器端到端验证，不解密或伪造消息。HTTP 房间查询和系统网络保持正常。采集线程、解码器和去重状态在故障期间保留，不手动刷新或重启。

## 复现

```powershell
$env:NANATYPE_DOUYIN_ROOM = '694562812381' # 运行时必须仍在直播
$env:NANATYPE_DOUYIN_NATIVE_SECONDS = '120'
cargo test -p nanatype-douyin --lib native_live_network_acceptance -j 2 -- --ignored --nocapture
```

## 检查

- 工作区测试：341 项通过，0 项失败，3 项显式验收默认忽略；上述联网验收另行运行并通过。
- 工作区 Clippy（所有目标、`--no-deps -- -D warnings`）、格式、差异空白和许可检查通过，502 个依赖许可声明有效。
- 聚合日志：`target/douyin-native-simplified-live.log`、`target/douyin-simplified-workspace-tests.log`，不记录 Cookie、消息内容或完整签名地址。

在设置的抖音页填写房间号或网址后连接即可。实现仅包含原生 Rust；旧浏览器配置可读取并忽略。Cookie 账号验证与 macOS 尚未实测，许可和协议说明见[接入文档](douyin.md)。
