# AI assistants (MCP)

## Retained-feature automation

The MCP `control` tool takes `{"operation": {...}}`, with the same typed operation schema as
`misterdownloadmanager-cli control JSON`. See the complete [operation inventory](command-line.md#typed-non-interactive-app-control).
It manages the **running app's list**, including properties, settings, categories, basic queues,
TorBox accounts, remote pairing/devices, video/gallery/playlist downloads and torrent
metadata/selection/trackers/recheck/order. This is separate from standalone CLI downloads.

`add_download` now returns the app's real id/result and never asks a duplicate question.
Its `duplicate` policy defaults to `skip`; `add`, `resume`, or destructive `replace` must be
chosen explicitly. `category` and `queue` are also accepted.

Settings and account reads omit credentials; write-only credentials must come from the user,
not inferred values. Missing patch fields preserve existing values. Removed feature keys are
rejected. Poll lookup jobs to completion; moves, renames and torrent actions can remain pending
after acceptance. Inspect properties/status afterward; do not report acceptance as success.

No GUI is needed for these operations. Inspect `helpers` and use `install_helper` jobs when media
helpers are missing (or use the standalone CLI install commands).
`choose_helper` validates/persists trusted custom executables (null resets to the default).
`verify_checksum` starts a completed-file verification job; inspect its final `matched` and
`status`, not just `pending:false`. A mismatch does not delete or redownload the file.
TorBox browser authorization is not exposed: a user-supplied API key through `torbox` is the
supported functional sign-in alternative. UI-only layout/window preferences are not exposed;
retained operational settings, including subtitle defaults and torrent associations, are configurable. (MCP)

[← Back to docs index](README.md)

MDM comes with an [MCP](https://modelcontextprotocol.io) server, `misterdownloadmanager-mcp`, so an AI assistant that supports MCP (Claude Desktop, Claude Code, Cursor, VS Code and others) can work with the downloads in your running MDM. You ask in your own words:

- "What is MDM downloading right now, and how long is left?"
- "Download https://example.com/big.iso to D:\ISOs tonight, don't start it yet."
- "Pause everything that's downloading."
- "Remove the finished downloads from the list but keep the files."

The assistant starts the server itself; you only tell the assistant where it is, once. The server passes each request to the app through its [automation API](command-line.md#the-automation-api), on this computer only.

## What the assistant can do

| Tool | What it does |
|---|---|
| `list_downloads` | Lists your downloads, newest first: file, address, folder, status, size, how much is done, speed. It can list only one id, only one status (Queued, Downloading, Stopped, Complete, Error), only names and addresses that contain some text, or only one category or queue; reports available disk space too. |
| `add_download` | Adds HTTP/HTTPS, FTP/FTPS or a magnet without dialogs; accepts folder, file name, category, queue, checksum, Referer, User-Agent, Cookie, `private`, `no_complete_dialog` and explicit duplicate policy. A magnet takes all files asynchronously; use `control` with torrent lookup/add to select files and observe failures. |
| `control` | Typed retained-feature operations described above; inspect/edit the running app without direct settings-file writes. |
| `pause_downloads` | Pauses running downloads. |
| `resume_downloads` | Starts or resumes stopped, queued or failed downloads. |
| `remove_downloads` | Removes downloads from the list and keeps their files, or deletes the files too when asked. A running download must be paused first. |
| `start_queue` | Starts a download queue (the main queue by default, or by name). |
| `stop_queue` | Stops a download queue (the main queue by default, or by name). |

The assistant can change retained settings through `control`, but cannot read your browser or turn the automation API on by itself.

## Set it up

### 1. Turn on the automation API

Listing and acting on downloads needs MDM's automation API, which is off until you turn it on. With MDM running, run this once (it stays on):

```
misterdownloadmanager-cli api on
```

`misterdownloadmanager-cli api off` turns it off again. Without the API, the assistant can still add downloads, the same as MDM's [command line](command-line.md#the-apps-command-line).

### 2. Find the server

`misterdownloadmanager-mcp` is next to the app:

| You installed | The server |
|---|---|
| Windows, for you only (the installer's default) | `%LOCALAPPDATA%\Programs\Mister Download Manager\misterdownloadmanager-mcp.exe` |
| Windows, for everyone | `C:\Program Files\Mister Download Manager\misterdownloadmanager-mcp.exe` |
| Windows, a zip or [portable copy](portable.md) | `misterdownloadmanager-mcp.exe` in its folder |
| macOS | `/Applications/Mister Download Manager.app/Contents/MacOS/misterdownloadmanager-mcp` |
| Linux, the .deb or .rpm | `/usr/bin/misterdownloadmanager-mcp` |
| Linux, the .tar.gz | `misterdownloadmanager-mcp` in its folder (the AppImage doesn't expose it: use the .tar.gz) |

A portable copy's server finds the portable copy's MDM by itself, from its own folder.

### 3. Tell your assistant

**Claude Desktop**: **Settings → Developer → Edit Config** opens `claude_desktop_config.json`. Add the server, then restart Claude Desktop:

```json
{
  "mcpServers": {
    "misterdownloadmanager": {
      "command": "C:\\Users\\you\\AppData\\Local\\Programs\\Mister Download Manager\\misterdownloadmanager-mcp.exe"
    }
  }
}
```

**Claude Code**:

```
claude mcp add misterdownloadmanager -- "C:\Users\you\AppData\Local\Programs\Mister Download Manager\misterdownloadmanager-mcp.exe"
```

**Cursor**: the same `mcpServers` entry as Claude Desktop's, in `~/.cursor/mcp.json` (or add it in Cursor's MCP settings).

**VS Code**: in `.vscode/mcp.json`, or **MCP: Add Server** in the command palette:

```json
{
  "servers": {
    "misterdownloadmanager": {
      "type": "stdio",
      "command": "C:\\Users\\you\\AppData\\Local\\Programs\\Mister Download Manager\\misterdownloadmanager-mcp.exe"
    }
  }
}
```

Use the path from step 2 (in JSON, every `\` is written `\\`). Other assistants take the same thing: a command to start, no arguments.

## Privacy

MDM itself still sends nothing anywhere: the server talks only to the MDM on this computer, over the token-protected local channel. But what the assistant reads, it passes to its own AI service: when it lists your downloads, their file names, addresses and folders go to that service, under that service's privacy terms. Turn the automation API off, or remove the server from the assistant, when you don't want that.

## When it doesn't work

| The assistant says | What to do |
|---|---|
| Mister Download Manager isn't running | Start MDM. The server can't start it for you. |
| The automation API is off | Run `misterdownloadmanager-cli api on` with MDM running. |
| MDM didn't answer in time | A window in MDM may be waiting for you (a question, **Add torrent**); answer it and ask again. |
| MDM refused the request | MDM's channel file is from an earlier start; restart MDM. |
| The tools don't show up at all | Check the path in the assistant's settings, and restart the assistant. Started by hand, `misterdownloadmanager-mcp --help` says what it is. |
