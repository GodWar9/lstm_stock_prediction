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


class MultiHorizonLoss(nn.Module):
    """Multi-horizon composite loss function.

    Computes weighted loss across multiple forecasting horizons:
    L = Σ_{k} w_k * L_{base}(y_pred^{(k)}, y_true^{(k)})
    """

    def __init__(
        self,
        base_criterion: Optional[nn.Module] = None,
        weights: Optional[list[float]] = None,
    ):
        super().__init__()
        self.base_criterion = base_criterion or DirectionalAsymmetricLoss(alpha=2.0)
        self.weights = weights

    def forward(self, y_pred: torch.Tensor, y_true: torch.Tensor) -> torch.Tensor:
        """Forward pass.

        Args:
            y_pred: Tensor of shape [batch_size, num_horizons]
            y_true: Tensor of shape [batch_size, num_horizons]

        Returns:
            Weighted scalar loss.
        """
        assert y_pred.shape == y_true.shape, f"Shape mismatch: {y_pred.shape} vs {y_true.shape}"
        num_horizons = y_pred.shape[-1]

        if self.weights is None:
            # Default: equal weighting
            weights = [1.0 / num_horizons] * num_horizons
        else:
            assert len(self.weights) == num_horizons, (
                f"Weights length {len(self.weights)} != horizons {num_horizons}"
            )
            total = sum(self.weights)
            weights = [w / total for w in self.weights]

        total_loss = torch.tensor(0.0, device=y_pred.device, dtype=y_pred.dtype)
        for h_idx, w in enumerate(weights):
            pred_h = y_pred[:, h_idx : h_idx + 1]
            true_h = y_true[:, h_idx : h_idx + 1]
            loss_h = self.base_criterion(pred_h, true_h)
            total_loss = total_loss + w * loss_h

        return total_loss
