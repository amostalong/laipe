#!/usr/bin/env python3
"""Add built-in tool schemas to Laipe lib (Rust + TS)."""
import shutil, sys, os

ROOT = r"D:\Projects\Laipe"
SRC  = ROOT

# 1. Copy builtin_tools.txt -> crates/laipe-core/src/builtin_tools.rs
shutil.copyfile(
    os.path.join(ROOT, "builtin_tools.txt"),
    os.path.join(SRC, "crates", "laipe-core", "src", "builtin_tools.rs"),
)
print("OK: crates/laipe-core/src/builtin_tools.rs")

# 2. Patch crates/laipe-core/src/lib.rs: add `pub mod builtin_tools;` + re-export
LIB_RS = os.path.join(SRC, "crates", "laipe-core", "src", "lib.rs")
with open(LIB_RS, "rb") as f:
    data = f.read()
add_module = b"pub mod builtin_tools;\n"
if add_module not in data:
    data = data.replace(b"pub mod tool;\n", b"pub mod builtin_tools;\npub mod tool;\n", 1)
    print("OK: lib.rs add pub mod builtin_tools;")

# Add re-exports after existing tool re-exports
old_re = b"pub use tool::{ToolCallInfo, ToolCallPartial, ToolDefinition, ToolResult};\n"
new_re = (
    b"pub use tool::{ToolCallInfo, ToolCallPartial, ToolDefinition, ToolResult};\n"
    b"pub use builtin_tools::{\n"
    b"    ask_free_text_schema, ask_user_question_schema, builtin_meta_by_name,\n"
    b"    builtin_tools, update_doc_item_schema, BuiltinTool, BuiltinToolMeta,\n"
    b"    ToolPermission, ToolRisk, BUILTIN_TOOL_META,\n"
    b"};\n"
)
if old_re in data and b"pub use builtin_tools::" not in data:
    data = data.replace(old_re, new_re, 1)
    print("OK: lib.rs add builtin_tools re-exports")
elif b"pub use builtin_tools::" in data:
    print("SKIP: lib.rs already has builtin_tools re-exports")

with open(LIB_RS, "wb") as f:
    f.write(data)

# 3. Patch laipe-app/src/tools.ts: use builtin_tools from laipe-ts
#    Keep the 2 demo tools (get_current_time, echo) and concat builtin tools.
TOOLS_TS = os.path.join(SRC, "laipe-app", "src", "tools.ts")
with open(TOOLS_TS, "rb") as f:
    tdata = f.read()

old_import = b'import type { ToolDefinition } from "laipe-ts";\n'
new_import = (
    b'import type { ToolDefinition } from "laipe-ts";\n'
    b'import { builtin_tools } from "laipe-ts";\n'
)
if old_import in tdata and b'import { builtin_tools }' not in tdata:
    tdata = tdata.replace(old_import, new_import, 1)

old_export = (
    b"export const TOOLS: ToolDefinition[] = [GET_CURRENT_TIME, ECHO];\n"
)
new_export = (
    b"// Demo tools (laipe-app starter) + the 3 built-in canonical\n"
    b"// patterns laipe ships in lib (see packages/laipe-ts/src/builtin-tools.ts\n"
    b"// and crates/laipe-core/src/builtin_tools.rs). Apps would usually\n"
    b"// disable some of the built-ins in production (e.g. update_doc_item\n"
    b"// if your fork has no project documents to mutate).\n"
    b"export const TOOLS: ToolDefinition[] = [\n"
    b"  GET_CURRENT_TIME,\n"
    b"  ECHO,\n"
    b"  ...builtin_tools(),\n"
    b"];\n"
)
if old_export in tdata:
    tdata = tdata.replace(old_export, new_export, 1)
    print("OK: laipe-app/src/tools.ts updated")
else:
    print("WARN: laipe-app/src/tools.ts old_export not found")

with open(TOOLS_TS, "wb") as f:
    f.write(tdata)
