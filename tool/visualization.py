from matplotlib.pyplot import rcParams, close, subplots, tight_layout
from matplotlib.font_manager import fontManager, FontProperties
from pandas import read_csv, to_numeric
from colorsys import hls_to_rgb
from datetime import datetime
from math import pi, atan2
import numpy as np
import os

CSV_PATH = "log.csv"
OUTPUT_DIR = "tool/visualization"
CJK_FONT_CANDIDATES = [
    "Microsoft YaHei",
    "SimHei",
    "PingFang SC",
    "Heiti SC",
    "STHeiti",
    "Noto Sans CJK SC",
    "Noto Sans CJK TC",
    "Noto Sans CJK JP",
    "Source Han Sans SC",
    "Arial Unicode MS",
]
FALLBACK_FONTS = ["DejaVu Sans"]
FIG_SIZE = (16, 16)
BAR_WIDTH = 0.95
TIME_UNIT_SCALE = 1e-6
Y_ABS_LIMIT = (-1e7 * TIME_UNIT_SCALE, 1e7 * TIME_UNIT_SCALE)


def _srgb_to_linear(c):
    if c <= 0.04045:
        return c / 12.92
    return ((c + 0.055) / 1.055) ** 2.4


def _rgb_to_oklab(rgb):
    r, g, b = rgb
    r_lin = _srgb_to_linear(r)
    g_lin = _srgb_to_linear(g)
    b_lin = _srgb_to_linear(b)
    l_val = 0.4122214708 * r_lin + 0.5363325363 * g_lin + 0.0514459929 * b_lin
    m = 0.2119034982 * r_lin + 0.6806995451 * g_lin + 0.1073969566 * b_lin
    s = 0.0883024619 * r_lin + 0.2817188376 * g_lin + 0.6299787005 * b_lin
    l_ = l_val ** (1 / 3)
    m_ = m ** (1 / 3)
    s_ = s ** (1 / 3)
    l_val = 0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_
    a = 1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_
    b = 0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_
    return (l_val, a, b)


def _oklab_to_oklch(lab):
    l_val, a, b = lab
    c = (a * a + b * b) ** 0.5
    h = atan2(b, a)
    if h < 0:
        h += 2 * pi
    return (l_val, c, h)


def _oklch_distance(c1, c2):
    hue_diff = abs(c1[2] - c2[2])
    hue_wrap = min(hue_diff, 2 * pi - hue_diff)
    hue_term = hue_wrap * ((c1[1] + c2[1]) / 2)
    return ((c1[0] - c2[0]) ** 2 + (c1[1] - c2[1]) ** 2 + hue_term**2) ** 0.5


def generate_distinct_colors(count):
    if count <= 0:
        return []
    hue_count = max(60, count * 3)
    hues = np.linspace(0, 1, hue_count, endpoint=False)
    lightness = [0.38, 0.5, 0.62]
    saturation = [0.6, 0.78, 0.92]
    candidates = []
    for h in hues:
        for lightness_value in lightness:
            for saturation_value in saturation:
                candidates.append(
                    hls_to_rgb(float(h), lightness_value, saturation_value)
                )
    oklch_values = [_oklab_to_oklch(_rgb_to_oklab(c)) for c in candidates]
    gray_oklch = _oklab_to_oklch(_rgb_to_oklab((0.5, 0.5, 0.5)))
    first_idx = max(
        range(len(candidates)),
        key=lambda i: _oklch_distance(oklch_values[i], gray_oklch),
    )
    selected = [candidates[first_idx]]
    selected_oklch = [oklch_values[first_idx]]
    remaining = [i for i in range(len(candidates)) if i != first_idx]
    while len(selected) < count:
        best_idx = remaining[0]
        best_score = -1.0
        for i in remaining:
            oklch = oklch_values[i]
            min_dist = min(_oklch_distance(oklch, s) for s in selected_oklch)
            if min_dist > best_score:
                best_score = min_dist
                best_idx = i
        selected.append(candidates[best_idx])
        selected_oklch.append(oklch_values[best_idx])
        remaining.remove(best_idx)
    return selected


def _load_log(csv_path):
    head = read_csv(csv_path, nrows=0)
    all_cols = list(head.columns)
    time_cols = [c for c in all_cols if "耗时" in str(c)]
    extra_cols = []
    if "回合" in all_cols:
        extra_cols.append("回合")
    if "深度" in all_cols:
        extra_cols.append("深度")
    usecols = time_cols + extra_cols
    df = read_csv(csv_path, usecols=usecols)
    time_cols = [c for c in df.columns if "耗时" in str(c)]
    return df, time_cols


def _build_x_axis(df, plot_df):
    use_round_depth = "回合" in df.columns and "深度" in df.columns
    if use_round_depth:
        x = np.arange(1, len(plot_df) + 1)
        x_labels = (
            df["回合"].astype(str).str.strip()
            + "."
            + df["深度"].astype(str).str.strip()
        )
        return x, x_labels, use_round_depth
    if "深度" in df.columns:
        x = to_numeric(df["深度"], errors="coerce")
        return x, None, use_round_depth
    x = np.arange(1, len(plot_df) + 1)
    return x, None, use_round_depth


def _load_custom_fonts():
    font_paths = []
    for env_name in ("CJK_FONT_PATH", "CJK_FONT_PATHS", "FONT_PATH"):
        value = os.environ.get(env_name)
        if not value:
            continue
        for raw_path in value.split(os.pathsep):
            path = raw_path.strip()
            if path:
                font_paths.append(path)
    loaded_names = []
    for path in font_paths:
        if not os.path.isfile(path):
            continue
        try:
            fontManager.addfont(path)
            name = FontProperties(fname=path).get_name()
        except Exception:
            continue
        loaded_names.append(name)
    return loaded_names


def _resolve_font_families():
    custom_fonts = _load_custom_fonts()
    available = {f.name for f in fontManager.ttflist}
    selected = []
    for name in custom_fonts + CJK_FONT_CANDIDATES + FALLBACK_FONTS:
        if name in available and name not in selected:
            selected.append(name)
    if not selected:
        selected = FALLBACK_FONTS[:]
    has_cjk = any(name in CJK_FONT_CANDIDATES for name in selected) or bool(
        custom_fonts
    )
    return selected, has_cjk


def _set_plot_style():
    rcParams["axes.unicode_minus"] = False
    font_families, has_cjk = _resolve_font_families()
    rcParams["font.sans-serif"] = font_families
    rcParams["font.size"] = 20
    rcParams["figure.facecolor"] = "black"
    rcParams["axes.facecolor"] = "black"
    rcParams["axes.edgecolor"] = "white"
    rcParams["axes.labelcolor"] = "white"
    rcParams["xtick.color"] = "white"
    rcParams["ytick.color"] = "white"
    rcParams["text.color"] = "white"
    rcParams["legend.facecolor"] = "black"
    rcParams["legend.edgecolor"] = "white"
    rcParams["legend.labelcolor"] = "white"
    rcParams["grid.color"] = "gray"


def _plot_absolute(ax, x, series_values, time_cols, colors):
    total = series_values.sum(axis=1)
    bottom = -total / 2
    for idx, col in enumerate(time_cols):
        values = series_values[:, idx]
        ax.bar(
            x,
            values,
            bottom=bottom,
            label=col,
            color=colors[idx],
            alpha=1.0,
            width=BAR_WIDTH,
        )
        bottom = bottom + values
    ax.axhline(0, color="white", linewidth=1.5)
    ax.set_ylabel("耗时（秒）", fontsize=26)
    ax.set_ylim(*Y_ABS_LIMIT)
    ax.grid(True, alpha=1, color="gray")
    return total


def _plot_percent(ax, x, series_values, total, colors):
    total_safe = total.copy()
    total_safe[total_safe == 0] = np.nan
    percent_values = (
        np.divide(
            series_values,
            total_safe[:, None],
            out=np.zeros_like(series_values),
            where=~np.isnan(total_safe)[:, None],
        )
        * 100
    )
    valid_rows = ~np.isnan(total_safe)
    if np.any(valid_rows):
        avg_percent = np.mean(percent_values[valid_rows], axis=0)
        order = np.argsort(-avg_percent)
    else:
        order = np.arange(percent_values.shape[1])
    bottom = np.zeros(len(series_values))
    for idx in order:
        values = percent_values[:, idx]
        ax.bar(
            x,
            values,
            bottom=bottom,
            color=colors[idx],
            alpha=1.0,
            width=BAR_WIDTH,
        )
        bottom = bottom + values
    ax.set_ylabel("占比（%）", fontsize=26)
    ax.set_xlabel("回合-深度", fontsize=26)
    ax.set_ylim(0, 100)
    ax.grid(True, alpha=1, color="gray")


def _apply_xticks(ax, x, x_labels, use_round_depth):
    if use_round_depth:
        ax.set_xticks(x, labels=x_labels, fontsize=20, rotation=45, ha="right")
    else:
        ax.set_xticks(x)
        ax.set_xticklabels(
            ax.get_xticks(), fontsize=20, rotation=45, ha="right"
        )


def _save_figure(fig):
    timestamp = datetime.now().strftime("%m-%d_%H-%M")
    os.makedirs(OUTPUT_DIR, exist_ok=True)
    svg_path = os.path.join(OUTPUT_DIR, f"{timestamp}.svg")
    fig.savefig(
        svg_path, bbox_inches="tight", facecolor="black", edgecolor="none"
    )
    close(fig)
    return svg_path


def main():
    df, time_cols = _load_log(CSV_PATH)
    plot_df = (
        df[time_cols]
        .apply(to_numeric, errors="coerce", downcast="float")
        .mul(TIME_UNIT_SCALE)
    )
    x, x_labels, use_round_depth = _build_x_axis(df, plot_df)
    _set_plot_style()
    fig, axes = subplots(
        2,
        1,
        figsize=FIG_SIZE,
        sharex=True,
        gridspec_kw={"height_ratios": [1, 1]},
    )
    colors = generate_distinct_colors(len(time_cols))
    plot_filled = plot_df.fillna(0)
    series_values = plot_filled[time_cols].to_numpy()
    total = _plot_absolute(axes[0], x, series_values, time_cols, colors)
    handles, labels = axes[0].get_legend_handles_labels()
    fig.legend(
        handles,
        labels,
        loc="center left",
        bbox_to_anchor=(1.02, 0.5),
        frameon=False,
        fontsize=18,
    )
    _plot_percent(axes[1], x, series_values, total, colors)
    _apply_xticks(axes[1], x, x_labels, use_round_depth)
    for ax in axes:
        ax.tick_params(axis="y", labelsize=20)
    tight_layout(rect=(0, 0, 1, 1))
    _save_figure(fig)


if __name__ == "__main__":
    main()
