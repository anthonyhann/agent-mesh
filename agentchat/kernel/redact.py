#!/usr/bin/env python3
"""redact.py — 发布前脱敏（单一事实源，publish_result.sh 与 smoke 测试共用）

将 HTML 中的本机绝对路径脱敏：$HOME → ~，/Users/<name>/ → ~/。
用法: redact.py <in.html> <out.html>
"""
import os
import re
import sys


def redact(text: str, home: str) -> str:
    out = text.replace(home, "~")
    out = re.sub(r"/Users/[A-Za-z0-9_.-]+/", "~/", out)
    return out


def main():
    src_path, dst_path = sys.argv[1], sys.argv[2]
    src = open(src_path, errors="replace").read()
    open(dst_path, "w").write(redact(src, os.environ.get("HOME", "")))
    return 0


if __name__ == "__main__":
    sys.exit(main())
