"""LSTM Model Architectures for Financial Time Series Forecasting."""

from typing import Optional
import torch
import torch.nn as nn


class LSTMForecaster(nn.Module):
    """Multi-layer LSTM model with residual projection head and custom gate init."""

    def __init__(
        self,
        input_dim: int,
        hidden_dim: int = 128,
        num_layers: int = 2,
        dropout: float = 0.2,
        output_dim: int = 1,
    ):
        super().__init__()
        self.input_dim = input_dim
        self.hidden_dim = hidden_dim
        self.num_layers = num_layers

        self.lstm = nn.LSTM(
            input_size=input_dim,
            hidden_size=hidden_dim,
            num_layers=num_layers,
            batch_first=True,
            dropout=dropout if num_layers > 1 else 0.0,
        )

        self.layer_norm = nn.LayerNorm(hidden_dim)
        self.head = nn.Sequential(
            nn.Linear(hidden_dim, hidden_dim // 2),
            nn.GELU(),
            nn.Dropout(dropout),
            nn.Linear(hidden_dim // 2, output_dim),
        )

        self._init_weights()

    def _init_weights(self):
        """Initialize LSTM gates with forget-gate bias = 1.0 and Xavier uniform weights."""
        for name, param in self.lstm.named_parameters():
            if "weight_ih" in name:
                nn.init.xavier_uniform_(param.data)
            elif "weight_hh" in name:
                nn.init.orthogonal_(param.data)
            elif "bias" in name:
                param.data.fill_(0.0)
                # In PyTorch LSTM, bias has 4 * hidden_dim elements:
                # [b_ii|b_if|b_ig|b_io]. Second quarter is forget gate (b_if).
                n = param.size(0)
                param.data[n // 4 : n // 2].fill_(1.0)

        for m in self.head.modules():
            if isinstance(m, nn.Linear):
                nn.init.kaiming_normal_(m.weight, nonlinearity="relu")
                if m.bias is not None:
                    m.bias.data.fill_(0.0)

    def forward(
        self,
        x: torch.Tensor,
        state: Optional[tuple] = None,
    ) -> torch.Tensor:
        """Forward pass.
        
        Args:
            x: Tensor of shape [batch_size, seq_len, input_dim]
            state: Optional tuple (h_0, c_0)
            
        Returns:
            Tensor of shape [batch_size, output_dim]
        """
        # lstm_out: [batch_size, seq_len, hidden_dim]
        lstm_out, _ = self.lstm(x, state)

        # Take last time-step representation
        last_hidden = lstm_out[:, -1, :]
        normed = self.layer_norm(last_hidden)
        out = self.head(normed)
        return out
