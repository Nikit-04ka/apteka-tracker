# -*- coding: utf-8 -*-
"""Генерация иконки «Домашняя аптечка»: аптечный ящик с красным крестом и часами.

Запуск из корня проекта:  python assets/make_icon.py
Создаёт assets/icon.png (512) и assets/icon.ico (16..256).
"""
from PIL import Image, ImageDraw, ImageFilter

S = 1024  # рисуем в большом разрешении, даунсемплим для сглаживания

img = Image.new("RGBA", (S, S), (0, 0, 0, 0))

# --- фон: вертикальный градиент teal внутри скруглённого квадрата ---
top, bottom = (45, 212, 191), (15, 118, 110)  # teal-400 -> teal-700
grad = Image.new("RGBA", (S, S))
gd = ImageDraw.Draw(grad)
for y in range(S):
    t = y / (S - 1)
    c = tuple(round(top[i] + (bottom[i] - top[i]) * t) for i in range(3)) + (255,)
    gd.line([(0, y), (S, y)], fill=c)

mask = Image.new("L", (S, S), 0)
ImageDraw.Draw(mask).rounded_rectangle(
    [0, 0, S - 1, S - 1], radius=int(S * 0.22), fill=255
)
img.paste(grad, (0, 0), mask)

# --- тень коробки ---
shadow = Image.new("RGBA", (S, S), (0, 0, 0, 0))
ImageDraw.Draw(shadow).rounded_rectangle(
    [212, 316, 812, 742], radius=48, fill=(0, 0, 0, 70)
)
img.alpha_composite(shadow.filter(ImageFilter.GaussianBlur(18)))

d = ImageDraw.Draw(img)

# --- ручка ящика ---
d.rounded_rectangle(
    [432, 252, 592, 330], radius=34,
    fill=(226, 232, 240, 255), outline=(203, 213, 225, 255), width=6,
)

# --- белый ящик ---
d.rounded_rectangle([212, 300, 812, 724], radius=48, fill=(255, 255, 255, 255))

# --- светлая полоса-крышка (нижние углы квадратные) ---
d.rounded_rectangle([212, 300, 812, 400], radius=48, fill=(248, 250, 252, 255))
d.rectangle([212, 352, 812, 400], fill=(248, 250, 252, 255))
d.line([212, 400, 812, 400], fill=(226, 232, 240, 255), width=6)

# --- красный крест ---
cx, cy = 512, 562
arm, th = 112, 78
d.rounded_rectangle(
    [cx - th // 2, cy - arm, cx + th // 2, cy + arm], radius=28, fill=(220, 38, 38, 255)
)
d.rounded_rectangle(
    [cx - arm, cy - th // 2, cx + arm, cy + th // 2], radius=28, fill=(220, 38, 38, 255)
)

# --- часы «срок годности» в правом нижнем углу ---
ccx, ccy, r = 796, 748, 116
csh = Image.new("RGBA", (S, S), (0, 0, 0, 0))
ImageDraw.Draw(csh).ellipse(
    [ccx - r + 8, ccy - r + 14, ccx + r + 8, ccy + r + 14], fill=(0, 0, 0, 60)
)
img.alpha_composite(csh.filter(ImageFilter.GaussianBlur(10)))

d = ImageDraw.Draw(img)
d.ellipse(
    [ccx - r, ccy - r, ccx + r, ccy + r],
    fill=(255, 255, 255, 255), outline=(51, 65, 85, 255), width=26,
)
# стрелки: «без 12» — вверх и вправо
d.line([ccx, ccy, ccx, ccy - 58], fill=(51, 65, 85, 255), width=18)
d.line([ccx, ccy, ccx + 48, ccy], fill=(51, 65, 85, 255), width=18)
d.ellipse([ccx - 11, ccy - 11, ccx + 11, ccy + 11], fill=(51, 65, 85, 255))
d.ellipse([ccx - 5, ccy - 5, ccx + 5, ccy + 5], fill=(255, 255, 255, 255))

# --- даунсемпл и сохранение ---
master = img.resize((512, 512), Image.LANCZOS)
master.save("assets/icon.png")
master.save(
    "assets/icon.ico",
    sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
)
print("OK: assets/icon.png + assets/icon.ico")
