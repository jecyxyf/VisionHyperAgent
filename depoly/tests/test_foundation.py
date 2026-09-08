"""Run real file/config/singleton tests without a GUI, network or production data."""
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]


def main():
    stages = [
        ("日志和配置功能测试（日期、并发、备份与失败保护）",
         ["cargo", "test", "--offline", "--lib", "foundation::"]),
        ("全局单例黑盒测试（独立进程、真实文件、不同工作目录）",
         ["cargo", "test", "--offline", "--test", "foundation"]),
    ]
    for label, command in stages:
        print(f"\n=== {label} ===", flush=True)
        try:
            result = subprocess.run(command, cwd=ROOT, check=False)
        except OSError as error:
            print(f"无法执行测试：{error}", file=sys.stderr)
            return 1
        if result.returncode:
            return result.returncode
    print("\n全部功能测试通过；测试文件位于临时目录，未修改用户配置。", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
