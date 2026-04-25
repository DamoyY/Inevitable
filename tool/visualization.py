from collections.abc import Sequence
from colorsys import hls_to_rgb
from datetime import UTC, datetime
from math import atan2, pi
from os import environ, pathsep
from pathlib import Path
from typing import TypeAlias

import numpy as np
from matplotlib.axes import Axes
from matplotlib.figure import Figure
from matplotlib.font_manager import FontProperties, fontManager
from matplotlib.pyplot import close, rcParams, subplots, tight_layout
from numpy.typing import NDArray
from pandas import DataFrame, Series, read_csv, to_numeric

Color: TypeAlias = tuple[float, float, float]
FloatArray: TypeAlias = NDArray[np.float64]
CSV_PATH = Path("log.csv")
OUTPUT_DIR = Path("tool/visualization")
CJK_FONT_CANDIDATES: list[str] = [
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
FALLBACK_FONTS: list[str] = ["DejaVu Sans"]
FIG_SIZE = 16, 16
BAR_WIDTH = 0.95
TIME_UNIT_SCALE = 1e-06
Y_ABS_LIMIT: tuple[float, float] = (
    -15000000.0 * TIME_UNIT_SCALE,
    15000000.0 * TIME_UNIT_SCALE,
)


def _srgb_to_linear(color_channel: float) -> float:
    if color_channel <= 0.04045:
        return color_channel / 12.92
    return ((color_channel + 0.055) / 1.055) ** 2.4


def _rgb_to_oklab(rgb: Color) -> Color:
    r, g, b = rgb
    r_lin = _srgb_to_linear(color_channel=r)
    g_lin = _srgb_to_linear(color_channel=g)
    b_lin = _srgb_to_linear(color_channel=b)
    l_val = 0.4122214708 * r_lin + 0.5363325363 * g_lin + 0.0514459929 * b_lin
    m = 0.2119034982 * r_lin + 0.6806995451 * g_lin + 0.1073969566 * b_lin
    s = 0.0883024619 * r_lin + 0.2817188376 * g_lin + 0.6299787005 * b_lin
    l_ = l_val ** (1 / 3)
    m_ = m ** (1 / 3)
    s_ = s ** (1 / 3)
    l_val = 0.2104542553 * l_ + 0.793617785 * m_ - 0.0040720468 * s_
    a = 1.9779984951 * l_ - 2.428592205 * m_ + 0.4505937099 * s_
    b = 0.0259040371 * l_ + 0.7827717662 * m_ - 0.808675766 * s_
    return l_val, a, b


def _oklab_to_oklch(lab: Color) -> Color:
    l_val, a, b = lab
    c = (a * a + b * b) ** 0.5
    h: float = atan2(b, a)
    if h < 0:
        h += 2 * pi
    return l_val, c, h


def _oklch_distance(first: Color, second: Color) -> float:
    hue_diff = abs(first[2] - second[2])
    hue_wrap = min(hue_diff, 2 * pi - hue_diff)
    hue_term = hue_wrap * ((first[1] + second[1]) / 2)
    return (
        (first[0] - second[0]) ** 2 + (first[1] - second[1]) ** 2 + hue_term**2
    ) ** 0.5


def generate_distinct_colors(count: int) -> list[Color]:
    if count <= 0:
        return []
    hue_count: int = max(60, count * 3)
    hues = np.linspace(start=0, stop=1, num=hue_count, endpoint=False)
    lightness: list[float] = [0.38, 0.5, 0.62]
    saturation: list[float] = [0.6, 0.78, 0.92]
    candidates: list[Color] = [
        hls_to_rgb(h=float(hue), l=lightness_value, s=saturation_value)
        for hue in hues
        for lightness_value in lightness
        for saturation_value in saturation
    ]
    oklch_values = [
        _oklab_to_oklch(lab=_rgb_to_oklab(rgb=c)) for c in candidates
    ]
    gray_oklch = _oklab_to_oklch(lab=_rgb_to_oklab(rgb=(0.5, 0.5, 0.5)))
    first_idx: int = max(
        range(len(candidates)),
        key=lambda i: _oklch_distance(
            first=oklch_values[i],
            second=gray_oklch,
        ),
    )
    selected = [candidates[first_idx]]
    selected_oklch = [oklch_values[first_idx]]
    remaining: list[int] = [
        i for i in range(len(candidates)) if i != first_idx
    ]
    while len(selected) < count:
        best_idx: int = remaining[0]
        best_score: float = -1.0
        for i in remaining:
            oklch = oklch_values[i]
            min_dist = min(
                _oklch_distance(first=oklch, second=s) for s in selected_oklch
            )
            if min_dist > best_score:
                best_score = min_dist
                best_idx = i
        selected.append(candidates[best_idx])
        selected_oklch.append(oklch_values[best_idx])
        remaining.remove(best_idx)
    return selected


def _load_log(csv_path: Path) -> tuple[DataFrame, list[str]]:
    head: DataFrame = read_csv(filepath_or_buffer=csv_path, nrows=0)
    all_cols: list[str] = list(head.columns)
    time_cols: list[str] = [c for c in all_cols if "耗时" in str(c)]
    extra_cols: list[str] = []
    if "回合" in all_cols:
        extra_cols.append("回合")
    if "深度" in all_cols:
        extra_cols.append("深度")
    usecols: list[str] = time_cols + extra_cols
    df: DataFrame = read_csv(filepath_or_buffer=csv_path, usecols=usecols)
    time_cols = [c for c in df.columns if "耗时" in str(c)]
    return df, time_cols


def _build_x_axis(
    df: DataFrame,
    plot_df: DataFrame,
) -> tuple[FloatArray, Series | None, bool]:
    use_round_depth: bool = "回合" in df.columns and "深度" in df.columns
    if use_round_depth:
        x = np.arange(1, stop=len(plot_df) + 1, dtype=np.float64)
        x_labels = (
            df["回合"].astype(str).str.strip()
            + "."
            + df["深度"].astype(str).str.strip()
        )
        return x, x_labels, use_round_depth
    if "深度" in df.columns:
        x = np.asarray(
            to_numeric(df["深度"], errors="coerce"),
            dtype=np.float64,
        )
        return x, None, use_round_depth
    x = np.arange(1, stop=len(plot_df) + 1, dtype=np.float64)
    return x, None, use_round_depth


def _load_custom_fonts() -> list[str]:
    font_paths: list[Path] = []
    for env_name in ("CJK_FONT_PATH", "CJK_FONT_PATHS", "FONT_PATH"):
        value: str | None = environ.get(env_name)
        if not value:
            continue
        for raw_path in value.split(sep=pathsep):
            path_value: str = raw_path.strip()
            if path_value:
                font_paths.append(Path(path_value))
    loaded_names: list[str] = []
    for font_path in font_paths:
        if not font_path.is_file():
            print(f"字体文件不存在，已跳过: {font_path}")
            continue
        try:
            fontManager.addfont(path=str(font_path))
            name = FontProperties(fname=str(font_path)).get_name()
        except (OSError, RuntimeError, ValueError) as error:
            print(f"加载字体失败，已跳过 {font_path}: {error}")
        else:
            loaded_names.append(name)
    return loaded_names


def _resolve_font_families() -> list[str]:
    custom_fonts = _load_custom_fonts()
    available: set[str] = {f.name for f in fontManager.ttflist}
    selected: list[str] = []
    for name in custom_fonts + CJK_FONT_CANDIDATES + FALLBACK_FONTS:
        if name in available and name not in selected:
            selected.append(name)
    if not selected:
        selected = FALLBACK_FONTS[:]
    return selected


def _set_plot_style() -> None:
    rcParams["axes.unicode_minus"] = False
    font_families = _resolve_font_families()
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


def _plot_absolute(
    ax: Axes,
    x: FloatArray,
    series_values: FloatArray,
    time_cols: Sequence[str],
    colors: Sequence[Color],
) -> FloatArray:
    total = series_values.sum(axis=1)
    valid_rows = total != 0
    if np.any(valid_rows):
        avg_time = np.mean(a=series_values[valid_rows], axis=0)
        order = _centered_order_by_average(avg_values=avg_time)
    else:
        order = np.arange(series_values.shape[1])
    bottom = -total / 2
    for idx in order:
        values = series_values[:, idx]
        ax.bar(
            x,
            values,
            bottom=bottom,
            label=time_cols[idx],
            color=colors[idx],
            alpha=1.0,
            width=BAR_WIDTH,
        )
        bottom += values
    ax.axhline(0, color="white", linewidth=1.5)
    ax.set_ylabel("耗时（秒）", fontsize=26)
    ax.set_ylim(*Y_ABS_LIMIT)
    ax.grid(visible=True, alpha=1, color="gray")
    return total


def _plot_percent(
    ax: Axes,
    x: FloatArray,
    series_values: FloatArray,
    total: FloatArray,
    colors: Sequence[Color],
) -> None:
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
    if np.any(a=valid_rows):
        avg_percent = np.mean(a=percent_values[valid_rows], axis=0)
        order = np.argsort(a=-avg_percent)
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
        bottom += values
    ax.set_ylabel("占比（%）", fontsize=26)
    ax.set_xlabel("回合-深度", fontsize=26)
    ax.set_ylim(0, 100)
    ax.grid(visible=True, alpha=1, color="gray")


def _centered_order_by_average(avg_values: FloatArray) -> NDArray[np.int64]:
    count: int = len(avg_values)
    if count == 0:
        return np.array(object=[], dtype=int)
    sorted_idx = np.argsort(a=-avg_values)
    center: float = (count - 1) / 2
    positions = list(range(count))
    positions.sort(key=lambda i: abs(i - center))
    order = np.empty(count, dtype=int)
    for rank, pos in enumerate(iterable=positions):
        order[pos] = sorted_idx[rank]
    return order


def _apply_xticks(
    ax: Axes,
    x: FloatArray,
    x_labels: Series | None,
    *,
    use_round_depth: bool,
) -> None:
    if use_round_depth:
        if x_labels is None:
            message = "启用回合-深度坐标时缺少坐标标签。"
            raise ValueError(message)
        ax.set_xticks(
            ticks=x,
            labels=x_labels,
            fontsize=20,
            rotation=45,
            ha="right",
        )
    else:
        ax.set_xticks(ticks=x)
        ax.set_xticklabels(
            ax.get_xticks(),
            fontsize=20,
            rotation=45,
            ha="right",
        )


def _save_figure(fig: Figure) -> Path:
    timestamp: str = datetime.now(tz=UTC).astimezone().strftime("%m-%d_%H-%M")
    OUTPUT_DIR.mkdir(exist_ok=True, parents=True)
    svg_path = OUTPUT_DIR / f"{timestamp}.svg"
    fig.savefig(
        svg_path,
        bbox_inches="tight",
        facecolor="black",
        edgecolor="none",
    )
    close(fig=fig)
    return svg_path


def main() -> None:
    df, time_cols = _load_log(csv_path=CSV_PATH)
    numeric_plot: DataFrame | Series = df[time_cols].apply(
        func=to_numeric,
        errors="coerce",
        downcast="float",
    )
    if not isinstance(numeric_plot, DataFrame):
        message = "耗时列转换结果不是 DataFrame。"
        raise TypeError(message)
    plot_df: DataFrame = numeric_plot.mul(other=TIME_UNIT_SCALE)
    x, x_labels, use_round_depth = _build_x_axis(df, plot_df)
    _set_plot_style()
    fig, axes = subplots(
        nrows=2,
        ncols=1,
        figsize=FIG_SIZE,
        sharex=True,
        gridspec_kw={"height_ratios": [1, 1]},
    )
    colors = generate_distinct_colors(count=len(time_cols))
    plot_filled: DataFrame = plot_df.fillna(value=0)
    series_values = plot_filled[time_cols].to_numpy(dtype=np.float64)
    total = _plot_absolute(
        ax=axes[0],
        x=x,
        series_values=series_values,
        time_cols=time_cols,
        colors=colors,
    )
    handles, labels = axes[0].get_legend_handles_labels()
    fig.legend(
        handles=handles,
        labels=labels,
        loc="center left",
        bbox_to_anchor=(1.02, 0.5),
        frameon=False,
        fontsize=18,
    )
    _plot_percent(
        ax=axes[1],
        x=x,
        series_values=series_values,
        total=total,
        colors=colors,
    )
    _apply_xticks(
        ax=axes[1],
        x=x,
        x_labels=x_labels,
        use_round_depth=use_round_depth,
    )
    for ax in axes:
        ax.tick_params(axis="y", labelsize=20)
    tight_layout(rect=(0, 0, 1, 1))
    _save_figure(fig=fig)


if __name__ == "__main__":
    main()
