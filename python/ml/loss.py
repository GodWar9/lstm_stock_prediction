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
