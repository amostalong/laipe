#!/usr/bin/env python3
"""Add TS-side builtin-tools.ts + export from index.ts."""
import shutil, os, sys

ROOT = r"D:\Projects\Laipe"
SRC  = ROOT

# 1. Copy builtin-tools.txt -> packages/laipe-ts/src/builtin-tools.ts
shutil.copyfile(
    os.path.join(ROOT, "builtin-tools.txt"),
    os.path.join(SRC, "packages", "laipe-ts", "src", "builtin-tools.ts"),
)
print("OK: packages/laipe-ts/src/builtin-tools.ts")

# 2. Patch packages/laipe-ts/src/index.ts: add re-exports
INDEX_TS = os.path.join(SRC, "packages", "laipe-ts", "src", "index.ts")
with open(INDEX_TS, "rb") as f:
    data = f.read()
if b"builtin-tools" not in data:
    add_line = (
        b"export {\n"
        b"  askFreeTextSchema,\n"
        b"  askUserQuestionSchema,\n"
        b"  builtinMetaByName,\n"
        b"  builtinTools,\n"
        b"  updateDocItemSchema,\n"
        b"  BUILTIN_TOOL_META,\n"
        b"  BUILTIN_TOOL_NAMES,\n"
        b"} from \"./builtin-tools.js\";\n"
        b"export type {\n"
        b"  BuiltinToolMeta,\n"
        b"  BuiltinToolName,\n"
        b"  ToolPermission,\n"
        b"  ToolRisk,\n"
        b"} from \"./builtin-tools.js\";\n"
    )
    data = data + add_line
    print("OK: laipe-ts/src/index.ts added builtin-tools exports")
else:
    print("SKIP: laipe-ts/src/index.ts already exports builtin-tools")
with open(INDEX_TS, "wb") as f:
    f.write(data)
