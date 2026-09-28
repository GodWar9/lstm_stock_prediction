#!/usr/bin/env python3
"""
Python child process helper for yfinance data extraction.
Called exclusively by Rust's YfinanceAdapter.
Does NOT perform any modeling, feature calculation, or backtesting.
"""

import argparse
import os
import sys
from pathlib import Path


def fetch_ohlcv(symbol: str, start: str, end: str, output_path: str):
    try:
        require_network_permission()
        import yfinance as yf
        ticker = yf.Ticker(symbol)
        df = ticker.history(start=start, end=end, auto_adjust=False)
        if df.empty:
            raise ValueError(f"No historical data returned for {symbol} between {start} and {end}")

        out_lines = ["timestamp,open,high,low,close,volume"]
        for dt, row in df.iterrows():
            ts_str = dt.strftime("%Y-%m-%d")
            out_lines.append(
                f"{ts_str},{row['Open']:.4f},{row['High']:.4f},{row['Low']:.4f},{row['Close']:.4f},{int(row['Volume'])}"
            )

        Path(output_path).parent.mkdir(parents=True, exist_ok=True)
        with open(output_path, "w", encoding="utf-8") as f:
            f.write("\n".join(out_lines) + "\n")
        print(f"Wrote {len(out_lines)-1} bars to {output_path}")

    except Exception as e:
        sys.stderr.write(f"Error fetching OHLCV for {symbol}: {e}\n")
        sys.exit(1)


def fetch_actions(symbol: str, start: str, end: str, output_path: str):
    try:
        require_network_permission()
        import yfinance as yf
        ticker = yf.Ticker(symbol)
        actions = ticker.actions
        out_lines = ["date,action_type,value"]

        if actions is not None and not actions.empty:
            for dt, row in actions.iterrows():
                dt_str = dt.strftime("%Y-%m-%d")
                if start <= dt_str <= end:
                    if "Stock Splits" in row and row["Stock Splits"] > 0:
                        out_lines.append(f"{dt_str},SPLIT,{row['Stock Splits']:.4f}")
                    if "Dividends" in row and row["Dividends"] > 0:
                        out_lines.append(f"{dt_str},DIVIDEND,{row['Dividends']:.4f}")

        Path(output_path).parent.mkdir(parents=True, exist_ok=True)
        with open(output_path, "w", encoding="utf-8") as f:
            f.write("\n".join(out_lines) + "\n")
        print(f"Wrote {len(out_lines)-1} actions to {output_path}")

    except Exception as e:
        sys.stderr.write(f"Error fetching actions for {symbol}: {e}\n")
        sys.exit(1)


def require_network_permission():
    if os.environ.get("QUANTCTL_ALLOW_NETWORK") != "1":
        raise RuntimeError("Network acquisition is disabled; use local CSV data. Explicit online acquisition requires QUANTCTL_ALLOW_NETWORK=1.")


def main():
    parser = argparse.ArgumentParser(description="Market data fetch helper")
    parser.add_argument("--symbol", required=True, help="Ticker symbol")
    parser.add_argument("--start", required=True, help="Start date (YYYY-MM-DD)")
    parser.add_argument("--end", required=True, help="End date (YYYY-MM-DD)")
    parser.add_argument("--output", required=True, help="Output CSV path")
    parser.add_argument("--actions", action="store_true", help="Fetch corporate actions instead of OHLCV")

    args = parser.parse_args()

    if args.actions:
        fetch_actions(args.symbol, args.start, args.end, args.output)
    else:
        fetch_ohlcv(args.symbol, args.start, args.end, args.output)


if __name__ == "__main__":
    main()
