#!/bin/sh
set -eu
inspect_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
inspect_engine="$inspect_dir/inspect-engine.jar"
inspect_java=java
if [ ! -f "$inspect_engine" ] && [ -f "$inspect_dir/../engine/inspect-engine.jar" ]; then
    inspect_engine="$inspect_dir/../engine/inspect-engine.jar"
    if [ -x "$inspect_dir/../engine/runtime/bin/java" ]; then
        inspect_java="$inspect_dir/../engine/runtime/bin/java"
    fi
fi
exec "$inspect_java" -Xmx256m -Dfile.encoding=UTF-8 -Dcom.sun.security.enableAIAcaIssuers=false -jar "$inspect_engine" "$@"
