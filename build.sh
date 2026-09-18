#!/usr/bin/env bash
# VisionHyperAgent Windows package build script.
# The only deployment artifact left in bin/ is the green-program zip.
set -euo pipefail
cd "$(dirname "$0")"

echo "=== 1/3 构建前端 ==="
cd source/frontend
npm ci --silent --no-audit --no-fund
npm run build

echo "=== 2/3 构建 Windows x64 release ==="
cd ../backend
if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "win32" ]]; then
  windows_exe="target/release/vha-server.exe"
else
  cargo build --target x86_64-pc-windows-gnu --release --bin vha-server
  windows_exe="target/x86_64-pc-windows-gnu/release/vha-server.exe"
fi

echo "=== 3/3 收集产物并打包 ==="
cd ../..
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
windows_exe = root / "source" / "backend" / os.environ["VHA_WINDOWS_EXE"]

# Preserve private runtime state in bin/, but never expose loose build outputs.
for legacy in [
    "VisionHyperAgent",
    "VisionHyperAgent.exe",
    "VisionHyperAgent.next",
    "VisionHyperAgent.exe.next",
    "README-Windows.txt",
]:
    path = bin_dir / legacy
    if path.exists():
        path.unlink()

staging = bin_dir / ".windows-package"
if staging.exists():
    shutil.rmtree(staging)
staging.mkdir()

readme = """VisionHyperAgent Windows 部署说明
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

更多说明见 docs/codex-agent-usage.md。
"""
(staging / "README-Windows.txt").write_text(readme, encoding="utf-8")

entries = [
    (windows_exe, "VisionHyperAgent.exe"),
    (root / "docs/app-config.example.json", "app_config.example.json"),
    (root / "docs/codex-agent-usage.md", "codex-agent-usage.md"),
    (root / "docs/codex-agent-test-report.md", "codex-agent-test-report.md"),
    (root / "docs/frontend-backend-architecture.md", "frontend-backend-architecture.md"),
    (staging / "README-Windows.txt", "README-Windows.txt"),
]
for source, name in entries:
    if not source.is_file():
        raise SystemExit(f"missing package entry: {source}")
    destination = staging / name
    if source != destination:
        shutil.copy2(source, destination)

zip_path = bin_dir / "VisionHyperAgent-windows-x64.zip"
temporary = zip_path.with_suffix(".zip.next")
with zipfile.ZipFile(temporary, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
    for _, name in entries:
        archive.write(staging / name, name)
temporary.replace(zip_path)
shutil.rmtree(staging)

def artifact(path: Path):
    content = path.read_bytes()
    return {"bytes": len(content), "sha256": hashlib.sha256(content).hexdigest()}

artifacts = {zip_path.name: artifact(zip_path)}
(bin_dir / "test-artifacts/final/artifacts.json").write_text(
    json.dumps(artifacts, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
)
PY

echo "✅ 完成:"
ls -lh bin/VisionHyperAgent-windows-x64.zip
