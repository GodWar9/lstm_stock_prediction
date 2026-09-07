"""Production PyTorch model training loop with early stopping and learning rate scheduling."""

from typing import Dict, List, Optional
import os
import torch
import torch.nn as nn
from torch.utils.data import DataLoader


class ModelTrainer:
    """Trainer with gradient clipping, early stopping, and checkpointing."""

    def __init__(
        self,
        model: nn.Module,
        optimizer: torch.optim.Optimizer,
        criterion: nn.Module,
        device: Optional[torch.device] = None,
        clip_grad_norm: float = 1.0,
        scheduler: Optional[torch.optim.lr_scheduler._LRScheduler] = None,
    ):
        self.device = device or torch.device("cuda" if torch.cuda.is_available() else "cpu")
        self.model = model.to(self.device)
        self.optimizer = optimizer
        self.criterion = criterion
        self.clip_grad_norm = clip_grad_norm
        self.scheduler = scheduler
        self.best_val_loss = float("inf")
        self.best_model_state = None

    def train_epoch(self, dataloader: DataLoader) -> float:
        self.model.train()
        total_loss = 0.0
        n_batches = 0

        for x_batch, y_batch in dataloader:
            x_batch = x_batch.to(self.device)
            y_batch = y_batch.to(self.device).view(-1, 1)

            self.optimizer.zero_grad()
            preds = self.model(x_batch)
            loss = self.criterion(preds, y_batch)
            loss.backward()

            if self.clip_grad_norm > 0:
                nn.utils.clip_grad_norm_(self.model.parameters(), self.clip_grad_norm)

            self.optimizer.step()
            total_loss += loss.item()
            n_batches += 1

        if self.scheduler is not None:
            self.scheduler.step()

        return total_loss / max(1, n_batches)

    @torch.no_grad()
    def evaluate(self, dataloader: DataLoader) -> float:
        self.model.eval()
        total_loss = 0.0
        n_batches = 0

        for x_batch, y_batch in dataloader:
            x_batch = x_batch.to(self.device)
            y_batch = y_batch.to(self.device).view(-1, 1)

            preds = self.model(x_batch)
            loss = self.criterion(preds, y_batch)
            total_loss += loss.item()
            n_batches += 1

        return total_loss / max(1, n_batches)

    def fit(
        self,
        train_loader: DataLoader,
        val_loader: Optional[DataLoader] = None,
        epochs: int = 50,
        patience: int = 10,
    ) -> Dict[str, List[float]]:
        history = {"train_loss": [], "val_loss": []}
        patience_counter = 0

        for epoch in range(epochs):
            train_loss = self.train_epoch(train_loader)
            history["train_loss"].append(train_loss)

            if val_loader is not None:
                val_loss = self.evaluate(val_loader)
                history["val_loss"].append(val_loss)

                if val_loss < self.best_val_loss:
                    self.best_val_loss = val_loss
                    self.best_model_state = {
                        k: v.cpu().clone() for k, v in self.model.state_dict().items()
                    }
                    patience_counter = 0
                else:
                    patience_counter += 1
                    if patience_counter >= patience:
                        print(f"Early stopping triggered at epoch {epoch + 1}")
                        break
            else:
                self.best_model_state = {
                    k: v.cpu().clone() for k, v in self.model.state_dict().items()
                }

        # Restore best weights
        if self.best_model_state is not None:
            self.model.load_state_dict(self.best_model_state)

        return history

    def save_checkpoint(self, path: str):
        os.makedirs(os.path.dirname(path), exist_ok=True)
        torch.save(
            {
                "model_state": self.model.state_dict(),
                "best_val_loss": self.best_val_loss,
            },
            path,
        )
