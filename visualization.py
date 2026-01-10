import os
from datetime import datetime

import numpy as np
import pandas as pd
import matplotlib.pyplot as plt
from matplotlib import font_manager


def read_csv_with_fallback(path: str) -> pd.DataFrame:
    try:
        return pd.read_csv(path, sep=None, engine="python", encoding="utf-8-sig")
    except UnicodeDecodeError:
        return pd.read_csv(path, sep=None, engine="python", encoding="gb18030")

df = read_csv_with_fallback("log.csv")
time_cols = [c for c in df.columns if "耗时" in str(c)]
plot_df = df[time_cols].apply(pd.to_numeric, errors="coerce")
use_round_depth = "回合" in df.columns and "深度" in df.columns
if use_round_depth:
    x = np.arange(1, len(plot_df) + 1)
    x_labels = (
        df["回合"].astype(str).str.strip()
        + "."
        + df["深度"].astype(str).str.strip()
    )
elif "深度" in df.columns:
    x = pd.to_numeric(df["深度"], errors="coerce")
else:
    x = np.arange(1, len(plot_df) + 1)
preferred_fonts = [
    "WenQuanYi Zen Hei",
    "Noto Sans CJK SC",
    "Noto Sans CJK",
    "SimHei",
    "Microsoft YaHei",
]
chosen = None
for f in preferred_fonts:
    try:
        fp = font_manager.findfont(f, fallback_to_default=False)
        if fp and os.path.exists(fp):
            chosen = f
            break
    except Exception:
        pass
if chosen:
    plt.rcParams["font.sans-serif"] = [chosen]
plt.rcParams["axes.unicode_minus"] = False
plt.rcParams["font.size"] = 20

fig_w, fig_h = 28, 10
plt.figure(figsize=(fig_w, fig_h))
colors = plt.cm.tab20(np.linspace(0, 1, len(time_cols)))
for i, c in enumerate(time_cols):
    plt.plot(
        x,
        plot_df[c].values,
        label=c,
        linewidth=2.0,
        color=colors[i],
    )

plt.xlabel("回合-深度", fontsize=26)
plt.ylabel("耗时（μs）", fontsize=26)
plt.grid(True, alpha=0.25)
if use_round_depth:
    plt.xticks(ticks=x, labels=x_labels, fontsize=20, rotation=45, ha="right")
else:
    plt.xticks(fontsize=20, rotation=45, ha="right")
plt.yticks(fontsize=20)
plt.legend(
    loc="upper right", frameon=False, fontsize=18
)
plt.tight_layout()
timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
png_root, png_ext = os.path.splitext("耗时折线图")
png_ext = png_ext or ".png"
png_path = f"{png_root}_{timestamp}{png_ext}"
plt.savefig(png_path)
plt.close()
