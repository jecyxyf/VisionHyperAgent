#!/usr/bin/env bash
# Build the embedded frontend and produce green desktop packages in bin/.
set -euo pipefail
cd "$(dirname "$0")"

echo "=== 1/4 构建前端 ==="
cd source/frontend
npm ci --silent --no-audit --no-fund
npm run build

echo "=== 2/4 构建 Linux x64 release ==="
cd ../backend
cargo build --manifest-path Cargo.toml --target x86_64-unknown-linux-gnu --release --bin vha-server
linux_exe="target/x86_64-unknown-linux-gnu/release/vha-server"

echo "=== 3/4 构建 Windows x64 release ==="
if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "win32" ]]; then
  cargo build --manifest-path Cargo.toml --release --bin vha-server
  windows_exe="target/release/vha-server.exe"
else
  cargo build --manifest-path Cargo.toml --target x86_64-pc-windows-gnu --release --bin vha-server
  windows_exe="target/x86_64-pc-windows-gnu/release/vha-server.exe"
fi

echo "=== 4/4 收集产物并打包 ==="
cd ../..
export VHA_LINUX_EXE="$linux_exe"
export VHA_WINDOWS_EXE="$windows_exe"
mkdir -p bin bin/test-artifacts/final

python3 - <<'PY'
from pathlib import Path
import hashlib
import json
import os
import shutil
import zipfile

root = Path.cwd()
bin_dir = root / "bin"
linux_exe = root / "source" / "backend" / os.environ["VHA_LINUX_EXE"]
windows_exe = root / "source" / "backend" / os.environ["VHA_WINDOWS_EXE"]

# Preserve private runtime state in bin/, but never expose loose build outputs.
for legacy in [
    "VisionHyperAgent",
    "VisionHyperAgent.exe",
    "VisionHyperAgent.next",
    "VisionHyperAgent.exe.next",
    "README-Windows.txt",
    "README-Linux.txt",
]:
    path = bin_dir / legacy
    if path.exists():
        path.unlink()

shared_entries = [
    (root / "docs/app-config.example.json", "app_config.example.json"),
    (root / "docs/codex-agent-usage.md", "codex-agent-usage.md"),
    (root / "docs/codex-agent-test-report.md", "codex-agent-test-report.md"),
    (root / "docs/frontend-backend-architecture.md", "frontend-backend-architecture.md"),
]

windows_readme = '''VisionHyperAgent Windows 部署说明
================================

1. 解压整个目录，不要只复制 exe。
2. 将 app_config.example.json 复制为 exe 同目录下的 app_config.json。
3. 编辑 app_config.json：
   - providers[].baseUrl 填上游 /v1 地址；
   - providers[].apiKey 填私有密钥；
   - models[].modelId 是前端显示和请求使用的唯一 ID；
   - models[].modelName 填上游真实模型名；
   - agent.activeModelId 必须指向已配置的 modelId。
4. Windows 需要能找到同平台原生 Codex 可执行文件。codex.executable 为空时，程序会按应用目录、depends/codex 和 PATH 查找。
5. 首次启动会把找到的原生 Codex 复制为 VisionHyperAgentCodex.exe，用于区分本应用持有的子进程。
6. 本版本使用 app_config.json，不再使用 config.local.toml。
7. app_config.json 包含密钥，不要提交到 Git、不要随压缩包分发。
8. 双击 VisionHyperAgent.exe 启动；浏览器关闭不会退出后台。需要退出时使用系统托盘图标的“退出”菜单。
9. 日志位于 logs/VisionHyperAgent.YYYY-MM-DD.log。

更多说明见 codex-agent-usage.md。
'''

linux_readme = '''VisionHyperAgent Linux x64 部署说明
==================================

1. 解压整个目录，不要只复制主程序。
2. 在应用目录执行：cp app_config.example.json app_config.json
3. 编辑 app_config.json：
   - providers[].baseUrl 填上游 /v1 地址；
   - providers[].apiKey 填私有密钥；
   - models[].modelId 是前端显示和请求使用的唯一 ID；
   - models[].modelName 填上游真实模型名；
   - agent.activeModelId 必须指向已配置的 modelId。
4. Linux 需要能找到同平台原生 Codex 可执行文件。codex.executable 为空时，程序会按应用目录、depends/codex 和 PATH 查找。
5. 首次启动会把找到的原生 Codex 复制为 VisionHyperAgentCodex，用于区分本应用持有的子进程。
6. 启动命令：./VisionHyperAgent
7. 浏览器关闭不会退出后台。需要退出时使用系统托盘图标的“退出”菜单。
8. app_config.json 包含密钥，不要提交到 Git、不要随压缩包分发。
9. 日志位于 logs/VisionHyperAgent.YYYY-MM-DD.log。

更多说明见 codex-agent-usage.md。
'''

def package(output: Path, executable: Path, executable_name: str, readme_name: str, readme: str):
    staging = bin_dir / f".{output.stem}-package"
    if staging.exists():
        shutil.rmtree(staging)
    staging.mkdir()

    entries = [(executable, executable_name), *shared_entries, (staging / readme_name, readme_name)]
    for source, _ in entries[:-1]:
        if not source.is_file():
            raise SystemExit(f"missing package entry: {source}")
    for source, name in entries[:-1]:
        destination = staging / name
        if source != destination:
            shutil.copy2(source, destination)
    (staging / readme_name).write_text(readme, encoding="utf-8")

    temporary = output.with_suffix(".zip.next")
    with zipfile.ZipFile(temporary, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for _, name in entries:
            info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.create_system = 3
            info.external_attr = (0o100755 if name == executable_name else 0o100644) << 16
            archive.writestr(info, (staging / name).read_bytes(), compress_type=zipfile.ZIP_DEFLATED, compresslevel=9)
    temporary.replace(output)
    shutil.rmtree(staging)

package(bin_dir / "VisionHyperAgent-linux-x64.zip", linux_exe, "VisionHyperAgent", "README-Linux.txt", linux_readme)
package(bin_dir / "VisionHyperAgent-windows-x64.zip", windows_exe, "VisionHyperAgent.exe", "README-Windows.txt", windows_readme)

def artifact(path: Path):
    content = path.read_bytes()
    return {"bytes": len(content), "sha256": hashlib.sha256(content).hexdigest()}

artifacts = {
    path.name: artifact(path)
    for path in [
        bin_dir / "VisionHyperAgent-linux-x64.zip",
        bin_dir / "VisionHyperAgent-windows-x64.zip",
    ]
}
(bin_dir / "test-artifacts/final/artifacts.json").write_text(
    json.dumps(artifacts, ensure_ascii=False, indent=2) + "\n",
    encoding="utf-8",
)
PY

echo "✅ 完成:"
ls -lh bin/VisionHyperAgent-linux-x64.zip bin/VisionHyperAgent-windows-x64.zip
