"""Evaluation metrics for financial forecasting and trading models."""

from typing import Dict
import numpy as np
from scipy import stats


def information_coefficient(y_pred: np.ndarray, y_true: np.ndarray) -> float:
    """Calculate Pearson Information Coefficient (IC)."""
    if len(y_pred) < 2 or np.std(y_pred) < 1e-8 or np.std(y_true) < 1e-8:
        return 0.0
    r, _ = stats.pearsonr(y_pred.flatten(), y_true.flatten())
    return float(r) if not np.isnan(r) else 0.0


def rank_information_coefficient(y_pred: np.ndarray, y_true: np.ndarray) -> float:
    """Calculate Spearman Rank Information Coefficient (Rank IC)."""
    if len(y_pred) < 2 or np.std(y_pred) < 1e-8 or np.std(y_true) < 1e-8:
        return 0.0
    rho, _ = stats.spearmanr(y_pred.flatten(), y_true.flatten())
    return float(rho) if not np.isnan(rho) else 0.0


def directional_accuracy(y_pred: np.ndarray, y_true: np.ndarray) -> float:
    """Percentage of predictions that correctly forecast return sign."""
    pred_sign = np.sign(y_pred.flatten())
    true_sign = np.sign(y_true.flatten())
    valid_mask = true_sign != 0
    if not np.any(valid_mask):
        return 0.5
    correct = (pred_sign[valid_mask] == true_sign[valid_mask]).sum()
    return float(correct / valid_mask.sum())


def annualized_sharpe_ratio(
    returns: np.ndarray,
    risk_free_rate: float = 0.0,
    periods_per_year: int = 252,
) -> float:
    """Calculate annualized Sharpe ratio from daily strategy returns."""
    excess = returns.flatten() - (risk_free_rate / periods_per_year)
    std = np.std(excess)
    if std < 1e-8:
        return 0.0
    return float(np.mean(excess) / std * np.sqrt(periods_per_year))


def max_drawdown(equity_curve: np.ndarray) -> float:
    """Calculate peak-to-trough maximum drawdown."""
    curve = equity_curve.flatten()
    if len(curve) < 2:
        return 0.0
    running_max = np.maximum.accumulate(curve)
    drawdowns = (curve - running_max) / np.maximum(running_max, 1e-8)
    return float(np.min(drawdowns))


def compute_metrics(
    y_pred: np.ndarray,
    y_true: np.ndarray,
    risk_free_rate: float = 0.04,
    periods_per_year: int = 252,
) -> Dict[str, float]:
    """Compute comprehensive out-of-sample evaluation report."""
    y_p = y_pred.flatten()
    y_t = y_true.flatten()

    # Synthetic strategy returns assuming proportional positions
    positions = np.tanh(y_p)
    strat_returns = positions * y_t
    equity = np.cumprod(1.0 + strat_returns)

    return {
        "ic": information_coefficient(y_p, y_t),
        "rank_ic": rank_information_coefficient(y_p, y_t),
        "directional_accuracy": directional_accuracy(y_p, y_t),
        "annualized_sharpe": annualized_sharpe_ratio(strat_returns, risk_free_rate, periods_per_year),
        "max_drawdown": max_drawdown(equity),
        "mean_return": float(np.mean(strat_returns)),
        "volatility": float(np.std(strat_returns) * np.sqrt(periods_per_year)),
    }
