#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
port=5900
app="${APP:-$(pwd)/dist/Vinny.app}"
if [[ ! -x "$app/Contents/MacOS/vinny" ]]; then
  echo "Build Vinny.app with ./scripts/package.sh first" >&2
  exit 1
fi
"$app/Contents/MacOS/vinny" >"${TMPDIR:-/tmp}/vinny-smoke.log" 2>&1 &
pid=$!
cleanup() {
  kill "$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true
}
trap cleanup EXIT

for _ in {1..50}; do
  if lsof -nP -a -p "$pid" -iTCP:"$port" -sTCP:LISTEN 2>/dev/null | grep -q "127.0.0.1:$port"; then
    break
  fi
  sleep .1
done
lsof -nP -a -p "$pid" -iTCP:"$port" -sTCP:LISTEN | grep "127.0.0.1:$port"

PORT="$port" python3 - <<'PY'
import os, socket, struct, time
port = int(os.environ["PORT"])
s = socket.create_connection(("127.0.0.1", port), timeout=3)

def exact(size):
    data = b""
    while len(data) < size:
        chunk = s.recv(size - len(data))
        if not chunk:
            raise EOFError(f"wanted {size} bytes, got {len(data)}")
        data += chunk
    return data

assert exact(12) == b"RFB 003.008\n"
s.sendall(b"RFB 003.008\n")
security_types = exact(exact(1)[0])
assert 1 in security_types
s.sendall(b"\x01")
assert exact(4) == b"\0\0\0\0"
s.sendall(b"\x01")
width, height = struct.unpack(">HH", exact(4))
pixel_format = exact(16)
bytes_per_pixel = pixel_format[0] // 8
name_length = struct.unpack(">I", exact(4))[0]
name = exact(name_length).decode()
assert name == "Vinny Display 1"
s.sendall(struct.pack(">BBH", 2, 0, 1) + struct.pack(">i", 0))
time.sleep(.3)
s.sendall(struct.pack(">BBHHHH", 3, 0, 0, 0, width, height))
message = exact(4)
assert message[0] == 0
rectangle_count = struct.unpack(">H", message[2:])[0]
variation = 0
for _ in range(rectangle_count):
    _, _, rect_width, rect_height, encoding = struct.unpack(">HHHHi", exact(12))
    assert encoding == 0
    pixels = exact(rect_width * rect_height * bytes_per_pixel)
    variation = max(variation, len(set(pixels[:200_000])))
assert rectangle_count > 0 and variation > 1
print(f"RFB framebuffer OK: {width}x{height}, {rectangle_count} rectangle(s)")
PY
