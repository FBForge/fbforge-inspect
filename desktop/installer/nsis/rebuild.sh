#!/bin/sh
# Run inside a bounded offline build sandbox. No downloads or installation.
set -eu
if [ "$#" -ne 2 ]; then
  printf '%s\n' 'Usage: rebuild.sh PINNED_INPUT_DIRECTORY OUTPUT_DIRECTORY' >&2
  exit 2
fi
fbforge_nsis_assets=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
fbforge_nsis_inputs=$(CDPATH= cd -- "$1" && pwd)
fbforge_nsis_output=$2
mkdir -p "$fbforge_nsis_output"
fbforge_nsis_output=$(CDPATH= cd -- "$fbforge_nsis_output" && pwd)
fbforge_nsis_scratch=$(mktemp -d "${TMPDIR:-/tmp}/fbforge-nsis-rebuild.XXXXXXXX")
trap 'rm -rf -- "$fbforge_nsis_scratch"' EXIT HUP INT TERM
python3 - "$fbforge_nsis_assets" "$fbforge_nsis_inputs" <<'PY'
import hashlib,json,pathlib,sys
assets,inputs=map(pathlib.Path,sys.argv[1:])
for item in json.loads((assets/'build-inputs.json').read_text()):
 data=(inputs/item['name']).read_bytes()
 if hashlib.sha256(data).hexdigest()!=item['sha256']:
  raise SystemExit('Pinned NSIS build input mismatch')
PY
mkdir -p "$fbforge_nsis_scratch/src" "$fbforge_nsis_scratch/scons" "$fbforge_nsis_scratch/mingw" "$fbforge_nsis_scratch/bin"
tar -xzf "$fbforge_nsis_inputs/nsis-v313.tar.gz" --strip-components=1 -C "$fbforge_nsis_scratch/src"
dpkg-deb -x "$fbforge_nsis_inputs/scons.deb" "$fbforge_nsis_scratch/scons"
for fbforge_nsis_package in \
  gcc-mingw-w64-i686-posix_13.2.0-6ubuntu1+26.1_amd64.deb \
  binutils-mingw-w64-i686_2.45.90.20260125-1ubuntu1+13.3_amd64.deb \
  mingw-w64-i686-dev_13.0.0-2ubuntu1_all.deb \
  gcc-mingw-w64-i686-posix-runtime_13.2.0-6ubuntu1+26.1_amd64.deb; do
  dpkg-deb -x "$fbforge_nsis_inputs/$fbforge_nsis_package" "$fbforge_nsis_scratch/mingw"
done
fbforge_nsis_mingw="$fbforge_nsis_scratch/mingw/usr"
cat > "$fbforge_nsis_scratch/bin/i686-w64-mingw32-gcc" <<'SH'
#!/bin/sh
exec "$FBFORGE_NSIS_MINGW/bin/i686-w64-mingw32-gcc-posix" --sysroot="$FBFORGE_NSIS_MINGW/i686-w64-mingw32" "$@"
SH
cp "$fbforge_nsis_scratch/bin/i686-w64-mingw32-gcc" "$fbforge_nsis_scratch/bin/i686-w64-mingw32-g++"
chmod 700 "$fbforge_nsis_scratch/bin/"*
export FBFORGE_NSIS_MINGW="$fbforge_nsis_mingw"
export PATH="$fbforge_nsis_scratch/bin:$fbforge_nsis_mingw/bin:$PATH"
export PYTHONPATH="$fbforge_nsis_scratch/scons/usr/lib/python3/dist-packages"
export SOURCE_DATE_EPOCH=1767225600
cd "$fbforge_nsis_scratch/src"
patch --fuzz=0 -p1 < "$fbforge_nsis_assets/no-temp-fallback.patch"
python3 "$fbforge_nsis_scratch/scons/usr/bin/scons" -j1 stubs VERSION=3.13 VER_MAJOR=3 VER_MINOR=13 TARGET_ARCH=x86 SKIPPLUGINS=all SKIPUTILS=all SKIPMISC=all SKIPDOC=all XGCC_W32_PREFIX=i686-w64-mingw32- PATH="$PATH"
python3 - "$fbforge_nsis_assets" "$fbforge_nsis_output" <<'PY'
import hashlib,json,pathlib,sys
assets,output=map(pathlib.Path,sys.argv[1:])
pins=json.loads((assets/'manifest.json').read_text())
for name,pin in pins['stubs'].items():
 compressor=name.split('-')[0].removesuffix('_solid')
 data=(pathlib.Path('build/urelease')/('stub_'+name)/('stub_'+compressor+'.exe')).read_bytes()
 if len(data)!=pin['size'] or hashlib.sha256(data).hexdigest()!=pin['sha256']:
  raise SystemExit('Rebuilt stub differs from the reviewed artifact')
 (output/name).write_bytes(data)
print('All six rebuilt Unicode stubs match the reviewed artifacts.')
PY
