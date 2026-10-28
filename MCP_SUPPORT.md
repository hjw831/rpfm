# Native MCP Support for RPFM

This fork of RPFM includes native support for the [Model Context Protocol (MCP)](https://modelcontextprotocol.io/), allowing AI agents (like Qwen Code, Cline, or Roo Code) to directly interact with RPFM to manage PackFiles.

## 🚀 Features

The MCP server exposes RPFM's core functionality as a set of tools, including:
- **Pack Management**: Open, create, and save PackFiles.
- **File Operations**: List, read, write, rename, and delete files within a pack.
- **Table Editing**: Read and edit DB tables (insert/update/delete rows).
- **Schema & Diagnostics**: Update schemas and run diagnostics.
- **Search**: Run searches across the pack contents.

## 🛠️ Installation & Build

To use the MCP features, you must build the `rpfm_server` from source.

### Prerequisites
- **Rust Toolchain**: Install via [rustup.rs](https://rustup.rs/).
- **C++ Build Tools**: Install "Desktop development with C++" via the [Visual Studio Installer](https://visualstudio.microsoft.com/visual-cpp-build-tools/).

### Build Steps
1. Clone this repository:
   ```bash
   git clone https://github.com/hjw831/rpfm.git
   cd rpfm
   ```
2. Initialize submodules:
   ```bash
   git submodule update --init --recursive
   ```
3. Build the server in release mode:
   ```bash
   cargo build --release -p rpfm_server
   ```

The binary will be located at: `target/release/rpfm_server.exe`

## ⚙️ Configuration

To connect an MCP client to RPFM, use the following configuration. The server must be launched with the `--mcp-stdio` argument.

### Example Configuration (`mcp_config.json`)
```json
{
  "mcpServers": {
    "rpfm": {
      "command": "C:/path/to/your/rpfm/target/release/rpfm_server.exe",
      "args": ["--mcp-stdio"]
    }
  }
}
```

## 📖 Usage Guide for AI Agents

To begin using RPFM, the agent should follow this sequence:
1. **Set Game**: Call `set_game` with the target game key (e.g., `"warhammer_3"`).
2. **Open Pack**: Call `open_pack` with the path to the `.pack` file.
3. **Operate**: Use tools like `list_files` or `table_rows` to inspect and modify the data.
