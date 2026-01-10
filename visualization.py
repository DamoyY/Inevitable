import os
from datetime import datetime
from numpy import arange, nan
import pandas as pd
import matplotlib.pyplot as plt

df = pd.read_csv("log.csv")
time_cols = [c for c in df.columns if "耗时" in str(c)]
plot_df = df[time_cols].apply(pd.to_numeric, errors="coerce")
use_round_depth = "回合" in df.columns and "深度" in df.columns
if use_round_depth:
    x = arange(1, len(plot_df) + 1)
    x_labels = (
        df["回合"].astype(str).str.strip()
        + "."
        + df["深度"].astype(str).str.strip()
    )
elif "深度" in df.columns:
    x = pd.to_numeric(df["深度"], errors="coerce")
else:
    x = arange(1, len(plot_df) + 1)
plt.rcParams["axes.unicode_minus"] = False
plt.rcParams["font.sans-serif"] = ["Microsoft YaHei", "SimHei", "DejaVu Sans"]
plt.rcParams["font.size"] = 20
fig_w, fig_h = 16, 16
fig, axes = plt.subplots(
    2,
    1,
    figsize=(fig_w, fig_h),
    sharex=True,
    gridspec_kw={"height_ratios": [1, 1]},
)
base_colors = list(plt.get_cmap("tab20").colors)
colors = [base_colors[i % len(base_colors)] for i in range(len(time_cols))]
series = [plot_df[c].values for c in time_cols]
total = plot_df.sum(axis=1).values
axes[0].stackplot(
    x, series, labels=time_cols, colors=colors, alpha=1.0, baseline="sym"
)
axes[0].axhline(0, color="#000000", linewidth=1.5)
axes[0].set_ylabel("耗时（μs）", fontsize=26)
axes[0].grid(True, alpha=0.25)
fig.legend(
    time_cols,
    loc="center left",
    bbox_to_anchor=(1.02, 0.5),
    frameon=False,
    fontsize=18,
)
total_safe = plot_df.sum(axis=1).replace(0, nan)
percent_df = plot_df.div(total_safe, axis=0) * 100
percent_series = [percent_df[c].values for c in time_cols]
axes[1].stackplot(x, percent_series, colors=colors, alpha=1.0)
axes[1].set_ylabel("占比（%）", fontsize=26)
axes[1].set_xlabel("回合-深度", fontsize=26)
axes[1].set_ylim(0, 100)
axes[1].grid(True, alpha=0.25)
if use_round_depth:
    axes[1].set_xticks(
        x, labels=x_labels, fontsize=20, rotation=45, ha="right"
    )
else:
    axes[1].set_xticks(x)
    axes[1].set_xticklabels(
        axes[1].get_xticks(), fontsize=20, rotation=45, ha="right"
    )
for ax in axes:
    ax.tick_params(axis="y", labelsize=20)
plt.tight_layout(rect=[0, 0, 1, 1])
timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
png_root, png_ext = os.path.splitext("耗时折线图")
png_ext = png_ext or ".png"
png_path = f"{png_root}_{timestamp}{png_ext}"
plt.savefig(png_path, bbox_inches="tight")
plt.close()
