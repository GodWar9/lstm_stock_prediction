"""Loss functions for financial forecasting and return prediction."""

import torch
import torch.nn as nn
import torch.nn.functional as F


class DirectionalAsymmetricLoss(nn.Module):
    """Asymmetric loss that penalizes predictions with the wrong sign more heavily.
    
    If y_pred * y_true < 0 (wrong direction), multiplier alpha > 1 is applied.
    """

    def __init__(self, alpha: float = 2.0, delta: float = 1.0):
        super().__init__()
        assert alpha >= 1.0, "alpha must be >= 1.0"
        self.alpha = alpha
        self.delta = delta

    def forward(self, y_pred: torch.Tensor, y_true: torch.Tensor) -> torch.Tensor:
        # Smooth L1 (Huber) base loss
        base_loss = F.huber_loss(y_pred, y_true, delta=self.delta, reduction="none")

        # Penalty mask for directional disagreement
        wrong_direction = (y_pred * y_true) < 0.0
        weights = torch.where(wrong_direction, self.alpha, 1.0)

        return torch.mean(weights * base_loss)


class SharpeAwareLoss(nn.Module):
    """Sharpe-ratio-aware objective maximizing return while penalizing volatility."""

    def __init__(self, risk_aversion: float = 1.0, eps: float = 1e-6):
        super().__init__()
        self.risk_aversion = risk_aversion
        self.eps = eps

    def forward(self, y_pred: torch.Tensor, y_true: torch.Tensor) -> torch.Tensor:
        # Position sizing proportional to predicted signal
        positions = torch.tanh(y_pred)
        strategy_returns = positions * y_true

        mean_ret = torch.mean(strategy_returns)
        std_ret = torch.std(strategy_returns) + self.eps

        # Negative Sharpe ratio
        sharpe = mean_ret / std_ret
        return -sharpe


class SortinoAwareLoss(nn.Module):
    """Sortino-ratio objective maximizing strategy return while penalizing downside semi-deviation."""

    def __init__(self, target_return: float = 0.0, eps: float = 1e-6):
        super().__init__()
        self.target_return = target_return
        self.eps = eps

    def forward(self, y_pred: torch.Tensor, y_true: torch.Tensor) -> torch.Tensor:
        # Position sizing bounded in [-1, 1]
        positions = torch.tanh(y_pred)
        strategy_returns = positions * y_true

        mean_ret = torch.mean(strategy_returns)
        # Penalize only downside returns below target
        downside = torch.clamp(self.target_return - strategy_returns, min=0.0)
        downside_dev = torch.sqrt(torch.mean(downside**2) + self.eps)

        sortino = (mean_ret - self.target_return) / downside_dev
        return -sortino

