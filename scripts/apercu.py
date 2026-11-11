"""Rend chaque fond intégré dans une planche-contact (nécessite Pillow)."""
import subprocess, sys, os
from PIL import Image
exe, out = sys.argv[1], sys.argv[2]
t = sys.argv[3] if len(sys.argv) > 3 else "12"
ids = ["aurore", "encre", "ondes", "horizon", "constellation", "lave", "brume"]
W, H = 480, 270
sheet = Image.new("RGB", (W * 2, H * 4))
for i, f in enumerate(ids):
    bmp = os.path.join(os.path.dirname(out), f + ".bmp")
    subprocess.run([exe, "--capture", f, bmp, "960", "540", t], check=True)
    sheet.paste(Image.open(bmp).convert("RGB").resize((W, H)), ((i % 2) * W, (i // 2) * H))
sheet.save(out)
