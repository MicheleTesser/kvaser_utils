import argparse
import csv
from pathlib import Path

import pandas as pd
from asammdf import MDF

DEFAULT_RESAMPLE_PERIOD = "5ms"
DEFAULT_TIMEZONE = "Europe/Rome"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Export MF4 logs to CSV with DBC decoding (format aligned with reference CSV)."
    )
    parser.add_argument(
        "--logs-dir",
        type=Path,
        help="Folder containing .mf4 files. If omitted, a folder picker is shown.",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        help='Output folder for CSV files (default: "<logs-dir>/csv").',
    )
    parser.add_argument(
        "--can1-dbc",
        type=Path,
        help="DBC file mapped to CAN1 (channel mask 1).",
    )
    parser.add_argument(
        "--can2-dbc",
        type=Path,
        help="DBC file mapped to CAN2 (channel mask 2).",
    )
    parser.add_argument(
        "--resample-period",
        default=DEFAULT_RESAMPLE_PERIOD,
        help=f'Resample period (default: "{DEFAULT_RESAMPLE_PERIOD}"). Use empty string to disable.',
    )
    parser.add_argument(
        "--timezone",
        default=DEFAULT_TIMEZONE,
        help=f'Timezone for timestamps (default: "{DEFAULT_TIMEZONE}").',
    )
    return parser.parse_args()


def pick_logs_dir() -> Path:
    import tkinter as tk
    from tkinter import filedialog

    root = tk.Tk()
    root.withdraw()
    selected = filedialog.askdirectory(title="Select logs folder")
    if not selected:
        raise SystemExit("No logs folder selected")
    return Path(selected)


def pick_dbc_paths() -> tuple[Path, Path]:
    import tkinter as tk
    from tkinter import filedialog

    root = tk.Tk()
    root.withdraw()
    selected = filedialog.askopenfilenames(
        title="Select CAN1 and CAN2 DBC files",
        filetypes=(("DBC files", "*.dbc"), ("All files", "*.*")),
    )
    if len(selected) < 2:
        raise SystemExit("You need to select both CAN1 and CAN2 DBC files")

    can1 = None
    can2 = None
    for item in selected:
        path = Path(item).resolve()
        stem = path.stem.lower()
        if stem == "can1":
            can1 = path
        elif stem == "can2":
            can2 = path

    if can1 is None or can2 is None:
        raise SystemExit("Could not infer both CAN1 and CAN2 DBC files from selected names")
    return can1, can2


def load_dataframe(mf4_path: Path, databases: dict[str, list[tuple[Path, int]]]) -> pd.DataFrame:
    mdf = MDF(mf4_path)

    # Preferred path: decode bus logging using DBC files.
    extracted = mdf.extract_bus_logging(databases)
    df = extracted.to_dataframe(
        time_as_date=True,
        use_display_names=True,
    )

    # Fallback: file may already be signal-based.
    if df.empty:
        df = mdf.to_dataframe(
            time_as_date=True,
            use_display_names=True,
        )
    return df


def normalize_dataframe(
    df: pd.DataFrame,
    timezone: str,
    resample_period: str,
) -> pd.DataFrame:
    if not isinstance(df.index, pd.DatetimeIndex):
        df.index = pd.to_datetime(df.index, utc=True, errors="coerce")

    if df.index.tz is None:
        df.index = df.index.tz_localize("UTC")
    df.index = df.index.tz_convert(timezone)

    df = df.apply(pd.to_numeric, errors="coerce")

    if resample_period:
        df = df.resample(resample_period).mean()

    return df


def main() -> None:
    args = parse_args()

    logs_dir = args.logs_dir.resolve() if args.logs_dir else pick_logs_dir()
    if not logs_dir.is_dir():
        raise SystemExit(f"Logs directory not found: {logs_dir}")

    if args.can1_dbc and args.can2_dbc:
        can1_dbc = args.can1_dbc.resolve()
        can2_dbc = args.can2_dbc.resolve()
    else:
        can1_dbc, can2_dbc = pick_dbc_paths()

    if not can1_dbc.is_file():
        raise SystemExit(f"CAN1 DBC not found: {can1_dbc}")
    if not can2_dbc.is_file():
        raise SystemExit(f"CAN2 DBC not found: {can2_dbc}")

    output_dir = args.output_dir.resolve() if args.output_dir else logs_dir / "csv"
    output_dir.mkdir(parents=True, exist_ok=True)

    # Deterministic order: CAN1 first, then CAN2.
    databases = {"CAN": sorted([(can1_dbc, 1), (can2_dbc, 2)], key=lambda item: item[1])}

    mf4_files = sorted(logs_dir.glob("*.mf4"))
    if not mf4_files:
        raise SystemExit(f"No .mf4 files found in {logs_dir}")

    print(f"Logs dir: {logs_dir}")
    print(f"Output dir: {output_dir}")
    print(f"CAN1 DBC: {can1_dbc}")
    print(f"CAN2 DBC: {can2_dbc}")
    print(f"Resample: {args.resample_period or 'disabled'}")
    print(f"Timezone: {args.timezone}")

    for mf4_path in mf4_files:
        out_csv = output_dir / f"{mf4_path.stem}.csv"
        try:
            print(f"\nExtracting: {mf4_path}")
            df = load_dataframe(mf4_path, databases)
            df = normalize_dataframe(df, args.timezone, args.resample_period)

            print(f"Exporting: {out_csv}")
            df.to_csv(
                out_csv,
                index=True,
                index_label="timestamps",
                quoting=csv.QUOTE_NONNUMERIC,
            )
            print(f"Done: {out_csv}")
        except KeyboardInterrupt:
            print("Interrupted by user")
            break
        except Exception as exc:
            print(f"Error exporting {mf4_path}: {exc}")

    print("\nCompleted")


if __name__ == "__main__":
    main()
