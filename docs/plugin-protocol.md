# Background Studio 插件协议

`pluginProtocol: 1`

Codex / Notion Background Studio 可作为 Background Studio 壳的插件进程运行。

## 启动

```text
CodexBackgroundStudio.exe --plugin
```

插件模式行为：

- 不创建系统托盘
- 不写本应用的 Windows 自启动项
- 主窗口默认隐藏；由壳通过 IPC `open-ui` 打开
- 在 Named Pipe 上提供控制接口

独立启动（无 `--plugin`）保持原有托盘与自启动行为。

## Pipe 名称

| 插件 | Pipe |
|------|------|
| Codex | `\\.\pipe\background-studio-codex` |
| Notion | `\\.\pipe\background-studio-notion` |

## 消息格式

换行分隔 JSON（NDJSON）。主机发起请求，插件回复。

### 请求

```json
{"id":"1","cmd":"status"}
{"id":"2","cmd":"open-ui"}
{"id":"3","cmd":"apply"}
{"id":"4","cmd":"pause"}
{"id":"5","cmd":"restore"}
{"id":"6","cmd":"quit-keep-target"}
```

### 成功响应

```json
{
  "id": "1",
  "ok": true,
  "result": {
    "pluginProtocol": 1,
    "pluginId": "codex",
    "version": "0.5.0",
    "phase": "active",
    "message": "已连接",
    "activeTargets": 1,
    "paused": false
  }
}
```

### 失败响应

```json
{"id":"3","ok":false,"error":"……"}
```

## Release 产物

除 NSIS 安装包外，每个插件仓发布：

- `CodexBackgroundStudio-<version>-plugin.zip`
- `NotionBackgroundStudio-<version>-plugin.zip`

壳解压到：

`%LOCALAPPDATA%\BackgroundStudio\plugins\<pluginId>\<version>\`
