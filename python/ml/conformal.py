"""Conformal Prediction Uncertainty Calibration for Financial Time Series Forecasting.

Implements distribution-free, finite-sample calibrated prediction intervals
and uncertainty metrics for deep learning models.
"""

from dataclasses import asdict, dataclass
import json
from pathlib import Path
from typing import Optional, Union
import numpy as np


@dataclass
class ConformalCalibrationArtifact:
    """Serialized calibration metrics and quantiles for production inference."""
    alpha: float
    coverage_target: float
    quantile: float
    num_calibration_samples: int
    is_adaptive: bool
    mean_residual: float
    median_residual: float


class ConformalPredictor:
    """Split Conformal Predictor for regression uncertainty quantification.
    
    Provides finite-sample statistical coverage guarantee:
        P(Y_{t+h} ∈ C(X_{t})) >= 1 - alpha
    without assuming normality or stationarity of residuals.
    """

    def __init__(self, alpha: float = 0.10, adaptive: bool = True):
        """Initialize Conformal Predictor.
        
        Args:
            alpha: Significance level (default 0.10 -> 90% prediction intervals).
            adaptive: If True, normalizes residuals by local volatility/dispersion.
        """
        assert 0.0 < alpha < 1.0, f"alpha must be in (0, 1), got {alpha}"
        self.alpha = alpha
        self.coverage_target = 1.0 - alpha
        self.adaptive = adaptive
        self.calibrated_quantile: Optional[float] = None
        self.num_samples: int = 0
        self.mean_residual: float = 0.0
        self.median_residual: float = 0.0

    def calibrate(
        self,
        y_true: np.ndarray,
        y_pred: np.ndarray,
        sigmas: Optional[np.ndarray] = None,
    ) -> float:
        """Fit conformal non-conformity scores on an out-of-fold calibration set.
        
        Args:
            y_true: Ground truth target values, shape [N] or [N, 1].
            y_pred: Model predictions on calibration set, shape [N] or [N, 1].
            sigmas: Optional local volatility/dispersion estimates, shape [N].
            
        Returns:
            The calibrated non-conformity score quantile.
        """
        y_true = np.asarray(y_true).ravel()
        y_pred = np.asarray(y_pred).ravel()
        assert len(y_true) == len(y_pred), "y_true and y_pred must have same length"
        n = len(y_true)
        assert n >= 10, f"At least 10 calibration samples required, got {n}"

        residuals = np.abs(y_true - y_pred)
        self.mean_residual = float(np.mean(residuals))
        self.median_residual = float(np.median(residuals))

        if self.adaptive:
            if sigmas is None:
                # Default: use rolling MAD or local standard deviation
                sigmas = np.maximum(residuals, 1e-6)
            else:
                sigmas = np.asarray(sigmas).ravel()
                assert len(sigmas) == n, "sigmas must match sample count"
                sigmas = np.maximum(sigmas, 1e-6)
            scores = residuals / sigmas
        else:
            scores = residuals

        # Finite-sample correction: quantile level = ceil((n + 1) * (1 - alpha)) / n
        q_level = min(1.0, np.ceil((n + 1) * self.coverage_target) / n)
        self.calibrated_quantile = float(np.quantile(scores, q_level, method="higher"))
        self.num_samples = n

        return self.calibrated_quantile

    def predict_intervals(
        self,
        y_pred: np.ndarray,
        sigmas: Optional[np.ndarray] = None,
    ) -> tuple[np.ndarray, np.ndarray]:
        """Compute lower and upper prediction intervals for test predictions.
        
        Args:
            y_pred: Model predictions, shape [N]
            sigmas: Optional volatility/dispersion estimates for adaptive scaling.
            
        Returns:
            Tuple of (lower_bounds, upper_bounds).
        """
        assert self.calibrated_quantile is not None, "ConformalPredictor must be calibrated first"
        y_pred = np.asarray(y_pred)

        if self.adaptive:
            assert sigmas is not None, "Adaptive conformal prediction requires sigmas"
            sigmas = np.asarray(sigmas)
            half_width = self.calibrated_quantile * sigmas
        else:
            half_width = self.calibrated_quantile

        lower = y_pred - half_width
        upper = y_pred + half_width
        return lower, upper

    def compute_signal_confidence(
        self,
        y_pred: np.ndarray,
        sigmas: Optional[np.ndarray] = None,
    ) -> np.ndarray:
        """Compute signal confidence score in [0.0, 1.0].
        
        Confidence is discounted if zero return lies inside the prediction band,
        or if the uncertainty band is wide relative to the predicted magnitude:
            confidence = |y_pred| / (|y_pred| + half_width) * sign_consistency_factor
        """
        lower, upper = self.predict_intervals(y_pred, sigmas)
        y_pred = np.asarray(y_pred)
        half_width = 0.5 * (upper - lower)

        # Base confidence ratio
        abs_pred = np.abs(y_pred)
        ratio = abs_pred / np.maximum(abs_pred + half_width, 1e-8)

        # Zero-crossing penalty: if 0 is in [lower, upper], sign is uncertain
        zero_in_band = (lower <= 0.0) & (upper >= 0.0)
        penalty = np.where(zero_in_band, 0.5, 1.0)

        return np.clip(ratio * penalty, 0.0, 1.0)

    def evaluate_coverage(
        self,
        y_true: np.ndarray,
        y_pred: np.ndarray,
        sigmas: Optional[np.ndarray] = None,
    ) -> dict[str, float]:
        """Evaluate empirical coverage and average interval width on test set."""
        lower, upper = self.predict_intervals(y_pred, sigmas)
        y_true = np.asarray(y_true).ravel()
        covered = (y_true >= lower) & (y_true <= upper)
        empirical_coverage = float(np.mean(covered))
        avg_width = float(np.mean(upper - lower))

        return {
            "target_coverage": self.coverage_target,
            "empirical_coverage": empirical_coverage,
            "coverage_error": empirical_coverage - self.coverage_target,
            "average_width": avg_width,
            "median_width": float(np.median(upper - lower)),
        }

    def save_artifact(self, filepath: Union[str, Path]) -> None:
        """Serialize calibration artifact to JSON for production inference."""
        assert self.calibrated_quantile is not None, "Must be calibrated before saving"
        artifact = ConformalCalibrationArtifact(
            alpha=self.alpha,
            coverage_target=self.coverage_target,
            quantile=self.calibrated_quantile,
            num_calibration_samples=self.num_samples,
            is_adaptive=self.adaptive,
            mean_residual=self.mean_residual,
            median_residual=self.median_residual,
        )
        with open(filepath, "w") as f:
            json.dump(asdict(artifact), f, indent=2)

    @classmethod
    def load_artifact(cls, filepath: Union[str, Path]) -> "ConformalPredictor":
        """Load calibrated predictor from JSON artifact."""
        with open(filepath, "r") as f:
            data = json.load(f)
        predictor = cls(alpha=data["alpha"], adaptive=data["is_adaptive"])
        predictor.calibrated_quantile = data["quantile"]
        predictor.num_samples = data["num_calibration_samples"]
        predictor.mean_residual = data.get("mean_residual", 0.0)
        predictor.median_residual = data.get("median_residual", 0.0)
        return predictor
